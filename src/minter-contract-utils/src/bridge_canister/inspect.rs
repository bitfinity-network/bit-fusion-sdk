use ic_exports::ic_cdk::api;
use ic_storage::IcStorage;

use crate::bridge_canister::BridgeCore;

pub fn bridge_inspect() {
    let core = BridgeCore::get();
    let core = core.borrow();
    let method = api::call::method_name();

    match method.as_str() {
        "set_logger_filter" => core.inspect_set_logger_filter(),
        "ic_logs" => core.inspect_ic_logs(),
        "set_owner" => core.inspect_set_owner(),
        "set_evm_principal" => core.inspect_set_evm_principal(),
        "set_bft_bridge_contract" => core.inspect_set_bft_bridge_contract(),
        _ => {} // "set_evm_principal" => {
                //     let (evm,) = api::call::arg_data::<(Principal,)>(Default::default());
                //     MinterCanister::set_evm_principal_inspect_message_check(ic::caller(), evm, &state)
                // }
                // "set_owner" => {
                //     let (owner,) = api::call::arg_data::<(Principal,)>(Default::default());
                //     MinterCanister::set_owner_inspect_message_check(ic::caller(), owner, &state)
                // }
                // "add_to_whitelist" | "remove_from_whitelist" => {
                //     let (principal,) = api::call::arg_data::<(Principal,)>(Default::default());
                //     MinterCanister::access_control_inspect_message_check(ic::caller(), principal, &state)
                // }
                // _ => Ok(()),
    }
}
