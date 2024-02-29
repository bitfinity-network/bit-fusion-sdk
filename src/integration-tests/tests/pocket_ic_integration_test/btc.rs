use crate::context::{CanisterType, TestContext};
use crate::pocket_ic_integration_test::PocketIcTestContext;
use crate::utils::btc::{UpdateBalanceError, UtxoStatus};
use candid::{CandidType, Principal};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[tokio::test]
async fn set_up_btc_canisters() {
    let ctx = PocketIcTestContext::new(&CanisterType::BTC_CANISTER_SET).await;
    let client = ctx.client(ctx.canisters().ck_btc_minter(), "alice");

    #[derive(Debug, CandidType, Serialize, Deserialize)]
    struct GetBtcAddressArgs {
        owner: Option<Principal>,
        subaccount: Option<Principal>,
    }

    ctx.advance_time(Duration::from_secs(10)).await;
    let response: String = client
        .update(
            "get_btc_address",
            (GetBtcAddressArgs {
                owner: None,
                subaccount: None,
            },),
        )
        .await
        .unwrap();
    println!("{response}");

    let response: Result<UtxoStatus, UpdateBalanceError> = client
        .update(
            "update_balance",
            (GetBtcAddressArgs {
                owner: None,
                subaccount: None,
            },),
        )
        .await
        .unwrap();
    println!("{response:?}");
}
