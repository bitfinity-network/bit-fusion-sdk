use std::sync::Arc;
use std::time::Duration;

use alloy_sol_types::SolCall;
use bridge_canister::bridge::Operation;
use bridge_client::BridgeCanisterClient;
use bridge_did::id256::Id256;
use bridge_did::operations::IcrcBridgeOp;
use bridge_did::reason::ApproveAfterMint;
use bridge_utils::WrappedToken;
use did::{H160, U64, U256};
use eth_signer::LocalWallet;
use ic_canister_client::CanisterClientError;
use ic_exports::ic_kit::mock_principals::{alice, john};
use ic_exports::pocket_ic::RejectResponse;
use icrc2_bridge::ops::IcrcBridgeOpImpl;
use tokio::sync::Semaphore;
use tokio_util::sync::CancellationToken;

use super::{JOHN, PocketIcTestContext};
use crate::context::stress::{StressTestConfig, icrc};
use crate::context::{
    CanisterType, DEFAULT_GAS_PRICE, ICRC1_INITIAL_BALANCE, ICRC1_TRANSFER_FEE, TestContext,
};
use crate::pocket_ic_integration_test::{ADMIN, ALICE, block_until_succeeds};
use crate::utils::TestEvm;

#[tokio::test]
async fn test_icrc2_tokens_roundtrip() {
    let (ctx, john_wallet, btf_bridge, fee_charge) = init_bridge().await;

    let bridge_client = ctx.icrc_bridge_client(ADMIN);
    bridge_client
        .add_to_whitelist(ctx.canisters().token_1())
        .await
        .unwrap()
        .unwrap();

    let base_token_id = Id256::from(&ctx.canisters().token_1());
    let wrapped_token = ctx
        .create_wrapped_token(&john_wallet, &btf_bridge, base_token_id)
        .await
        .unwrap();

    let amount = 300_000u64;

    let evm_client = ctx.wrapped_evm();
    let native_token_amount = 10_u64.pow(17);
    ctx.native_token_deposit(
        &evm_client,
        fee_charge.clone(),
        &john_wallet,
        native_token_amount.into(),
    )
    .await
    .unwrap();

    let john_address: H160 = john_wallet.address().into();

    eprintln!("burning icrc tokens and creating mint order");
    ctx.burn_icrc2(
        JOHN,
        &john_wallet,
        &btf_bridge,
        &wrapped_token,
        amount as _,
        Some(john_address),
        None,
    )
    .await
    .unwrap();

    ctx.advance_by_times(Duration::from_secs(2), 25).await;

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
            &ctx.wrapped_evm(),
            &john_wallet,
            &wrapped_token,
            base_token_id.0.as_slice(),
            (&john()).into(),
            &btf_bridge,
            wrapped_balance,
        )
        .await
        .unwrap()
        .0;

    ctx.advance_by_times(Duration::from_secs(2), 10).await;

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
    let (ctx, john_wallet, btf_bridge, fee_charge) = init_bridge().await;

    let minter_client = ctx.icrc_bridge_client(ADMIN);
    minter_client
        .add_to_whitelist(ctx.canisters().token_1())
        .await
        .unwrap()
        .unwrap();
    let base_token_id = Id256::from(&ctx.canisters().token_1());
    let wrapped_token = ctx
        .create_wrapped_token(&john_wallet, &btf_bridge, base_token_id)
        .await
        .unwrap();

    let amount = 3_000_000u64;

    let evm_client = ctx.wrapped_evm();
    let native_token_amount = 10_u64.pow(17);
    ctx.native_token_deposit(
        &evm_client,
        fee_charge.clone(),
        &john_wallet,
        native_token_amount.into(),
    )
    .await
    .unwrap();

    eprintln!("burning icrc tokens and creating mint order");
    let john_address: H160 = john_wallet.address().into();
    ctx.burn_icrc2(
        JOHN,
        &john_wallet,
        &btf_bridge,
        &wrapped_token,
        amount as _,
        Some(john_address.clone()),
        None,
    )
    .await
    .unwrap();

    ctx.advance_by_times(Duration::from_secs(2), 25).await;

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
            &ctx.wrapped_evm(),
            &john_wallet,
            &wrapped_token,
            base_token_id.0.as_slice(),
            john_principal_id256,
            &btf_bridge,
            wrapped_balance,
        )
        .await
        .unwrap()
        .0;

    ctx.advance_by_times(Duration::from_secs(2), 20).await;

    let minter_client = ctx.icrc_bridge_client(ADMIN);
    let operation = dbg!(
        minter_client
            .get_operations_list(&john_address, None, None)
            .await
            .unwrap()
    )
    .last()
    .cloned()
    .unwrap()
    .1;

    let IcrcBridgeOp::WaitForErc20MintConfirm {
        order,
        tx_hash,
        is_refund,
        mint_results,
    } = operation
    else {
        panic!("expected ConfirmMint operation state");
    };

    assert!(is_refund);
    assert!(tx_hash.is_none());
    assert!(mint_results.is_empty());

    let receipt = ctx
        .batch_mint_erc_20_with_order(&john_wallet, &btf_bridge, order)
        .await
        .unwrap();

    assert_eq!(
        receipt.status,
        Some(U64::from(1u64)),
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
    let ctx = PocketIcTestContext::new(&[CanisterType::Icrc2Bridge]).await;
    let mut admin_client = ctx.icrc_bridge_client(ADMIN);
    admin_client.set_owner(alice()).await.unwrap();

    // Now Alice is owner, so admin can't update owner anymore.
    let err = admin_client.set_owner(alice()).await.unwrap_err();
    assert!(matches!(
        err,
        CanisterClientError::PocketIcTestError(RejectResponse {
            error_code: ic_exports::pocket_ic::ErrorCode::CanisterCalledTrap,
            ..
        })
    ));

    // Now Alice is owner, so she can update owner.
    let mut alice_client = ctx.icrc_bridge_client(ALICE);
    alice_client.set_owner(alice()).await.unwrap();
}

#[tokio::test]
async fn canister_log_config_should_still_be_storable_after_upgrade() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Icrc2Bridge]).await;

    let minter_client = ctx.icrc_bridge_client(ADMIN);

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
    ctx.upgrade_icrc2_bridge_canister().await.unwrap();
    let settings = minter_client.get_logger_settings().await.unwrap();
    println!("LOGGER SETTINGS: {settings:?}");
    minter_client
        .set_logger_filter("debug".to_string())
        .await
        .unwrap()
        .unwrap();
}

#[tokio::test]
async fn test_canister_build_data() {
    let ctx = PocketIcTestContext::new(&[CanisterType::Icrc2Bridge]).await;
    let minter_client = ctx.icrc_bridge_client(ALICE);
    let build_data = minter_client.get_canister_build_data().await.unwrap();
    assert!(build_data.pkg_name.contains("icrc2_bridge"));
}

#[tokio::test]
async fn test_icrc2_tokens_approve_after_mint() {
    let (ctx, john_wallet, btf_bridge, fee_charge) = init_bridge().await;

    let base_token_id = Id256::from(&ctx.canisters().token_1());
    let wrapped_token = ctx
        .create_wrapped_token(&john_wallet, &btf_bridge, base_token_id)
        .await
        .unwrap();

    let amount = 300_000u64;
    let approve_amount = U256::from(1000_u64);

    eprintln!("burning icrc tokens and creating mint order");
    let john_address: H160 = john_wallet.address().into();
    let spender_wallet = ctx.new_wallet(0).await.unwrap();

    let evm_client = ctx.wrapped_evm();
    let native_token_amount = 10_u64.pow(17);
    ctx.native_token_deposit(
        &evm_client,
        fee_charge.clone(),
        &john_wallet,
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
        &btf_bridge,
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

    ctx.advance_by_times(Duration::from_secs(2), 25).await;

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

    let allowance_data = ctx
        .wrapped_evm()
        .eth_call(
            Some(john_address),
            Some(wrapped_token),
            Some(0u64.into()),
            3_000_000,
            Some(DEFAULT_GAS_PRICE.into()),
            Some(input.into()),
        )
        .await
        .expect("eth_call failed");

    let allowance: U256 = WrappedToken::allowanceCall::abi_decode_returns(&allowance_data, true)
        .unwrap()
        ._0
        .into();

    assert_eq!(allowance, approve_amount);
}

async fn icrc2_token_bridge(
    ctx: &PocketIcTestContext,
    john_wallet: LocalWallet,
    btf_bridge: &H160,
    fee_charge: &H160,
    wrapped_token: &H160,
) {
    let minter_client = ctx.icrc_bridge_client(ADMIN);
    minter_client
        .add_to_whitelist(ctx.canisters().token_1())
        .await
        .unwrap()
        .unwrap();

    let amount = 300_000u64;

    let evm_client = ctx.wrapped_evm();
    let native_token_amount = 10_u64.pow(10);
    ctx.native_token_deposit(
        &evm_client,
        fee_charge.clone(),
        &john_wallet,
        native_token_amount.into(),
    )
    .await
    .unwrap();

    let john_address: H160 = john_wallet.address().into();

    eprintln!("burning icrc tokens and creating mint order");
    ctx.burn_icrc2(
        JOHN,
        &john_wallet,
        btf_bridge,
        wrapped_token,
        amount as _,
        Some(john_address.clone()),
        None,
    )
    .await
    .unwrap();

    ctx.advance_by_times(Duration::from_secs(2), 20).await;

    let (id, operation) = minter_client
        .get_operations_list(&john_address, None, None)
        .await
        .unwrap()
        .last()
        .cloned()
        .unwrap();

    let operation = IcrcBridgeOpImpl(operation);

    if !(operation.is_complete()) {
        let _ = dbg!(minter_client.get_operation_log(id).await);
    }

    assert!(operation.is_complete());
}

#[tokio::test]
async fn test_minter_canister_address_balances_gets_replenished_after_roundtrip() {
    let (ctx, john_wallet, btf_bridge, fee_charge) = init_bridge().await;
    let evm_client = ctx.wrapped_evm();

    let minter_address = ctx
        .get_icrc_bridge_canister_evm_address(ADMIN)
        .await
        .unwrap();

    let native_token_amount = 10_u64.pow(17);
    ctx.native_token_deposit(
        &evm_client,
        fee_charge.clone(),
        &john_wallet,
        native_token_amount.into(),
    )
    .await
    .unwrap();

    let base_token_id = Id256::from(&ctx.canisters().token_1());

    let wrapped_token = ctx
        .create_wrapped_token(&john_wallet, &btf_bridge, base_token_id)
        .await
        .unwrap();

    let bridge_balance_before_mint = evm_client
        .eth_get_balance(&minter_address, did::BlockNumber::Latest)
        .await
        .expect("eth_get_balance failed");

    const TOTAL_TX: u64 = 10;
    for _ in 0..TOTAL_TX {
        icrc2_token_bridge(
            &ctx,
            john_wallet.clone(),
            &btf_bridge,
            &fee_charge,
            &wrapped_token,
        )
        .await;

        let bridge_balance_after_mint = evm_client
            .eth_get_balance(&minter_address, did::BlockNumber::Latest)
            .await
            .expect("eth_get_balance failed");

        assert!(dbg!(bridge_balance_before_mint.clone()) <= dbg!(bridge_balance_after_mint));
    }
}

#[tokio::test]
async fn rescheduling_deposit_operation() {
    let (ctx, john_wallet, btf_bridge, fee_charge) = init_bridge().await;

    let bridge_client = ctx.icrc_bridge_client(ADMIN);
    bridge_client
        .add_to_whitelist(ctx.canisters().token_1())
        .await
        .unwrap()
        .unwrap();

    let base_token_id = Id256::from(&ctx.canisters().token_1());
    let wrapped_token = ctx
        .create_wrapped_token(&john_wallet, &btf_bridge, base_token_id)
        .await
        .unwrap();

    let amount = 300_000u64;

    let evm_client = ctx.wrapped_evm();
    let native_token_amount = 10_u64.pow(17);
    ctx.native_token_deposit(
        &evm_client,
        fee_charge.clone(),
        &john_wallet,
        native_token_amount.into(),
    )
    .await
    .unwrap();

    let john_address: H160 = john_wallet.address().into();

    ctx.advance_by_times(Duration::from_secs(2), 5).await;

    eprintln!("burning icrc tokens and creating mint order");
    ctx.burn_icrc2(
        JOHN,
        &john_wallet,
        &btf_bridge,
        &wrapped_token,
        amount as _,
        Some(john_address),
        None,
    )
    .await
    .unwrap();

    // Stop token canister to make BurnIcrc operation fail.
    ctx.client
        .stop_canister(ctx.canisters().token_1(), Some(ctx.admin()))
        .await
        .unwrap();

    let mut num_tries = 0;
    const MAX_RETRIES: usize = 30;
    loop {
        num_tries += 1;
        if num_tries > MAX_RETRIES {
            panic!("Deposit operation was not scheduled after {MAX_RETRIES} tries");
        }
        ctx.advance_time(Duration::from_secs(1)).await;

        let operations_list = bridge_client
            .get_operations_list(&john_wallet.address().into(), None, None)
            .await
            .unwrap();

        // Loop until mint tx sent by bridge canister.
        let Some(last_op) = operations_list.last() else {
            continue;
        };

        let IcrcBridgeOp::BurnIcrc2Tokens { .. } = dbg!(last_op.clone().1) else {
            continue;
        };

        break;
    }

    // Resume token canister to make the following operations work.
    ctx.client
        .start_canister(ctx.canisters().token_1(), Some(ctx.admin()))
        .await
        .unwrap();

    let base_token_client = ctx.icrc_token_1_client(JOHN);
    let base_balance = base_token_client
        .icrc1_balance_of(john().into())
        .await
        .unwrap();

    let ctx_t = ctx.clone();
    let john_wallet_t = john_wallet.clone();

    let (operation_id, log) = block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let bridge_client = ctx.icrc_bridge_client(JOHN);
            let john_wallet = john_wallet_t.clone();
            Box::pin(async move {
                let (operation_id, _) = bridge_client
                    .get_operations_list(&john_wallet.address().into(), None, None)
                    .await?
                    .last()
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Operation not found"))?;

                let log = bridge_client.get_operation_log(operation_id).await?;
                if let Some(log) = log {
                    Ok((operation_id, log))
                } else {
                    anyhow::bail!("Operation log is empty")
                }
            })
        },
        &ctx,
        Duration::from_secs(120),
    )
    .await;

    eprintln!("OPERATION LOG");
    dbg!(&log);

    eprintln!("checking wrapped token balance");
    let wrapped_balance = ctx
        .check_erc20_balance(&wrapped_token, &john_wallet, None)
        .await
        .unwrap();

    assert_eq!(base_balance, ICRC1_INITIAL_BALANCE - ICRC1_TRANSFER_FEE);
    assert_eq!(wrapped_balance as u64, 0);

    let ctx_t = ctx.clone();
    let john_wallet_t = john_wallet.clone();

    block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let bridge_client = ctx.icrc_bridge_client(JOHN);
            let john_wallet = john_wallet_t.clone();
            Box::pin(async move {
                let (_, op) = bridge_client
                    .get_operations_list(&john_wallet.address().into(), None, None)
                    .await?
                    .last()
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Operation not found"))?;

                if matches!(op, IcrcBridgeOp::BurnIcrc2Tokens { .. }) {
                    Ok(())
                } else {
                    anyhow::bail!("Operation is not complete; {op:?}")
                }
            })
        },
        &ctx,
        Duration::from_secs(120),
    )
    .await;

    ctx.reschedule_operation(operation_id, &john_wallet, &btf_bridge)
        .await
        .unwrap();

    let ctx_t = ctx.clone();
    let john_wallet_t = john_wallet.clone();

    block_until_succeeds(
        move || {
            let ctx = ctx_t.clone();
            let bridge_client = ctx.icrc_bridge_client(JOHN);
            let john_wallet = john_wallet_t.clone();
            Box::pin(async move {
                let (_, op) = bridge_client
                    .get_operations_list(&john_wallet.address().into(), None, None)
                    .await?
                    .last()
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("Operation not found"))?;

                if matches!(op, IcrcBridgeOp::WrappedTokenMintConfirmed { .. }) {
                    Ok(())
                } else {
                    anyhow::bail!("Operation is not complete; {op:?}")
                }
            })
        },
        &ctx,
        Duration::from_secs(120),
    )
    .await;

    let wrapped_balance = ctx
        .check_erc20_balance(&wrapped_token, &john_wallet, None)
        .await
        .unwrap();
    assert_eq!(wrapped_balance as u64, amount);
}

// Let's check that spamming reschedule does not break anything
#[tokio::test]
async fn test_icrc2_tokens_roundtrip_with_reschedule_spam() {
    async fn spam_reschedule_requests(
        ctx: PocketIcTestContext,
        wallet: LocalWallet,
        btf_bridge: H160,
        spam_stopper: CancellationToken,
        semaphore: Arc<Semaphore>,
    ) {
        let bridge_client = ctx.icrc_bridge_client(JOHN);
        while !spam_stopper.is_cancelled() {
            let operations = bridge_client
                .get_operations_list(&wallet.address().into(), None, None)
                .await
                .unwrap();
            for (id, _) in &operations {
                let _ = semaphore.acquire().await;
                ctx.reschedule_operation(*id, &wallet, &btf_bridge)
                    .await
                    .unwrap();
            }
        }
    }

    let (ctx, john_wallet, btf_bridge, fee_charge) = init_bridge().await;

    let spam_stopper = CancellationToken::new();
    let semaphore = Arc::new(Semaphore::new(1));
    tokio::task::spawn(spam_reschedule_requests(
        ctx.clone(),
        john_wallet.clone(),
        btf_bridge.clone(),
        spam_stopper.clone(),
        semaphore.clone(),
    ));
    let _ = spam_stopper.drop_guard();

    let bridge_client = ctx.icrc_bridge_client(ADMIN);
    bridge_client
        .add_to_whitelist(ctx.canisters().token_1())
        .await
        .unwrap()
        .unwrap();

    let base_token_id = Id256::from(&ctx.canisters().token_1());
    let wrapped_token = ctx
        .create_wrapped_token(&john_wallet, &btf_bridge, base_token_id)
        .await
        .unwrap();

    let amount = 300_000u64;

    let evm_client = ctx.wrapped_evm();
    let native_token_amount = 10_u64.pow(17);
    ctx.native_token_deposit(
        &evm_client,
        fee_charge.clone(),
        &john_wallet,
        native_token_amount.into(),
    )
    .await
    .unwrap();

    let john_address: H160 = john_wallet.address().into();

    eprintln!("burning icrc tokens and creating mint order");
    {
        let _ = semaphore.acquire().await;
        ctx.burn_icrc2(
            JOHN,
            &john_wallet,
            &btf_bridge,
            &wrapped_token,
            amount as _,
            Some(john_address),
            None,
        )
        .await
        .unwrap();
    }

    ctx.advance_by_times(Duration::from_secs(2), 25).await;

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

    {
        let _ = semaphore.acquire().await;
        let _operation_id = ctx
            .burn_wrapped_erc_20_tokens(
                &ctx.wrapped_evm(),
                &john_wallet,
                &wrapped_token,
                base_token_id.0.as_slice(),
                (&john()).into(),
                &btf_bridge,
                wrapped_balance,
            )
            .await
            .unwrap()
            .0;
    }

    ctx.advance_by_times(Duration::from_secs(2), 10).await;

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
async fn icrc_bridge_stress_test() {
    let context = PocketIcTestContext::new(&[CanisterType::Icrc2Bridge]).await;

    let config = StressTestConfig {
        users_number: 5,
        user_deposits_per_token: 3,
        init_user_balance: 2u64.pow(30).into(),
        operation_amount: 2u64.pow(20).into(),
        operation_timeout: Duration::from_secs(30),
        wait_per_iteration: Duration::from_secs(1),
        charge_fee: true,
    };

    // If set more then one token, tests probably will fail because of
    // parallel tx nonces calculation issue.
    icrc::stress_test_icrc_bridge_with_ctx(context, 1, config).await;
}

/// Initialize test environment with:
/// - john wallet with native tokens,
/// - operation points for john,
/// - bridge contract
pub async fn init_bridge() -> (PocketIcTestContext, LocalWallet, H160, H160) {
    let ctx = PocketIcTestContext::new(&CanisterType::ICRC2_MINTER_TEST_SET).await;
    let john_wallet = ctx.new_wallet(u128::MAX).await.unwrap();

    let fee_charge_deployer = ctx.new_wallet(u128::MAX).await.unwrap();
    let expected_fee_charge_address =
        bridge_utils::get_contract_address(fee_charge_deployer.address(), U256::zero());

    let wrapped_token_deployer = ctx
        .initialize_wrapped_token_deployer_contract(&john_wallet)
        .await
        .unwrap();

    // give some time to the bridge to initialize
    let mut minter_canister_address = None;
    let icrc_bridge_client = &ctx.icrc_bridge_client(ADMIN);
    for _ in 0..10 {
        minter_canister_address = match icrc_bridge_client
            .get_bridge_canister_evm_address()
            .await
            .unwrap()
        {
            Ok(addr) => Some(addr),
            Err(_) => {
                println!("retry");
                ctx.advance_time(Duration::from_secs(2)).await;
                continue;
            }
        };

        break;
    }

    let Some(minter_canister_address) = minter_canister_address else {
        panic!("Bridge canister was not initialized");
    };

    let btf_bridge = ctx
        .initialize_btf_bridge(
            minter_canister_address,
            Some(expected_fee_charge_address.into()),
            wrapped_token_deployer,
        )
        .await
        .unwrap();

    icrc_bridge_client
        .set_btf_bridge_contract(&btf_bridge)
        .await
        .unwrap();

    let fee_charge_address = ctx
        .initialize_fee_charge_contract(&fee_charge_deployer, &[btf_bridge.clone()])
        .await
        .unwrap();
    assert_eq!(expected_fee_charge_address, fee_charge_address.0);

    (ctx, john_wallet, btf_bridge, fee_charge_address)
}
