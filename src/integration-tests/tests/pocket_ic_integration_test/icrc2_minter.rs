use std::time::Duration;

use did::H160;
use eth_signer::Signer;
use ic_canister_client::CanisterClientError;
use ic_exports::ic_kit::mock_principals::{alice, john};
use ic_exports::pocket_ic::{CallError, ErrorCode, UserError};
use minter_did::id256::Id256;

use super::{init_bridge, PocketIcTestContext, JOHN};
use crate::context::{CanisterType, TestContext, ICRC1_INITIAL_BALANCE, ICRC1_TRANSFER_FEE};
use crate::pocket_ic_integration_test::{ADMIN, ALICE};

#[tokio::test]
async fn test_icrc2_tokens_roundtrip() {
    let (ctx, john_wallet, bft_bridge, fee_charge) = init_bridge().await;

    let minter_client = ctx.minter_client(ADMIN);
    minter_client
        .add_to_whitelist(ctx.canisters().token_1())
        .await
        .unwrap()
        .unwrap();

    let base_token_id = Id256::from(&ctx.canisters().token_1());
    let wrapped_token = ctx
        .create_wrapped_token(&john_wallet, &bft_bridge, base_token_id)
        .await
        .unwrap();

    let amount = 300_000u64;

    let evm_client = ctx.evm_client(ADMIN);
    let john_principal_id = Id256::from(&john());
    let native_token_amount = 10_u64.pow(17);
    ctx.native_token_deposit(
        &evm_client,
        fee_charge.clone(),
        &john_wallet,
        &[john_principal_id],
        native_token_amount.into(),
    )
    .await
    .unwrap();

    let john_address: H160 = john_wallet.address().into();

    eprintln!("burning icrc tokens and creating mint order");
    ctx.burn_icrc2(
        JOHN,
        &john_wallet,
        &bft_bridge,
        amount as _,
        Some(john_address),
    )
    .await
    .unwrap();

    ctx.advance_by_times(Duration::from_secs(2), 10).await;

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

    ctx.advance_by_times(Duration::from_secs(2), 4).await;

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
    let (ctx, john_wallet, bft_bridge, fee_charge) = init_bridge().await;

    let minter_client = ctx.minter_client(ADMIN);
    minter_client
        .add_to_whitelist(ctx.canisters().token_1())
        .await
        .unwrap()
        .unwrap();
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
        fee_charge.clone(),
        &john_wallet,
        &[john_principal_id],
        native_token_amount.into(),
    )
    .await
    .unwrap();

    eprintln!("burning icrc tokens and creating mint order");
    let john_address: H160 = john_wallet.address().into();
    ctx.burn_icrc2(
        JOHN,
        &john_wallet,
        &bft_bridge,
        amount as _,
        Some(john_address),
    )
    .await
    .unwrap();

    ctx.advance_by_times(Duration::from_secs(2), 8).await;

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

    ctx.advance_by_times(Duration::from_secs(2), 8).await;

    let refund_mint_order = ctx
        .icrc_minter_client(ADMIN)
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

    ctx.advance_by_times(Duration::from_secs(2), 10).await;

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
async fn set_owner_access() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Icrc2Minter]).await;
    let mut admin_client = ctx.icrc_minter_client(ADMIN);
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
    let mut alice_client = ctx.icrc_minter_client(ALICE);
    alice_client.set_owner(alice()).await.unwrap().unwrap();
}

#[tokio::test]
async fn double_register_bridge() {
    let ctx = PocketIcTestContext::new(&CanisterType::ICRC2_MINTER_TEST_SET).await;

    let _ = ctx
        .initialize_bft_bridge(ADMIN, H160::default())
        .await
        .unwrap();

    ctx.advance_by_times(Duration::from_secs(2), 2).await;

    let err = ctx
        .initialize_bft_bridge(ADMIN, H160::default())
        .await
        .unwrap_err();

    assert!(err
        .to_string()
        .contains("creation of BftBridge contract already finished"));
}

#[tokio::test]
async fn canister_log_config_should_still_be_storable_after_upgrade() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Icrc2Minter]).await;

    let minter_client = ctx.icrc_minter_client(ADMIN);

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
    let minter_client = ctx.icrc_minter_client(ALICE);
    let build_data = minter_client.get_canister_build_data().await.unwrap();
    assert!(build_data.pkg_name.contains("icrc2-minter"));
}
