use std::time::Duration;

use did::{H160, U256, U64};
use erc20_minter::state::Settings;
use eth_signer::{Signer, Wallet};
use ethers_core::abi::{Constructor, Param, ParamType, Token};
use evm_canister_client::EvmCanisterClient;
use ic_log::LogSettings;
use icrc2_minter::SigningStrategy;
use minter_contract_utils::bft_bridge_api;
use minter_contract_utils::bft_bridge_api::BURN;
use minter_contract_utils::build_data::test_contracts::{
    BFT_BRIDGE_SMART_CONTRACT_CODE, TEST_WTM_HEX_CODE,
};
use minter_contract_utils::evm_bridge::BridgeSide;
use minter_contract_utils::evm_link::EvmLink;
use minter_did::id256::Id256;
use minter_did::order::SignedMintOrder;

use super::PocketIcTestContext;
use crate::context::{evm_canister_init_data, CanisterType, TestContext};
use crate::pocket_ic_integration_test::ADMIN;
use crate::utils::{self, CHAIN_ID};

// Create a second EVM canister (external_evm) instance and create BFTBridge contract on it, // It will play role of external evm
// Create erc20-minter instance, initialized with EvmInfos for both EVM canisters.
// Deploy ERC-20 token on external_evm,
// Deploy Wrapped token on first EVM for the ERC-20 from previous step,
// Approve ERC-20 transfer on behalf of some user in external_evm,
// Call BFTBridge::burn() on behalf of the user in external_evm.
// Wait some time for the erc20-minter see and process it.
// Query SignedMintOrder from the erc20-minter. // Endpoint for it is In progress now.
// Send the SignedMintOrder to the BFTBridge::mint() endpoint of the first EVM.
// Make sure the tokens minted.
// Make sure SignedMintOrder removed from erc20-minter after some time.
#[tokio::test]
async fn test_external_bridging() {
    let ctx = PocketIcTestContext::new(&CanisterType::EVM_TEST_SET).await;
    let john_wallet = ctx.new_wallet(u128::MAX).await.unwrap();

    // Deploy external EVM canister.
    let (external_evm, external_evm_client) = {
        let external_evm = ctx
            .deploy_canister(
                CanisterType::Evm,
                (evm_canister_init_data(
                    ctx.canisters.signature_verification(),
                    ctx.admin(),
                    None,
                ),),
            )
            .await;

        (
            external_evm,
            EvmCanisterClient::new(ctx.client(external_evm, ctx.admin_name())),
        )
    };

    println!("Deployed external EVM canister: {}", external_evm);
    println!("Deployed EVM canister: {}", ctx.canisters().evm());

    // whitelist external EVM canister.
    {
        let signature =
            signature_verification_canister_client::SignatureVerificationCanisterClient::new(
                ctx.client(ctx.canisters.signature_verification(), ctx.admin_name()),
            );

        signature
            .add_principal_to_access_list(external_evm)
            .await
            .unwrap()
            .unwrap();
    }

    let bob_wallet = {
        let wallet = {
            let mut rng = rand::thread_rng();
            Wallet::new(&mut rng)
        };

        external_evm_client
            .mint_native_tokens(wallet.address().into(), 10_u64.pow(18).into())
            .await
            .unwrap()
            .unwrap();
        ctx.advance_time(Duration::from_secs(4)).await;

        wallet
    };
    let bob_address: H160 = bob_wallet.address().into();

    // Initialize erc20-minter with EvmInfos for both EVM canisters.
    let erc20_minter_canister = ctx
        .deploy_canister(
            CanisterType::EvmMinter,
            (Settings {
                base_evm_link: EvmLink::Ic(external_evm),
                wrapped_evm_link: EvmLink::Ic(ctx.canisters().evm()),
                base_bridge_contract: H160::default(),
                wrapped_bridge_contract: H160::default(),
                signing_strategy: SigningStrategy::Local {
                    private_key: [135; 32],
                },
                log_settings: Some(LogSettings {
                    enable_console: true,
                    in_memory_records: None,
                    log_filter: Some("trace".to_string()),
                }),
            },),
        )
        .await;

    println!("Deployed EVM minter canister: {}", erc20_minter_canister);

    // get evm minter canister address
    let erc20_minter_client = ctx.client(erc20_minter_canister, ADMIN);
    let erc20_minter_address = erc20_minter_client
        .update::<_, Option<H160>>("get_evm_address", ())
        .await
        .unwrap()
        .unwrap();

    // mint native tokens for the erc20-minter on both EVMs
    println!("Minting native tokens on both EVMs for {erc20_minter_address}");
    ctx.evm_client(ADMIN)
        .mint_native_tokens(erc20_minter_address.clone(), u64::MAX.into())
        .await
        .unwrap()
        .unwrap();
    external_evm_client
        .mint_native_tokens(erc20_minter_address.clone(), u64::MAX.into())
        .await
        .unwrap()
        .unwrap();
    ctx.advance_time(Duration::from_secs(2)).await;

    // Deploy the BFTBridge contract on the external EVM.
    let contract = BFT_BRIDGE_SMART_CONTRACT_CODE.clone();
    let input = bft_bridge_api::CONSTRUCTOR
        .encode_input(
            contract,
            &[Token::Address(erc20_minter_address.clone().into())],
        )
        .unwrap();

    let nonce = external_evm_client
        .account_basic(bob_address.clone())
        .await
        .unwrap()
        .nonce;

    let create_contract_tx = ctx.signed_transaction(&bob_wallet, None, nonce.clone(), 0, input);

    let hash = external_evm_client
        .send_raw_transaction(create_contract_tx)
        .await
        .unwrap()
        .unwrap();

    let external_bridge_address = ctx
        .wait_transaction_receipt_on_evm(&external_evm_client, &hash)
        .await
        .unwrap()
        .unwrap()
        .contract_address
        .expect("contract address");

    // Init BFTBridge contract in EVMc.
    let contract = BFT_BRIDGE_SMART_CONTRACT_CODE.clone();
    let input = bft_bridge_api::CONSTRUCTOR
        .encode_input(
            contract,
            &[Token::Address(erc20_minter_address.clone().into())],
        )
        .unwrap();
    let evmc_bridge_address = ctx
        .create_contract(&john_wallet, input.clone())
        .await
        .unwrap();

    // set bridge contract addresses in minter canister
    erc20_minter_client
        .update::<_, Option<()>>(
            "admin_set_bft_bridge_address",
            (BridgeSide::Wrapped, evmc_bridge_address.clone()),
        )
        .await
        .unwrap()
        .unwrap();
    erc20_minter_client
        .update::<_, Option<()>>(
            "admin_set_bft_bridge_address",
            (BridgeSide::Base, external_bridge_address.clone()),
        )
        .await
        .unwrap()
        .unwrap();

    // Deploy ERC-20 token on external EVM.
    let data: Constructor = Constructor {
        inputs: vec![Param {
            name: "initialSupply".into(),
            kind: ParamType::Uint(256),
            internal_type: None,
        }],
    };

    let data = data
        .encode_input(TEST_WTM_HEX_CODE.clone(), &[Token::Uint(u64::MAX.into())])
        .unwrap();

    let nonce = nonce.0.as_u64() + 1;
    let tx = ctx.signed_transaction(&bob_wallet, None, nonce.into(), 0, data);
    let erc20_token_address = {
        let hash = external_evm_client
            .send_raw_transaction(tx)
            .await
            .unwrap()
            .unwrap();

        let receipt = ctx
            .wait_transaction_receipt_on_evm(&external_evm_client, &hash)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(receipt.status, Some(U64::one()));

        receipt.contract_address.unwrap()
    };

    // Deploy Wrapped token on first EVM for the ERC-20 from previous step.
    let token_id = Id256::from_evm_address(&erc20_token_address, CHAIN_ID as _);
    let wrapped_token = ctx
        .create_wrapped_token(
            &ctx.new_wallet(u128::MAX).await.unwrap(),
            &evmc_bridge_address,
            token_id,
        )
        .await
        .unwrap();

    // Approve ERC-20 transfer on behalf of some user in external EVM.
    let alice_wallet = ctx.new_wallet(u128::MAX).await.unwrap();
    let alice_address: H160 = alice_wallet.address().into();

    let nonce = external_evm_client
        .account_basic(bob_address.clone())
        .await
        .unwrap()
        .nonce;

    let data = crate::utils::function_selector(
        "approve",
        &[
            Param {
                name: "spender".to_string(),
                kind: ethers_core::abi::ParamType::Address,
                internal_type: None,
            },
            Param {
                name: "amount".to_string(),
                kind: ethers_core::abi::ParamType::Uint(256),
                internal_type: None,
            },
        ],
    )
    .encode_input(&[
        Token::Address(external_bridge_address.clone().into()),
        Token::Uint(1000_u64.into()),
    ])
    .unwrap();

    let approve_tx = ctx.signed_transaction(
        &bob_wallet,
        Some(erc20_token_address.clone()),
        nonce.clone(),
        0,
        data,
    );

    let approve_tx_hash = external_evm_client
        .send_raw_transaction(approve_tx)
        .await
        .unwrap()
        .unwrap();

    let receipt = ctx
        .wait_transaction_receipt_on_evm(&external_evm_client, &approve_tx_hash)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(receipt.status, Some(U64::one()));

    // Call BFTBridge::burn() on behalf of the user in external EVM.
    let nonce = external_evm_client
        .account_basic(bob_address.clone())
        .await
        .unwrap()
        .nonce;

    let alice_id = Id256::from_evm_address(&alice_address, CHAIN_ID as _);

    let amount = 1000_u64;

    let input = bft_bridge_api::BURN
        .encode_input(&[
            Token::Uint(amount.into()),
            Token::Address(erc20_token_address.into()),
            Token::Bytes(alice_id.0.to_vec()),
        ])
        .unwrap();

    let burn_tx = ctx.signed_transaction(
        &bob_wallet,
        Some(external_bridge_address.clone()),
        nonce.clone(),
        0,
        input,
    );

    let burn_tx_hash = external_evm_client
        .send_raw_transaction(burn_tx)
        .await
        .unwrap()
        .unwrap();

    // Wait some time for the erc20-minter see and process it.
    let receipt = ctx
        .wait_transaction_receipt_on_evm(&external_evm_client, &burn_tx_hash)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(receipt.status, Some(U64::one()));

    let burn_operation_id = BURN
        .decode_output(receipt.output.unwrap().as_slice())
        .unwrap()
        .first()
        .cloned()
        .unwrap()
        .into_uint()
        .unwrap()
        .as_u32();

    // Query SignedMintOrder from the erc20-minter.
    let client = ctx.client(erc20_minter_canister, ADMIN);

    let bob_address_id = Id256::from_evm_address(&bob_address, CHAIN_ID as _);

    // Advance time to perform two tasks in erc20-minter:
    // 1. Minted event collection
    // 2. Mint order removal
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;

    // Chech the balance of the wrapped token.
    let data = utils::function_selector(
        "balanceOf",
        &[Param {
            name: "account".to_string(),
            kind: ParamType::Address,
            internal_type: None,
        }],
    )
    .encode_input(&[Token::Address(alice_address.clone().into())])
    .unwrap();

    let balance = ctx
        .evm_client(ADMIN)
        .eth_call(
            Some(alice_address),
            Some(wrapped_token.clone()),
            None,
            3_000_000,
            None,
            Some(data.into()),
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(amount, U256::from_hex_str(&balance).unwrap().0.as_u64());

    // Wait for mint order removal
    ctx.advance_time(Duration::from_secs(2)).await;

    // Check mint order removed
    let signed_order = client
        .update::<_, Option<SignedMintOrder>>(
            "get_mint_order",
            (bob_address_id, token_id, burn_operation_id),
        )
        .await
        .unwrap();

    assert!(signed_order.is_none())
}
