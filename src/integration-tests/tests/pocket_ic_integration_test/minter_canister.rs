use std::time::Duration;

use candid::{Nat, Principal};
use did::{H160, U256, U64};
use eth_signer::{Signer, Wallet};
use ethers_core::abi::{Constructor, Param, ParamType, Token};
use ethers_core::k256::ecdsa::SigningKey;
use evm_canister_client::EvmCanisterClient;
use evm_minter::client::EvmLink;
use evm_minter::state::Settings;
use ic_exports::ic_kit::mock_principals::{alice, john};
use ic_exports::icrc_types::icrc2::transfer_from::TransferFromError;
use ic_log::LogSettings;
use minter_canister::tokens::icrc1::IcrcTransferDst;
use minter_canister::SigningStrategy;
use minter_contract_utils::build_data::{
    BFT_BRIDGE_SMART_CONTRACT_CODE, WRAPPED_TOKEN_SMART_CONTRACT_CODE,
};
use minter_contract_utils::{bft_bridge_api, wrapped_token_api};
use minter_did::error::Error as McError;
use minter_did::id256::Id256;
use minter_did::order::SignedMintOrder;

use super::{PocketIcTestContext, JOHN};
use crate::context::{
    evm_canister_init_data, CanisterType, TestContext, ICRC1_INITIAL_BALANCE, ICRC1_TRANSFER_FEE,
};
use crate::pocket_ic_integration_test::{ADMIN, ALICE};
use crate::utils::error::TestError;
use crate::utils::{
    self, get_solidity_smart_contracts,
};

/// Initializez test environment with:
/// - john wallet with native tokens,
/// - opetaion points for john,
/// - bridge contract
async fn init_bridge() -> (PocketIcTestContext, Wallet<'static, SigningKey>, H160) {
    let ctx = PocketIcTestContext::new(&CanisterType::MINTER_TEST_SET).await;
    let john_wallet = ctx.new_wallet(u128::MAX).await.unwrap();

    let bft_bridge = ctx
        .initialize_bft_bridge(ADMIN, &john_wallet)
        .await
        .unwrap();
    (ctx, john_wallet, bft_bridge)
}

/// To be fixed in EPROD-634
#[ignore]
#[tokio::test]
async fn set_owner_access() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Minter]).await;
    let mut admin_client = ctx.minter_client(ADMIN);
    admin_client.set_owner(alice()).await.unwrap().unwrap();

    // Now Alice is owner, so admin can't update owner anymore.
    admin_client.set_owner(alice()).await.unwrap().unwrap_err();

    // Now Alice is owner, so she can update owner.
    let mut alice_client = ctx.minter_client(ALICE);
    alice_client.set_owner(alice()).await.unwrap().unwrap();
}

#[tokio::test]
async fn invalid_bridge_contract() {
    let ctx = PocketIcTestContext::new(&CanisterType::MINTER_TEST_SET).await;
    let minter_client = ctx.minter_client(ADMIN);
    let res = minter_client
        .register_evmc_bft_bridge(H160::from_slice(&[20; 20]))
        .await
        .unwrap()
        .unwrap_err();

    assert_eq!(res, McError::InvalidBftBridgeContract);
}

#[tokio::test]
async fn invalid_bridge() {
    let ctx = PocketIcTestContext::new(&CanisterType::MINTER_TEST_SET).await;
    let admin = ADMIN;
    let admin_wallet = ctx.new_wallet(u128::MAX).await.unwrap();
    let minter_canister_address = ctx.get_minter_canister_evm_address(admin).await.unwrap();

    let contract_code = WRAPPED_TOKEN_SMART_CONTRACT_CODE.clone();
    let input = wrapped_token_api::CONSTRUCTOR
        .encode_input(
            contract_code,
            &[
                Token::String("name".into()),
                Token::String("symbol".into()),
                Token::Address(minter_canister_address.into()),
            ],
        )
        .unwrap();
    let contract = ctx.create_contract(&admin_wallet, input).await.unwrap();

    let minter_client = ctx.minter_client(ADMIN);
    let res = minter_client
        .register_evmc_bft_bridge(contract)
        .await
        .unwrap()
        .unwrap_err();

    assert_eq!(res, McError::InvalidBftBridgeContract);
}

/// To be fixed in EPROD-634
#[ignore]
#[tokio::test]
async fn double_register_bridge() {
    let ctx = PocketIcTestContext::new(&CanisterType::MINTER_TEST_SET).await;
    let admin_wallet = ctx.new_wallet(u128::MAX).await.unwrap();

    let bft_bridge = ctx
        .initialize_bft_bridge(ADMIN, &admin_wallet)
        .await
        .unwrap();
    let err = ctx
        .initialize_bft_bridge(ADMIN, &admin_wallet)
        .await
        .unwrap_err();

    let TestError::MinterCanister(McError::BftBridgeAlreadyRegistered(registered)) = err else {
        panic!("unexpected error");
    };

    assert_eq!(registered, bft_bridge);
}

#[tokio::test]
async fn test_erc20_forbids_double_spend() {
    let (ctx, john_wallet, bft_bridge) = init_bridge().await;

    let base_token_id = Id256::from(&ctx.canisters().token_1());
    let wrapped_token = ctx
        .create_wrapped_token(&john_wallet, &bft_bridge, base_token_id)
        .await
        .unwrap();

    let amount = 300_000u64;
    let operation_id = 42;

    let mint_order = ctx
        .burn_icrc2(JOHN, &john_wallet, amount as _, operation_id)
        .await
        .unwrap();

    let receipt = ctx
        .mint_erc_20_with_order(&john_wallet, &bft_bridge, mint_order)
        .await
        .unwrap();
    assert_eq!(receipt.status, Some(U64::one()));

    let wrapped_balance = ctx
        .check_erc20_balance(&wrapped_token, &john_wallet)
        .await
        .unwrap();
    assert_eq!(wrapped_balance as u64, amount);

    let receipt = ctx
        .mint_erc_20_with_order(&john_wallet, &bft_bridge, mint_order)
        .await
        .unwrap();
    assert_eq!(receipt.status, Some(U64::zero()));

    let wrapped_balance = ctx
        .check_erc20_balance(&wrapped_token, &john_wallet)
        .await
        .unwrap();
    assert_eq!(wrapped_balance as u64, amount);
}

#[tokio::test]
async fn test_icrc2_tokens_roundtrip() {
    let (ctx, john_wallet, bft_bridge) = init_bridge().await;

    let minter_client = ctx.minter_client(JOHN);

    let base_token_id = Id256::from(&ctx.canisters().token_1());
    let wrapped_token = ctx
        .create_wrapped_token(&john_wallet, &bft_bridge, base_token_id)
        .await
        .unwrap();

    let amount = 300_000u64;
    let operation_id = 42;

    println!("burning icrc tokens and creating mint order");
    let mint_order = ctx
        .burn_icrc2(JOHN, &john_wallet, amount as _, operation_id)
        .await
        .unwrap();

    // lose mint order
    _ = mint_order;

    // get stored mint order from minter canister
    let sender_id = Id256::from(&john());
    let mint_orders = ctx
        .minter_client(JOHN)
        .list_mint_orders(sender_id, base_token_id)
        .await
        .unwrap();
    let (_, mint_order) = mint_orders
        .into_iter()
        .find(|(id, _)| *id == operation_id)
        .unwrap();

    ctx.mint_erc_20_with_order(&john_wallet, &bft_bridge, mint_order)
        .await
        .unwrap();

    let base_token_client = ctx.icrc_token_1_client(JOHN);
    let base_balance = base_token_client
        .icrc1_balance_of(john().into())
        .await
        .unwrap();

    let wrapped_balance = ctx
        .check_erc20_balance(&wrapped_token, &john_wallet)
        .await
        .unwrap();
    assert_eq!(
        base_balance,
        ICRC1_INITIAL_BALANCE - amount - ICRC1_TRANSFER_FEE * 2
    );
    assert_eq!(wrapped_balance as u64, amount);

    println!("burning wrapped token");
    let operation_id = ctx
        .burn_erc_20_tokens(
            &john_wallet,
            &wrapped_token,
            (&john()).into(),
            &bft_bridge,
            wrapped_balance,
        )
        .await
        .unwrap()
        .0;

    println!("minting icrc1 token");
    let john_address = john_wallet.address().into();
    let approved_amount = minter_client
        .start_icrc2_mint(&john_address, operation_id)
        .await
        .unwrap()
        .unwrap();

    println!("removing burn info");
    ctx.finish_burn(&john_wallet, operation_id, &bft_bridge)
        .await
        .unwrap();

    let approved_amount_without_fee = approved_amount.clone() - ICRC1_TRANSFER_FEE;
    minter_client
        .finish_icrc2_mint(
            operation_id,
            &john_address,
            ctx.canisters().token_1(),
            john(),
            approved_amount_without_fee,
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        approved_amount,
        wrapped_balance - ICRC1_TRANSFER_FEE as u128
    );

    let base_balance = base_token_client
        .icrc1_balance_of(john().into())
        .await
        .unwrap();
    let wrapped_balance = ctx
        .check_erc20_balance(&wrapped_token, &john_wallet)
        .await
        .unwrap();
    assert_eq!(base_balance, ICRC1_INITIAL_BALANCE - ICRC1_TRANSFER_FEE * 4);
    assert_eq!(wrapped_balance, 0);
}

#[tokio::test]
async fn test_icrc2_burn_by_different_users() {
    let (ctx, john_wallet, bft_bridge) = init_bridge().await;

    let alice_wallet = ctx.new_wallet(u128::MAX).await.unwrap();

    let base_token_id = Id256::from(&ctx.canisters().token_1());
    let _wrapped_token = ctx
        .create_wrapped_token(&john_wallet, &bft_bridge, base_token_id)
        .await
        .unwrap();

    let amount = 300_000u64;
    let operation_id = 42;
    let john_mint_order = ctx
        .burn_icrc2(JOHN, &john_wallet, amount as _, operation_id)
        .await
        .unwrap();
    let alice_mint_order = ctx
        .burn_icrc2(ALICE, &alice_wallet, amount as _, operation_id)
        .await
        .unwrap();

    ctx.mint_erc_20_with_order(&john_wallet, &bft_bridge, john_mint_order)
        .await
        .unwrap();
    ctx.mint_erc_20_with_order(&alice_wallet, &bft_bridge, alice_mint_order)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_user_should_not_transfer_icrc_if_erc20_burn_not_finished() {
    let (ctx, john_wallet, bft_bridge) = init_bridge().await;

    let base_token_id = Id256::from(&ctx.canisters().token_1());
    let wrapped_token = ctx
        .create_wrapped_token(&john_wallet, &bft_bridge, base_token_id)
        .await
        .unwrap();

    let amount = 300_000u64;
    let operation_id = 42;

    println!("burning icrc tokens and creating mint order");
    let mint_order = ctx
        .burn_icrc2(JOHN, &john_wallet, amount as _, operation_id)
        .await
        .unwrap();

    println!("minting erc20");
    ctx.mint_erc_20_with_order(&john_wallet, &bft_bridge, mint_order)
        .await
        .unwrap();

    println!("burning wrapped token");
    let operation_id = ctx
        .burn_erc_20_tokens(
            &john_wallet,
            &wrapped_token,
            (&john()).into(),
            &bft_bridge,
            amount as _,
        )
        .await
        .unwrap()
        .0;

    println!("minting icrc1 token");
    let john_address = john_wallet.address().into();
    let minter_client = ctx.minter_client(JOHN);
    let approved_amount = minter_client
        .start_icrc2_mint(&john_address, operation_id)
        .await
        .unwrap()
        .unwrap();

    // Here user skips the BFTBridge::finish_burn() step...

    let approved_amount_without_fee = approved_amount.clone() - ICRC1_TRANSFER_FEE;
    let err = minter_client
        .finish_icrc2_mint(
            operation_id,
            &john_address,
            ctx.canisters().token_1(),
            john(),
            approved_amount_without_fee,
        )
        .await
        .unwrap()
        .unwrap_err();

    assert!(matches!(err, McError::InvalidBurnOperation(_)));
}

#[tokio::test]
async fn test_icrc2_forbid_double_spend() {
    let (ctx, john_wallet, bft_bridge) = init_bridge().await;

    let base_token_id = Id256::from(&ctx.canisters().token_1());
    let wrapped_token = ctx
        .create_wrapped_token(&john_wallet, &bft_bridge, base_token_id)
        .await
        .unwrap();

    let amount = 300_000u64;
    let operation_id = 42;
    let mint_order = ctx
        .burn_icrc2(JOHN, &john_wallet, amount as _, operation_id)
        .await
        .unwrap();

    ctx.mint_erc_20_with_order(&john_wallet, &bft_bridge, mint_order)
        .await
        .unwrap();

    let operation_id = ctx
        .burn_erc_20_tokens(
            &john_wallet,
            &wrapped_token,
            (&john()).into(),
            &bft_bridge,
            amount as _,
        )
        .await
        .unwrap()
        .0;

    println!("minting icrc1 token");
    let john_address = john_wallet.address().into();
    let minter_client = ctx.minter_client(JOHN);
    let approved_amount = minter_client
        .start_icrc2_mint(&john_address, operation_id)
        .await
        .unwrap()
        .unwrap();

    println!("removing burn info");
    ctx.finish_burn(&john_wallet, operation_id, &bft_bridge)
        .await
        .unwrap();

    let approved_amount_without_fee = approved_amount.clone() - ICRC1_TRANSFER_FEE;
    minter_client
        .finish_icrc2_mint(
            operation_id,
            &john_address,
            ctx.canisters().token_1(),
            john(),
            approved_amount_without_fee.clone(),
        )
        .await
        .unwrap()
        .unwrap();

    // Trying to transfer ICRC-2 twice...
    let err = minter_client
        .finish_icrc2_mint(
            operation_id,
            &john_address,
            ctx.canisters().token_1(),
            john(),
            approved_amount_without_fee,
        )
        .await
        .unwrap()
        .unwrap_err();
    assert!(matches!(
        err,
        McError::Icrc2TransferFromError(TransferFromError::InsufficientAllowance { .. })
    ));

    // Trying to use the same ERC-20 burn to mint ICRC-2 again...
    let err = minter_client
        .start_icrc2_mint(&john_address, operation_id)
        .await
        .unwrap()
        .unwrap_err();
    assert!(matches!(err, McError::InvalidBurnOperation(_)));
}

#[tokio::test]
async fn test_icrc2_forbid_unexisting_token_mint() {
    let (ctx, john_wallet, bft_bridge) = init_bridge().await;

    // Skip wrapped token creation step

    let amount = 300_000u64;
    let operation_id = 42;
    let mint_order = ctx
        .burn_icrc2(JOHN, &john_wallet, amount as _, operation_id)
        .await
        .unwrap();

    let receipt = ctx
        .mint_erc_20_with_order(&john_wallet, &bft_bridge, mint_order)
        .await
        .unwrap();
    assert_eq!(receipt.status, Some(U64::zero()));
}

#[tokio::test]
async fn spender_canister_access_control() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Spender]).await;
    let spender_client = ctx.client(ctx.canisters().spender(), JOHN);

    let dst_info = IcrcTransferDst {
        token: Principal::anonymous(),
        recipient: Principal::anonymous(),
    };

    let amount = Nat::default();
    spender_client
        .update::<_, TransferFromError>(
            "finish_icrc2_mint",
            (&dst_info, &[0u8; 32], &amount, &amount),
        )
        .await
        .unwrap_err();
}

#[tokio::test]
async fn canister_log_config_should_still_be_storable_after_upgrade() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Minter]).await;

    let minter_client = ctx.minter_client(ADMIN);

    assert!(minter_client
        .set_logger_filter("info".to_string())
        .await
        .unwrap()
        .is_ok());

    // Advance state to avoid canister rate limit.
    for _ in 0..100 {
        ctx.client.tick().await;
    }

    // upgrade canister
    ctx.upgrade_minter_canister().await.unwrap();
    assert!(minter_client
        .set_logger_filter("debug".to_string())
        .await
        .unwrap()
        .is_ok());
}

// Create a second EVM canister (external_evm) instance and create BFTBridge contract on it, // It will play role of external evm
// Create evm-minter instance, initialized with EvmInfos for both EVM canisters.
// Deploy ERC-20 token on external_evm,
// Deploy Wrapped token on first EVM for the ERC-20 from previous step,
// Approve ERC-20 transfer on behalf of some user in external_evm,
// Call BFTBridge::burn() on behalf of the user in external_evm.
// Wait some time for the evm-minter see and process it.
// Query SignedMintOrder from the evm-minter. // Endpoint for it is In progress now.
// Send the SignedMintOrder to the BFTBridge::mint() endpoint of the first EVM.
// Make sure the tokens minted.
// Make sure SignedMintOrder removed from evm-minter after some time.

#[tokio::test]
async fn test_external_bridging() {
    let (ctx, _john_wallet, _bft_bridge) = init_bridge().await;

    let minter_client = ctx.minter_client(ADMIN);

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

    let minter_canister_address = ctx
        .get_minter_canister_evm_address(ctx.admin_name())
        .await
        .unwrap();

    // Deploy the BFTBridge contract on the external EVM.
    let contract = BFT_BRIDGE_SMART_CONTRACT_CODE.clone();
    let input = bft_bridge_api::CONSTRUCTOR
        .encode_input(contract, &[Token::Address(minter_canister_address.into())])
        .unwrap();

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
        wallet
    };

    let bob_address: H160 = bob_wallet.address().into();
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

    // Initialize evm-minter with EvmInfos for both EVM canisters.
    let _evm_minter = ctx
        .deploy_canister(
            CanisterType::EvmMinter,
            (Settings {
                base_evm_link: EvmLink::Ic(ctx.canisters().evm()),
                wrapped_evm_link: EvmLink::Ic(external_evm),
                signing_strategy: SigningStrategy::Local {
                    private_key: [42; 32],
                },
                log_settings: Some(LogSettings {
                    enable_console: true,
                    in_memory_records: None,
                    log_filter: Some("info".to_string()),
                }),
            },),
        )
        .await;

    // Deploy ERC-20 token on external EVM.
    let erc20_token_bytecode = &get_solidity_smart_contracts()
        .get("TestWTM")
        .unwrap()
        .bytecode;

    let data: Constructor = Constructor {
        inputs: vec![Param {
            name: "initialSupply".into(),
            kind: ParamType::Uint(256),
            internal_type: None,
        }],
    };

    let data = data
        .encode_input(
            erc20_token_bytecode.to_vec(),
            &[Token::Uint(100_u64.pow(24).into())],
        )
        .unwrap();

    let tx = ctx.signed_transaction(&bob_wallet, None, nonce, 0, data);
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
    let token_id = Id256::from_evm_address(&erc20_token_address, 355113);
    let wrapped_token = ctx
        .create_wrapped_token(
            &ctx.new_wallet(9_000_000).await.unwrap(),
            &external_bridge_address,
            token_id,
        )
        .await
        .unwrap();

    // Approve ERC-20 transfer on behalf of some user in external EVM.
    let alice_wallet = ctx.new_wallet(u128::MAX).await.unwrap();
    let alice_address: H160 = alice_wallet.address().into();

    let nonce = external_evm_client
        .account_basic(alice_address.clone())
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
        Token::Address(alice_address.clone().into()),
        Token::Uint(10_u64.pow(18).into()),
    ])
    .unwrap();

    let approve_tx = ctx.signed_transaction(
        &bob_wallet,
        Some(erc20_token_address.clone()),
        nonce.clone(),
        0,
        data,
    );

    let hash = external_evm_client
        .send_raw_transaction(approve_tx)
        .await
        .unwrap()
        .unwrap();

    let receipt = ctx
        .wait_transaction_receipt_on_evm(&external_evm_client, &hash)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(receipt.status, Some(U64::one()));

    // Call BFTBridge::burn() on behalf of the user in external EVM.
    let nonce = external_evm_client
        .account_basic(alice_address.clone())
        .await
        .unwrap()
        .nonce;

    let alice_id = Id256::from_evm_address(&alice_address, 355113);

    let input = bft_bridge_api::BURN
        .encode_input(&[
            Token::Uint(10_u64.pow(18).into()),
            Token::Address(erc20_token_address.into()),
            Token::FixedBytes(alice_id.0.to_vec()),
        ])
        .unwrap();

    let burn_tx = ctx.signed_transaction(
        &bob_wallet,
        Some(external_bridge_address.clone()),
        nonce.clone(),
        0,
        input,
    );

    let hash = external_evm_client
        .send_raw_transaction(burn_tx)
        .await
        .unwrap()
        .unwrap();

    // Tick to advance time.
    ctx.advance_time(Duration::from_secs(10)).await;

    // Wait some time for the evm-minter see and process it.
    let receipt = ctx.wait_transaction_receipt(&hash).await.unwrap().unwrap();

    assert_eq!(receipt.status, Some(U64::one()));

    // Query SignedMintOrder from the evm-minter.
    // TODO: Endpoint for it is In progress now.
    let signed_order = SignedMintOrder([5; 262]); // Example

    // Send the SignedMintOrder to the BFTBridge::mint() endpoint of the first EVM.
    let nonce = external_evm_client
        .account_basic(bob_address.clone())
        .await
        .unwrap()
        .nonce;

    let input = bft_bridge_api::MINT
        .encode_input(&[Token::FixedBytes(signed_order.0.to_vec())])
        .unwrap();

    let mint_tx = ctx.signed_transaction(
        &bob_wallet,
        Some(external_bridge_address.clone()),
        nonce.clone(),
        0,
        input,
    );

    let hash = external_evm_client
        .send_raw_transaction(mint_tx)
        .await
        .unwrap()
        .unwrap();

    // Tick to advance time.
    ctx.advance_time(Duration::from_secs(10)).await;

    // Make sure the tokens minted.
    let receipt = ctx.wait_transaction_receipt(&hash).await.unwrap().unwrap();

    assert_eq!(receipt.status, Some(U64::one()));

    // Chech the balance of the wrapped token.
    let data = utils::function_selector(
        "balanceOf",
        &[Param {
            name: "account".to_string(),
            kind: ethers_core::abi::ParamType::Address,
            internal_type: None,
        }],
    )
    .encode_input(&[Token::Address(alice_address.into())])
    .unwrap();

    let balance = external_evm_client
        .eth_call(
            Some(bob_address),
            Some(wrapped_token.clone()),
            None,
            3_000_000,
            None,
            Some(data.into()),
        )
        .await
        .unwrap()
        .unwrap();

    assert_eq!(
        10_u64.pow(18),
        U256::from_hex_str(&balance).unwrap().0.as_u64()
    );
}
