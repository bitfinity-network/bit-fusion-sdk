use std::time::Duration;

use candid::Principal;
use did::{H160, U256, U64};
use eth_signer::{Signer, Wallet};
use ethers_core::abi::{Constructor, Param, ParamType, Token};
use ethers_core::k256::ecdsa::SigningKey;
use evm_canister_client::EvmCanisterClient;
use minter_contract_utils::bft_bridge_api::{
    self, decode_burn_operation_id, NATIVE_TOKEN_BALANCE, NATIVE_TOKEN_DEPOSIT,
};
use minter_contract_utils::build_data::test_contracts::{
    BFT_BRIDGE_SMART_CONTRACT_CODE, TEST_WTM_HEX_CODE,
};
use minter_contract_utils::evm_bridge::BridgeSide;
use minter_did::id256::Id256;
use minter_did::order::SignedMintOrder;

use super::PocketIcTestContext;
use crate::context::{CanisterType, TestContext};
use crate::pocket_ic_integration_test::ADMIN;
use crate::utils::{self, CHAIN_ID};

// Create a second EVM canister (base_evm) instance and create BFTBridge contract on it, // It will play role of external evm
// Create erc20-minter instance, initialized with EvmInfos for both EVM canisters.
// Deploy ERC-20 token on external_evm,
// Deploy Wrapped token on first EVM for the ERC-20 from previous step,
// Approve ERC-20 transfer on behalf of some user in external_evm,
// Call BFTBridge::burn() on behalf of the user in external_evm.
// Wait some time for the erc20-minter see and process it.
// Make sure the tokens minted.
// Make sure SignedMintOrder removed from erc20-minter after some time.
#[tokio::test]
async fn test_external_bridging() {
    let ctx = PocketIcTestContext::new(&CanisterType::EVM_MINTER_TEST_SET).await;

    // Deploy external EVM canister.
    let external_evm = ctx.canisters().external_evm();
    let base_evm_client = EvmCanisterClient::new(ctx.client(external_evm, ctx.admin_name()));

    println!("Deployed external EVM canister: {}", external_evm);
    println!("Deployed EVM canister: {}", ctx.canisters().evm());

    let mut rng = rand::thread_rng();

    let bob_wallet = Wallet::new(&mut rng);
    let bob_address: H160 = bob_wallet.address().into();

    // Mint native tokens for bob in both evms
    base_evm_client
        .mint_native_tokens(bob_address.clone(), u64::MAX.into())
        .await
        .unwrap()
        .unwrap();
    ctx.evm_client(ADMIN)
        .mint_native_tokens(bob_address.clone(), u64::MAX.into())
        .await
        .unwrap()
        .unwrap();
    ctx.advance_time(Duration::from_secs(2)).await;

    // get evm minter canister address
    let erc20_minter_client = ctx.client(ctx.canisters().ck_erc20_minter(), ADMIN);
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
    base_evm_client
        .mint_native_tokens(erc20_minter_address.clone(), u64::MAX.into())
        .await
        .unwrap()
        .unwrap();
    ctx.advance_time(Duration::from_secs(2)).await;

    // Deploy the BFTBridge contract on the external EVM.
    let base_bft_bridge = create_bft_bridge(
        &ctx,
        external_evm,
        erc20_minter_address.clone(),
        &bob_wallet,
    )
    .await;
    let wrapped_bft_bridge = create_bft_bridge(
        &ctx,
        ctx.canisters().evm(),
        erc20_minter_address.clone(),
        &bob_wallet,
    )
    .await;

    // set bridge contract addresses in minter canister
    erc20_minter_client
        .update::<_, Option<()>>(
            "admin_set_bft_bridge_address",
            (BridgeSide::Wrapped, wrapped_bft_bridge.clone()),
        )
        .await
        .unwrap()
        .unwrap();
    erc20_minter_client
        .update::<_, Option<()>>(
            "admin_set_bft_bridge_address",
            (BridgeSide::Base, base_bft_bridge.clone()),
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

    let nonce = base_evm_client
        .account_basic(bob_address.clone())
        .await
        .unwrap()
        .nonce;
    let tx = ctx.signed_transaction(&bob_wallet, None, nonce, 0, data);
    let erc20_token_address = {
        let hash = base_evm_client
            .send_raw_transaction(tx)
            .await
            .unwrap()
            .unwrap();

        let receipt = ctx
            .wait_transaction_receipt_on_evm(&base_evm_client, &hash)
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
            &wrapped_bft_bridge,
            token_id,
        )
        .await
        .unwrap();

    // Approve ERC-20 transfer on behalf of some user in base EVM.
    let alice_wallet = ctx.new_wallet(u128::MAX).await.unwrap();
    let alice_address: H160 = alice_wallet.address().into();

    let input = NATIVE_TOKEN_DEPOSIT
        .encode_input(&[Token::Address(bob_address.0)])
        .unwrap();

    // spender should deposit native tokens to bft bridge, to pay fee.
    let expected_init_native_balance = 10_u64.pow(15);
    let receipt = ctx
        .call_contract(
            &bob_wallet,
            &wrapped_bft_bridge,
            input,
            expected_init_native_balance.into(),
        )
        .await
        .unwrap()
        .1;
    let init_native_balance = NATIVE_TOKEN_DEPOSIT
        .decode_output(receipt.output.as_ref().unwrap())
        .unwrap()
        .first()
        .cloned()
        .unwrap()
        .into_uint()
        .unwrap();
    assert_eq!(init_native_balance, expected_init_native_balance.into());

    let nonce = base_evm_client
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
        Token::Address(base_bft_bridge.clone().into()),
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

    let approve_tx_hash = base_evm_client
        .send_raw_transaction(approve_tx)
        .await
        .unwrap()
        .unwrap();

    let receipt = ctx
        .wait_transaction_receipt_on_evm(&base_evm_client, &approve_tx_hash)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(receipt.status, Some(U64::one()));

    // Call BFTBridge::burn() on behalf of the user in external EVM.
    let nonce = base_evm_client
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
        Some(base_bft_bridge.clone()),
        nonce.clone(),
        0,
        input,
    );

    let burn_tx_hash = base_evm_client
        .send_raw_transaction(burn_tx)
        .await
        .unwrap()
        .unwrap();

    // Wait some time for the erc20-minter see and process it.
    let burn_receipt = ctx
        .wait_transaction_receipt_on_evm(&base_evm_client, &burn_tx_hash)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(burn_receipt.status, Some(U64::one()));

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
    ctx.advance_time(Duration::from_secs(2)).await;

    // Check mint order removed
    let bob_address_id = Id256::from_evm_address(&bob_address, CHAIN_ID as _);
    let burn_operation_id =
        decode_burn_operation_id(burn_receipt.output.unwrap().as_slice()).unwrap();
    let signed_order = erc20_minter_client
        .update::<_, Option<SignedMintOrder>>(
            "get_mint_order",
            (bob_address_id, token_id, burn_operation_id),
        )
        .await
        .unwrap();

    assert!(signed_order.is_none());

    // Check fee charged
    let input = NATIVE_TOKEN_BALANCE
        .encode_input(&[Token::Address(bob_address.0)])
        .unwrap();
    let response = ctx
        .evm_client(ADMIN)
        .eth_call(
            Some(bob_address),
            Some(wrapped_bft_bridge),
            None,
            3_000_000,
            None,
            Some(input.into()),
        )
        .await
        .unwrap()
        .unwrap();
    let native_balance_after_mint = NATIVE_TOKEN_DEPOSIT
        .decode_output(&hex::decode(response.trim_start_matches("0x")).unwrap())
        .unwrap()
        .first()
        .cloned()
        .unwrap()
        .into_uint()
        .unwrap();
    assert!(native_balance_after_mint > U256::zero().0);
    assert!(native_balance_after_mint < init_native_balance);
}

#[tokio::test]
async fn mint_should_fail_if_not_enough_tokens_on_fee_deposit() {
    let ctx = PocketIcTestContext::new(&CanisterType::EVM_MINTER_TEST_SET).await;

    // Deploy external EVM canister.
    let external_evm = ctx.canisters().external_evm();
    let base_evm_client = EvmCanisterClient::new(ctx.client(external_evm, ctx.admin_name()));

    println!("Deployed external EVM canister: {}", external_evm);
    println!("Deployed EVM canister: {}", ctx.canisters().evm());

    let mut rng = rand::thread_rng();

    let bob_wallet = Wallet::new(&mut rng);
    let bob_address: H160 = bob_wallet.address().into();

    // Mint native tokens for bob in both evms
    base_evm_client
        .mint_native_tokens(bob_address.clone(), u64::MAX.into())
        .await
        .unwrap()
        .unwrap();
    ctx.evm_client(ADMIN)
        .mint_native_tokens(bob_address.clone(), u64::MAX.into())
        .await
        .unwrap()
        .unwrap();
    ctx.advance_time(Duration::from_secs(2)).await;

    // get evm minter canister address
    let erc20_minter_client = ctx.client(ctx.canisters().ck_erc20_minter(), ADMIN);
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
    base_evm_client
        .mint_native_tokens(erc20_minter_address.clone(), u64::MAX.into())
        .await
        .unwrap()
        .unwrap();
    ctx.advance_time(Duration::from_secs(2)).await;

    // Deploy the BFTBridge contract on the external EVM.
    let base_bft_bridge = create_bft_bridge(
        &ctx,
        external_evm,
        erc20_minter_address.clone(),
        &bob_wallet,
    )
    .await;
    let wrapped_bft_bridge = create_bft_bridge(
        &ctx,
        ctx.canisters().evm(),
        erc20_minter_address.clone(),
        &bob_wallet,
    )
    .await;

    // set bridge contract addresses in minter canister
    erc20_minter_client
        .update::<_, Option<()>>(
            "admin_set_bft_bridge_address",
            (BridgeSide::Wrapped, wrapped_bft_bridge.clone()),
        )
        .await
        .unwrap()
        .unwrap();
    erc20_minter_client
        .update::<_, Option<()>>(
            "admin_set_bft_bridge_address",
            (BridgeSide::Base, base_bft_bridge.clone()),
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

    let nonce = base_evm_client
        .account_basic(bob_address.clone())
        .await
        .unwrap()
        .nonce;
    let tx = ctx.signed_transaction(&bob_wallet, None, nonce, 0, data);
    let erc20_token_address = {
        let hash = base_evm_client
            .send_raw_transaction(tx)
            .await
            .unwrap()
            .unwrap();

        let receipt = ctx
            .wait_transaction_receipt_on_evm(&base_evm_client, &hash)
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
            &wrapped_bft_bridge,
            token_id,
        )
        .await
        .unwrap();

    // Approve ERC-20 transfer on behalf of some user in base EVM.
    let alice_wallet = ctx.new_wallet(u128::MAX).await.unwrap();
    let alice_address: H160 = alice_wallet.address().into();

    let nonce = base_evm_client
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
        Token::Address(base_bft_bridge.clone().into()),
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

    let approve_tx_hash = base_evm_client
        .send_raw_transaction(approve_tx)
        .await
        .unwrap()
        .unwrap();

    let receipt = ctx
        .wait_transaction_receipt_on_evm(&base_evm_client, &approve_tx_hash)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(receipt.status, Some(U64::one()));

    // Call BFTBridge::burn() on behalf of the user in external EVM.
    let nonce = base_evm_client
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
        Some(base_bft_bridge.clone()),
        nonce.clone(),
        0,
        input,
    );

    let burn_tx_hash = base_evm_client
        .send_raw_transaction(burn_tx)
        .await
        .unwrap()
        .unwrap();

    // Wait some time for the erc20-minter see and process it.
    let burn_receipt = ctx
        .wait_transaction_receipt_on_evm(&base_evm_client, &burn_tx_hash)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(burn_receipt.status, Some(U64::one()));

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

    assert_eq!(0, U256::from_hex_str(&balance).unwrap().0.as_u64());

    // Wait for mint order removal
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;

    // Check mint order removed
    let bob_address_id = Id256::from_evm_address(&bob_address, CHAIN_ID as _);
    let burn_operation_id =
        decode_burn_operation_id(burn_receipt.output.unwrap().as_slice()).unwrap();
    let signed_order = erc20_minter_client
        .update::<_, Option<SignedMintOrder>>(
            "get_mint_order",
            (bob_address_id, token_id, burn_operation_id),
        )
        .await
        .unwrap();

    assert!(signed_order.is_some());
}

async fn create_bft_bridge(
    ctx: &PocketIcTestContext,
    evm: Principal,
    erc20_minter_address: H160,
    wallet: &Wallet<'_, SigningKey>,
) -> H160 {
    let evm_client = EvmCanisterClient::new(ctx.client(evm, ADMIN));
    let contract = BFT_BRIDGE_SMART_CONTRACT_CODE.clone();
    let input = bft_bridge_api::CONSTRUCTOR
        .encode_input(
            contract,
            &[Token::Address(erc20_minter_address.clone().into())],
        )
        .unwrap();

    let nonce = evm_client
        .account_basic(wallet.address().into())
        .await
        .unwrap()
        .nonce;

    let create_contract_tx = ctx.signed_transaction(wallet, None, nonce.clone(), 0, input);

    let hash = evm_client
        .send_raw_transaction(create_contract_tx)
        .await
        .unwrap()
        .unwrap();

    ctx.wait_transaction_receipt_on_evm(&evm_client, &hash)
        .await
        .unwrap()
        .unwrap()
        .contract_address
        .expect("contract address")
}
