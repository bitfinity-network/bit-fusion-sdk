#[cfg(feature = "export-api")]
use bridge_canister::bridge_inspect;
use bridge_did::error::BTFResult;
#[cfg(feature = "export-api")]
use ic_exports::ic_cdk::{api, inspect_message};
#[cfg(feature = "export-api")]
use ic_exports::ic_kit::ic;

use crate::BtcBridge;

#[cfg(feature = "export-api")]
#[inspect_message]
async fn inspect_message() {
    bridge_inspect();
    let check_result = inspect_method(&api::msg_method_name()).await;

    if let Err(e) = check_result {
        ic::trap(&format!("Call rejected by inspect check: {e:?}"));
    } else {
        api::accept_message();
    }
}

#[allow(dead_code)]
async fn inspect_method(method: &str) -> BTFResult<()> {
    match method {
        method if method.starts_with("admin_") => BtcBridge::inspect_caller_is_owner(),
        _ => Ok(()),
    }
}
