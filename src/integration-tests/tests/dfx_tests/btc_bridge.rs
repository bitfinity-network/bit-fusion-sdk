use std::time::Duration;

use crate::context::stress::StressTestConfig;
use crate::context::CanisterType;

#[tokio::test]
#[serial_test::serial]
async fn test_btc_bridge_stress_test() {
    let context = crate::dfx_tests::DfxTestContext::new(&CanisterType::BTC_CANISTER_SET).await;

    let config = StressTestConfig {
        users_number: 5,
        user_deposits_per_token: 2,
        init_user_balance: 100_000_000u64.into(), // 1 BTC
        operation_amount: 5_000_000u64.into(),    // 0.05 BTC
        operation_timeout: Duration::from_secs(120),
        wait_per_iteration: Duration::from_secs(10),
        charge_fee: false,
    };

    crate::context::stress::btc::stress_test_btc_bridge_with_ctx(context, config).await;
}
