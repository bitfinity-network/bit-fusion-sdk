#[cfg(target_family = "wasm")]
use bridge_canister::bridge_inspect;
use bridge_did::error::BftResult;
#[cfg(target_family = "wasm")]
use ic_exports::ic_cdk;
#[cfg(target_family = "wasm")]
use ic_exports::ic_cdk::{api, inspect_message};
#[cfg(target_family = "wasm")]
use ic_exports::ic_kit::ic;

use crate::BtcBridge;

#[cfg(target_family = "wasm")]
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

#[allow(dead_code)]
async fn inspect_method(method: &str) -> BftResult<()> {
    match method {
        method if method.starts_with("admin_") => BtcBridge::inspect_caller_is_owner(),
        _ => Ok(()),
    }
}
