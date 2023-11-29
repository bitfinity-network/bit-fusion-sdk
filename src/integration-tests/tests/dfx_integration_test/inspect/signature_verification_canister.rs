use crate::context::CanisterType;
use crate::dfx_integration_test::{DfxTestContext, ADMIN};

#[tokio::test]
async fn test_inspect_add_access_fail() {
    let ctx = DfxTestContext::new(&CanisterType::EVM_TEST_SET).await;

    ctx.assert_inspect_message_failure(
        &ctx.alice,
        ctx.canisters.signature_verification(),
        "add_access",
        (ctx.alice.get_principal().unwrap(),),
    )
    .await;
}

#[tokio::test]
async fn test_inspect_add_access_ok() {
    let ctx = DfxTestContext::new(&CanisterType::EVM_TEST_SET).await;

    ctx.assert_inspect_message_success(
        &ctx.agent_by_name(ADMIN),
        ctx.canisters.signature_verification(),
        "add_access",
        (ctx.alice.get_principal().unwrap(),),
    )
    .await;
}

#[tokio::test]
async fn test_inspect_remove_access_fail() {
    let ctx = DfxTestContext::new(&CanisterType::EVM_TEST_SET).await;

    ctx.assert_inspect_message_failure(
        &ctx.alice,
        ctx.canisters.signature_verification(),
        "remove_access",
        (ctx.alice.get_principal().unwrap(),),
    )
    .await;
}

#[tokio::test]
async fn test_inspect_remove_access_ok() {
    let ctx = DfxTestContext::new(&CanisterType::EVM_TEST_SET).await;

    ctx.assert_inspect_message_success(
        &ctx.agent_by_name(ADMIN),
        ctx.canisters.signature_verification(),
        "remove_access",
        (ctx.alice.get_principal().unwrap(),),
    )
    .await;
}

#[tokio::test]
async fn test_inspect_set_owner_fail() {
    let ctx = DfxTestContext::new(&CanisterType::EVM_TEST_SET).await;

    ctx.assert_inspect_message_failure(
        &ctx.alice,
        ctx.canisters.signature_verification(),
        "set_owner",
        (ctx.alice.get_principal().unwrap(),),
    )
    .await;
}

#[tokio::test]
async fn test_inspect_set_owner_ok() {
    let ctx = DfxTestContext::new(&CanisterType::EVM_TEST_SET).await;

    ctx.assert_inspect_message_success(
        &ctx.agent_by_name(ADMIN),
        ctx.canisters.signature_verification(),
        "set_owner",
        (ctx.alice.get_principal().unwrap(),),
    )
    .await;
}
