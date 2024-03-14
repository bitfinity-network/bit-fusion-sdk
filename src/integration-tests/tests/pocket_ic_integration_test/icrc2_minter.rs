use candid::{Nat, Principal};
use did::H160;
use ethers_core::abi::Token;
use ic_canister_client::CanisterClientError;
use ic_exports::ic_kit::mock_principals::{alice, john};
use ic_exports::icrc_types::icrc2::transfer_from::TransferFromError;
use ic_exports::pocket_ic::{CallError, ErrorCode, UserError};
use icrc2_minter::tokens::icrc1::IcrcTransferDst;
use minter_contract_utils::build_data::test_contracts::WRAPPED_TOKEN_SMART_CONTRACT_CODE;
use minter_contract_utils::wrapped_token_api;
use minter_did::error::Error as McError;
use minter_did::id256::Id256;

use super::{init_bridge, PocketIcTestContext, JOHN};
use crate::context::{CanisterType, TestContext, ICRC1_INITIAL_BALANCE, ICRC1_TRANSFER_FEE};
use crate::pocket_ic_integration_test::{ADMIN, ALICE};
use crate::utils::error::TestError;

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

    println!("burning icrc tokens and creating mint order");
    let _operation_id = ctx
        .burn_icrc2(JOHN, &john_wallet, amount as _, operation_id)
        .await
        .unwrap();

    ctx.advance_time(Duration::from_secs(2)).await;

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

    let _operation_id = ctx
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
    assert_eq!(base_balance, ICRC1_INITIAL_BALANCE - ICRC1_TRANSFER_FEE * 4);
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
async fn set_owner_access() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Minter]).await;
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

#[tokio::test]
async fn double_register_bridge() {
    let ctx = PocketIcTestContext::new(&CanisterType::MINTER_TEST_SET).await;
    let admin_wallet = ctx.new_wallet(u128::MAX).await.unwrap();

    let _ = ctx
        .initialize_bft_bridge(ADMIN, &admin_wallet)
        .await
        .unwrap();
    let err = ctx
        .initialize_bft_bridge(ADMIN, &admin_wallet)
        .await
        .unwrap_err();

    assert!(matches!(
        err,
        TestError::CanisterClient(CanisterClientError::PocketIcTestError(
            CallError::UserError(UserError {
                code: ErrorCode::CanisterCalledTrap,
                description: _,
            })
        ))
    ));
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

#[tokio::test]
async fn test_canister_build_data() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Minter]).await;
    let minter_client = ctx.minter_client(ALICE);
    let build_data = minter_client.get_canister_build_data().await.unwrap();
    assert!(build_data.pkg_name.contains("icrc2-minter"));
}
