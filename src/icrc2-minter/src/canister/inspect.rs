#[cfg(target_family = "wasm")]
use bridge_canister::bridge_inspect;
use bridge_did::error::BftResult;
use candid::Principal;
#[cfg(target_family = "wasm")]
use ic_exports::ic_cdk;
use ic_exports::ic_cdk::api;
#[cfg(target_family = "wasm")]
use ic_exports::ic_cdk::inspect_message;
use ic_exports::ic_kit::ic;

use crate::MinterCanister;

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
        "add_to_whitelist" | "remove_from_whitelist" => {
            let (principal,) = api::call::arg_data::<(Principal,)>(Default::default());
            MinterCanister::access_control_inspect_message_check(ic::caller(), principal)
        }
        _ => Ok(()),
    }
}
