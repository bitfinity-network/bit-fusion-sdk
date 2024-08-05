use bridge_canister::bridge_inspect;
use bridge_did::error::BftResult;
use ic_exports::ic_cdk;
use ic_exports::ic_cdk::{api, inspect_message};
use ic_exports::ic_kit::ic;

use crate::BtcBridge;

#[inspect_message]
async fn inspect_message() {
    bridge_inspect();
    let check_result = inspect_method(&api::call::method_name()).await;

    if let Err(e) = check_result {
        ic::trap(&format!("Call rejected by inspect check: {e:?}"));
    } else {
        api::call::accept_message();
    }
}

async fn inspect_method(method: &str) -> BftResult<()> {
    match method {
        method if method.starts_with("admin_") => BtcBridge::inspect_caller_is_owner(),
        _ => Ok(()),
    }
}
