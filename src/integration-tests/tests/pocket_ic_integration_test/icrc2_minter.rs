use std::time::Duration;

use alloy_sol_types::SolCall;
use bridge_client::BridgeCanisterClient;
use bridge_did::id256::Id256;
use bridge_did::reason::ApproveAfterMint;
use bridge_utils::WrappedToken;
use did::{H160, U256, U64};
use eth_signer::Signer;
use ic_canister_client::CanisterClientError;
use ic_exports::ic_kit::mock_principals::{alice, john};
use ic_exports::pocket_ic::{CallError, ErrorCode, UserError};

use super::{init_bridge, PocketIcTestContext, JOHN};
use crate::context::{
    CanisterType, TestContext, DEFAULT_GAS_PRICE, ICRC1_INITIAL_BALANCE, ICRC1_TRANSFER_FEE,
};
use crate::pocket_ic_integration_test::{ADMIN, ALICE};

#[tokio::test]
async fn test_icrc2_tokens_roundtrip() {
    let (ctx, john_wallet, bft_bridge, fee_charge) = init_bridge().await;

    let minter_client = ctx.icrc_minter_client(ADMIN);
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
        &wrapped_token,
        amount as _,
        Some(john_address),
        None,
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
        .check_erc20_balance(&wrapped_token, &john_wallet, None)
        .await
        .unwrap();
    assert_eq!(
        base_balance,
        ICRC1_INITIAL_BALANCE - amount - ICRC1_TRANSFER_FEE * 2
    );
    assert_eq!(wrapped_balance as u64, amount);

    let _operation_id = ctx
        .burn_wrapped_erc_20_tokens(
            &ctx.evm_client(ADMIN),
            &john_wallet,
            &wrapped_token,
            base_token_id.0.as_slice(),
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
        .check_erc20_balance(&wrapped_token, &john_wallet, None)
        .await
        .unwrap();
    assert_eq!(wrapped_balance, 0);
    assert_eq!(base_balance, ICRC1_INITIAL_BALANCE - ICRC1_TRANSFER_FEE * 3);
}

#[tokio::test]
async fn test_icrc2_token_canister_stopped() {
    let (ctx, john_wallet, bft_bridge, fee_charge) = init_bridge().await;

    let minter_client = ctx.icrc_minter_client(ADMIN);
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
        &wrapped_token,
        amount as _,
        Some(john_address.clone()),
        None,
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
        .check_erc20_balance(&wrapped_token, &john_wallet, None)
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
        .burn_wrapped_erc_20_tokens(
            &ctx.evm_client(ADMIN),
            &john_wallet,
            &wrapped_token,
            base_token_id.0.as_slice(),
            john_principal_id256,
            &bft_bridge,
            wrapped_balance,
        )
        .await
        .unwrap()
        .0;

    ctx.advance_by_times(Duration::from_secs(2), 20).await;

    let minter_client = ctx.icrc_minter_client(ADMIN);
    let (_, refund_mint_order) = minter_client
        .list_mint_orders(&john_address, &base_token_id, Some(0u64), Some(1024u64))
        .await
        .unwrap()[0];

    let receipt = ctx
        .mint_erc_20_with_order(&john_wallet, &bft_bridge, refund_mint_order)
        .await
        .unwrap();

    assert_eq!(
        receipt.status,
        Some(U64::one()),
        "Refund transaction failed: {}",
        String::from_utf8_lossy(&receipt.output.unwrap_or_default()),
    );

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
        .check_erc20_balance(&wrapped_token, &john_wallet, None)
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
    admin_client.set_owner(alice()).await.unwrap();

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
    alice_client.set_owner(alice()).await.unwrap();
}

#[tokio::test]
async fn canister_log_config_should_still_be_storable_after_upgrade() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Icrc2Minter]).await;

    let minter_client = ctx.icrc_minter_client(ADMIN);

    minter_client
        .set_logger_filter("info".to_string())
        .await
        .unwrap()
        .unwrap();

    // Advance state to avoid canister rate limit.
    for _ in 0..100 {
        ctx.client.tick().await;
    }

    // upgrade canister
    ctx.upgrade_minter_canister().await.unwrap();
    minter_client
        .set_logger_filter("debug".to_string())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn test_canister_build_data() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Icrc2Minter]).await;
    let minter_client = ctx.icrc_minter_client(ALICE);
    let build_data = minter_client.get_canister_build_data().await.unwrap();
    assert!(build_data.pkg_name.contains("icrc2-minter"));
}

#[tokio::test]
async fn test_icrc2_tokens_approve_after_mint() {
    let (ctx, john_wallet, bft_bridge, fee_charge) = init_bridge().await;

    let base_token_id = Id256::from(&ctx.canisters().token_1());
    let wrapped_token = ctx
        .create_wrapped_token(&john_wallet, &bft_bridge, base_token_id)
        .await
        .unwrap();

    let amount = 300_000u64;
    let approve_amount = U256::from(1000_u64);

    eprintln!("burning icrc tokens and creating mint order");
    let john_address: H160 = john_wallet.address().into();
    let spender_wallet = ctx.new_wallet(0).await.unwrap();

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

    println!("John address: {john_address:?}");

    let native_deposit_balance = ctx
        .native_token_deposit_balance(&evm_client, fee_charge.clone(), john_address.clone())
        .await;
    assert_eq!(native_deposit_balance, native_token_amount.into());

    ctx.burn_icrc2(
        JOHN,
        &john_wallet,
        &bft_bridge,
        &wrapped_token,
        amount as _,
        Some(john_address.clone()),
        Some(ApproveAfterMint {
            approve_spender: spender_wallet.address().into(),
            approve_amount: approve_amount.clone(),
        }),
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
        .check_erc20_balance(&wrapped_token, &john_wallet, None)
        .await
        .unwrap();
    assert_eq!(
        base_balance,
        ICRC1_INITIAL_BALANCE - amount - ICRC1_TRANSFER_FEE * 2
    );
    assert_eq!(wrapped_balance as u64, amount);

    let input = WrappedToken::allowanceCall {
        owner: john_address.clone().into(),
        spender: spender_wallet.address().0.into(),
    }
    .abi_encode();

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

    let allowance: U256 = WrappedToken::allowanceCall::abi_decode_returns(&allowance_data, true)
        .unwrap()
        ._0
        .into();

    assert_eq!(allowance, approve_amount);
}
