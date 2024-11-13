use bridge_canister::bridge_inspect;
use bridge_did::error::BTFResult;
use ic_exports::ic_cdk;
use ic_exports::ic_cdk::{api, inspect_message};
use ic_exports::ic_kit::ic;

use crate::canister;

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

async fn inspect_method(method: &str) -> BTFResult<()> {
    let config = canister::get_runtime_state().borrow().config.clone();
    match method {
        "set_base_btf_bridge_contract" => config.borrow().check_owner(ic::caller()),
        _ => Ok(()),
    }
}
