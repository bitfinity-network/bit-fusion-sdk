use std::time::Duration;

use did::keccak::keccak_hash;
use did::{H160, U256};
use eth_signer::Signer;
use ethers_core::abi::Token;
use ic_canister_client::CanisterClientError;
use ic_exports::ic_kit::mock_principals::{alice, john};
use ic_exports::pocket_ic::{CallError, ErrorCode, UserError};
use minter_contract_utils::wrapped_token_api::ERC_20_ALLOWANCE;
use minter_did::id256::Id256;
use minter_did::reason::ApproveMintedTokens;

use super::{init_bridge, PocketIcTestContext, JOHN};
use crate::context::{
    CanisterType, TestContext, DEFAULT_GAS_PRICE, ICRC1_INITIAL_BALANCE, ICRC1_TRANSFER_FEE,
};
use crate::pocket_ic_integration_test::{ADMIN, ALICE};

#[tokio::test]
async fn test_icrc2_tokens_roundtrip() {
    let (ctx, john_wallet, bft_bridge) = init_bridge().await;

    let base_token_id = Id256::from(&ctx.canisters().token_1());
    let wrapped_token = ctx
        .create_wrapped_token(&john_wallet, &bft_bridge, base_token_id)
        .await
        .unwrap();

    let amount = 300_000u64;
    let operation_id = 42;

    let evm_client = ctx.evm_client(ADMIN);
    let john_principal_id = Id256::from(&john());
    let native_token_amount = 10_u64.pow(17);
    ctx.native_token_deposit(
        &evm_client,
        bft_bridge.clone(),
        &john_wallet,
        &[john_principal_id],
        native_token_amount.into(),
    )
    .await
    .unwrap();

    let john_address: H160 = john_wallet.address().into();

    eprintln!("burning icrc tokens and creating mint order");
    let _operation_id = ctx
        .burn_icrc2(
            JOHN,
            &john_wallet,
            amount as _,
            operation_id,
            None,
            Some(john_address),
        )
        .await
        .unwrap();

    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;

    let base_token_client = ctx.icrc_token_1_client(JOHN);
    let base_balance = base_token_client
        .icrc1_balance_of(john().into())
        .await
        .unwrap();

    eprintln!("checking wrapped token balance");
    let wrapped_balance = ctx
        .check_erc20_balance(&wrapped_token, &john_wallet)
        .await
        .unwrap();
    assert_eq!(
        base_balance,
        ICRC1_INITIAL_BALANCE - amount - ICRC1_TRANSFER_FEE * 2
    );
    assert_eq!(wrapped_balance as u64, amount);

    let _operation_id = ctx
        .burn_erc_20_tokens(
            &ctx.evm_client(ADMIN),
            &john_wallet,
            &wrapped_token,
            (&john()).into(),
            &bft_bridge,
            wrapped_balance,
        )
        .await
        .unwrap()
        .0;

    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;

    println!("john principal: {}", john());

    let base_balance = base_token_client
        .icrc1_balance_of(john().into())
        .await
        .unwrap();
    let wrapped_balance = ctx
        .check_erc20_balance(&wrapped_token, &john_wallet)
        .await
        .unwrap();
    assert_eq!(wrapped_balance, 0);
    assert_eq!(base_balance, ICRC1_INITIAL_BALANCE - ICRC1_TRANSFER_FEE * 3);
}

#[tokio::test]
async fn test_icrc2_token_canister_stopped() {
    let (ctx, john_wallet, bft_bridge) = init_bridge().await;

    let base_token_id = Id256::from(&ctx.canisters().token_1());
    let wrapped_token = ctx
        .create_wrapped_token(&john_wallet, &bft_bridge, base_token_id)
        .await
        .unwrap();

    let amount = 3_000_000u64;

    let evm_client = ctx.evm_client(ADMIN);
    let john_principal_id = Id256::from(&john());
    let native_token_amount = 10_u64.pow(17);
    ctx.native_token_deposit(
        &evm_client,
        bft_bridge.clone(),
        &john_wallet,
        &[john_principal_id],
        native_token_amount.into(),
    )
    .await
    .unwrap();

    eprintln!("burning icrc tokens and creating mint order");
    let john_address: H160 = john_wallet.address().into();
    let _operation_id = ctx
        .burn_icrc2(
            JOHN,
            &john_wallet,
            amount as _,
            42,
            None,
            Some(john_address),
        )
        .await
        .unwrap();

    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;

    let base_token_client = ctx.icrc_token_1_client(JOHN);
    let base_balance = base_token_client
        .icrc1_balance_of(john().into())
        .await
        .unwrap();

    eprintln!("checking wrapped token balance");
    let wrapped_balance = ctx
        .check_erc20_balance(&wrapped_token, &john_wallet)
        .await
        .unwrap();
    assert_eq!(
        base_balance,
        ICRC1_INITIAL_BALANCE - amount - ICRC1_TRANSFER_FEE * 2
    );
    assert_eq!(wrapped_balance as u64, amount);

    ctx.client
        .stop_canister(ctx.canisters().token_1(), Some(ctx.admin()))
        .await
        .unwrap();

    let john_principal_id256 = Id256::from(&john());
    let _operation_id = ctx
        .burn_erc_20_tokens(
            &ctx.evm_client(ADMIN),
            &john_wallet,
            &wrapped_token,
            john_principal_id256,
            &bft_bridge,
            wrapped_balance,
        )
        .await
        .unwrap()
        .0;

    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;

    let refund_mint_order = ctx
        .minter_client(ADMIN)
        .list_mint_orders(john_principal_id256, base_token_id)
        .await
        .unwrap()[0]
        .1;

    ctx.mint_erc_20_with_order(&john_wallet, &bft_bridge, refund_mint_order)
        .await
        .unwrap();

    ctx.client
        .start_canister(ctx.canisters().token_1(), Some(ctx.admin()))
        .await
        .unwrap();

    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;

    // Check if the amount is refunded as wrapped token.
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
        ICRC1_INITIAL_BALANCE - ICRC1_TRANSFER_FEE * 2 - amount
    );
    assert_eq!(wrapped_balance as u64, amount);
}

#[tokio::test]
async fn test_icrc2_tokens_approve_after_mint() {
    let (ctx, john_wallet, bft_bridge) = init_bridge().await;

    let base_token_id = Id256::from(&ctx.canisters().token_1());
    let wrapped_token = ctx
        .create_wrapped_token(&john_wallet, &bft_bridge, base_token_id)
        .await
        .unwrap();

    let amount = 300_000u64;
    let operation_id = 42;

    let john_principal_hash = keccak_hash(john().as_slice());
    let principal_signature = john_wallet.sign_hash(john_principal_hash.0).unwrap().into();
    let approve_amount = U256::from(1000_u64);

    eprintln!("burning icrc tokens and creating mint order");
    let john_address: H160 = john_wallet.address().into();
    let spender_wallet = ctx.new_wallet(0).await.unwrap();

    let evm_client = ctx.evm_client(ADMIN);
    let john_principal_id = Id256::from(&john());
    let native_token_amount = 10_u64.pow(17);
    ctx.native_token_deposit(
        &evm_client,
        bft_bridge.clone(),
        &john_wallet,
        &[john_principal_id],
        native_token_amount.into(),
    )
    .await
    .unwrap();

    println!("John address: {john_address:?}");

    let native_deposit_balance = ctx.native_token_deposit_balance(&evm_client, bft_bridge.clone(), john_address.clone()).await;
    assert_eq!(native_deposit_balance, native_token_amount.into());

    let _operation_id = ctx
        .burn_icrc2(
            JOHN,
            &john_wallet,
            amount as _,
            operation_id,
            Some(ApproveMintedTokens {
                approve_spender: spender_wallet.address().into(),
                approve_amount: approve_amount.clone(),
                principal_signature,
            }),
            Some(john_address.clone()),
        )
        .await
        .unwrap();

    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;

    let base_token_client = ctx.icrc_token_1_client(JOHN);
    let base_balance = base_token_client
        .icrc1_balance_of(john().into())
        .await
        .unwrap();

    eprintln!("checking wrapped token balance");
    let wrapped_balance = ctx
        .check_erc20_balance(&wrapped_token, &john_wallet)
        .await
        .unwrap();
    assert_eq!(
        base_balance,
        ICRC1_INITIAL_BALANCE - amount - ICRC1_TRANSFER_FEE * 2
    );
    assert_eq!(wrapped_balance as u64, amount);

    let input = ERC_20_ALLOWANCE
        .encode_input(&[
            Token::Address(john_address.0),
            Token::Address(spender_wallet.address()),
        ])
        .unwrap();
    let allowance_response = ctx
        .evm_client(ADMIN)
        .eth_call(
            Some(john_address),
            Some(wrapped_token),
            Some(0u64.into()),
            3_000_000,
            Some(DEFAULT_GAS_PRICE.into()),
            Some(input.into()),
        )
        .await
        .unwrap()
        .unwrap();

    let allowance_data = hex::decode(allowance_response.trim_start_matches("0x")).unwrap();
    let allowance = ERC_20_ALLOWANCE
        .decode_output(&allowance_data)
        .unwrap()
        .first()
        .unwrap()
        .clone()
        .into_uint()
        .unwrap();

    assert_eq!(allowance, approve_amount.0);
}

#[tokio::test]
async fn set_owner_access() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Icrc2Minter]).await;
    let mut admin_client = ctx.minter_client(ADMIN);
    admin_client.set_owner(alice()).await.unwrap().unwrap();

    // Now Alice is owner, so admin can't update owner anymore.
    let err = admin_client.set_owner(alice()).await.unwrap_err();
    assert!(matches!(
        err,
        CanisterClientError::PocketIcTestError(CallError::UserError(UserError {
            code: ErrorCode::CanisterCalledTrap,
            description: _,
        }))
    ));

    // Now Alice is owner, so she can update owner.
    let mut alice_client = ctx.minter_client(ALICE);
    alice_client.set_owner(alice()).await.unwrap().unwrap();
}

#[tokio::test]
async fn double_register_bridge() {
    let ctx = PocketIcTestContext::new(&CanisterType::ICRC2_MINTER_TEST_SET).await;

    let _ = ctx.initialize_bft_bridge(ADMIN).await.unwrap();

    ctx.advance_time(Duration::from_secs(2)).await;
    ctx.advance_time(Duration::from_secs(2)).await;

    let err = ctx.initialize_bft_bridge(ADMIN).await.unwrap_err();

    assert!(err
        .to_string()
        .contains("creation of BftBridge contract already finised"));
}

#[tokio::test]
async fn canister_log_config_should_still_be_storable_after_upgrade() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Icrc2Minter]).await;

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

#[tokio::test]
async fn test_canister_build_data() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Icrc2Minter]).await;
    let minter_client = ctx.minter_client(ALICE);
    let build_data = minter_client.get_canister_build_data().await.unwrap();
    assert!(build_data.pkg_name.contains("icrc2-minter"));
}
