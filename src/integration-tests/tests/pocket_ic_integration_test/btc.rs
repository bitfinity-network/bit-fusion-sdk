use crate::context::CanisterType;
use crate::pocket_ic_integration_test::PocketIcTestContext;

#[tokio::test]
async fn set_up_btc_canisters() {
    let ctx = PocketIcTestContext::new(&CanisterType::BTC_CANISTER_SET).await;
}