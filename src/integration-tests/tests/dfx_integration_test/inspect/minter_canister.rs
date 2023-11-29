use std::time::Duration;

use candid::utils::ArgumentEncoder;
use candid::Principal;
use did::{TransactionReceipt, H160};
use ic_test_utils::{get_agent, Agent};
use minter_did::init::OperationPricing;
use minter_did::reason::Icrc2Burn;

use crate::context::CanisterType;
use crate::dfx_integration_test::DfxTestContext;

#[tokio::test]
async fn test_admin_endpoints() {
    let ctx = DfxTestContext::new(&[CanisterType::Minter]).await;

    let price = OperationPricing::default();
    let principal = Principal::management_canister();

    // check Alice is not able to do the calls
    async fn by_alice(
        ctx: &DfxTestContext,
        alice: &Agent,
        method: &str,
        args: impl ArgumentEncoder,
    ) {
        ctx.assert_inspect_message_failure(alice, ctx.canisters.minter(), method, args)
            .await;
    }
    let agent = get_agent(super::ALICE, None, Some(Duration::from_secs(180)))
        .await
        .unwrap();
    let alice = &agent;
    by_alice(&ctx, alice, "ic_logs", (5_usize,)).await;
    by_alice(&ctx, alice, "set_logger_filter", ("debug",)).await;
    by_alice(&ctx, alice, "set_evm_principal", (principal,)).await;
    by_alice(&ctx, alice, "set_operation_pricing", (price,)).await;
    by_alice(&ctx, alice, "set_owner", (principal,)).await;

    // check admin is able to do the calls
    async fn by_owner(ctx: &DfxTestContext, method: &str, args: impl ArgumentEncoder) {
        ctx.assert_inspect_message_success(&ctx.max, ctx.canisters.minter(), method, args)
            .await;
    }

    by_owner(&ctx, "ic_logs", (5_usize,)).await;
    by_owner(&ctx, "set_logger_filter", ("debug",)).await;
    by_owner(&ctx, "set_evm_principal", (principal,)).await;
    by_owner(&ctx, "set_operation_pricing", (price,)).await;
    by_owner(&ctx, "set_owner", (principal,)).await;
}

#[tokio::test]
async fn test_enough_operation_points() {
    let ctx = DfxTestContext::new(&[CanisterType::Minter]).await;

    let reason = Icrc2Burn {
        amount: 100u64.into(),
        icrc2_token_principal: Principal::management_canister(),
        from_subaccount: Default::default(),
        recipient_address: Default::default(),
        operation_id: 42,
    };

    // check if user can't perform actions without operation points.
    async fn by_alice(ctx: &DfxTestContext, method: &str, args: impl ArgumentEncoder) {
        ctx.assert_inspect_message_failure(&ctx.alice, ctx.canisters.minter(), method, args)
            .await;
    }

    by_alice(&ctx, "create_erc_20_mint_order", (reason,)).await;
    by_alice(
        &ctx,
        "start_icrc2_mint",
        (H160::from_slice(&[42; 20]), 42u32),
    )
    .await;
}

#[tokio::test]
async fn test_evm_tx_notification_from_wrong_principal() {
    let ctx = DfxTestContext::new(&[CanisterType::Minter]).await;
    ctx.assert_inspect_message_failure(
        &ctx.alice,
        ctx.canisters.minter(),
        "on_evm_transaction_notification",
        (
            Some(TransactionReceipt::default()),
            Principal::from_slice(&[42; 20]).as_slice().to_vec(),
        ),
    )
    .await;
}

#[tokio::test]
async fn test_register_evmc_bft_bridge_with_bridge_address() {
    let ctx = DfxTestContext::new(&[CanisterType::Minter]).await;
    ctx.assert_inspect_message_failure(
        &ctx.alice,
        ctx.canisters.minter(),
        "register_evmc_bft_bridge",
        (H160::zero(),),
    )
    .await;
}
