use candid::Principal;
use ic_exports::ic_cdk;
use ic_exports::ic_cdk::{api, inspect_message};
use ic_exports::ic_kit::ic;
use minter_contract_utils::bridge_canister::bridge_inspect;
use minter_did::error::Result;

use crate::MinterCanister;

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

async fn inspect_method(method: &str) -> Result<()> {
    match method {
        "add_to_whitelist" | "remove_from_whitelist" => {
            let (principal,) = api::call::arg_data::<(Principal,)>(Default::default());
            MinterCanister::access_control_inspect_message_check(ic::caller(), principal)
        }
        _ => Ok(()),
    }
}
