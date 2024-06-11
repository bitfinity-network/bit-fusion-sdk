use candid::Principal;
use ic_exports::ic_cdk::{self, api};
use ic_exports::ic_cdk_macros::inspect_message;
use ic_exports::ic_kit::ic;
use minter_did::error::Result;

use crate::state::State;
use crate::MinterCanister;

#[inspect_message]
async fn inspect_message() {
    let check_result = inspect_method(&api::call::method_name()).await;

    if let Err(e) = check_result {
        ic::trap(&format!("Call rejected by inspect check: {e:?}"));
    } else {
        api::call::accept_message();
    }
}

async fn inspect_method(method: &str) -> Result<()> {
    let state = State::default();

    match method {
        "set_logger_filter" => {
            MinterCanister::set_logger_filter_inspect_message_check(ic::caller(), &state)
        }
        "ic_logs" => MinterCanister::ic_logs_inspect_message_check(ic::caller(), &state),
        "set_evm_principal" => {
            let (evm,) = api::call::arg_data::<(Principal,)>(Default::default());
            MinterCanister::set_evm_principal_inspect_message_check(ic::caller(), evm, &state)
        }
        "set_owner" => {
            let (owner,) = api::call::arg_data::<(Principal,)>(Default::default());
            MinterCanister::set_owner_inspect_message_check(ic::caller(), owner, &state)
        }
                "add_to_whitelist" | "remove_from_whitelist" => {
            let (principal,) = api::call::arg_data::<(Principal,)>(Default::default());
            MinterCanister::access_control_inspect_message_check(ic::caller(), principal, &state)
        }
        _ => Ok(()),
    }
}
