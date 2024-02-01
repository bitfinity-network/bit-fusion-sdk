use candid::{Nat, Principal};
use did::{H160, U64};
use eth_signer::{Signer, Wallet};
use ethers_core::abi::Token;
use ethers_core::k256::ecdsa::SigningKey;
use ic_exports::ic_kit::mock_principals::{alice, john};
use ic_exports::icrc_types::icrc2::transfer_from::TransferFromError;
use minter_canister::tokens::icrc1::IcrcTransferDst;
use minter_contract_utils::build_data::WRAPPED_TOKEN_SMART_CONTRACT_CODE;
use minter_contract_utils::wrapped_token_api;
use minter_did::error::Error as McError;
use minter_did::id256::Id256;

use super::{PocketIcTestContext, JOHN};
use crate::context::{CanisterType, TestContext, ICRC1_INITIAL_BALANCE, ICRC1_TRANSFER_FEE};
use crate::pocket_ic_integration_test::{ADMIN, ALICE};
use crate::utils::error::TestError;

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

#[tokio::test]
#[should_panic(expected = "JoinError")]
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

#[tokio::test]
#[should_panic(expected = "JoinError")]
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
