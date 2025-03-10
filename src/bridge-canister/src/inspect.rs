use candid::Principal;
use ic_exports::ic_cdk::api;
use ic_exports::ic_kit::ic;
use ic_storage::IcStorage;

use crate::runtime::state::config::ConfigStorage;
use crate::runtime::state::SharedConfig;

/// Runs inspect checks for the bridge canister API methods. This function should be called from
/// the canister `#[inspect]` function. In case any of the checks do not pass, the function
/// will `trap` (panic).
pub fn bridge_inspect() {
    let config = ConfigStorage::get();
    let method = api::call::method_name();

    match method.as_str() {
        "set_logger_filter" => inspect_set_logger_filter(config),
        "ic_logs" => inspect_ic_logs(config),
        "set_owner" => inspect_set_owner(config),
        "set_btf_bridge_contract" => inspect_set_btf_bridge_contract(config),
        _ => {}
    }
}

/// Inspects if owner principal is not an anonymous.
pub fn inspect_new_owner_is_valid(new_owner: Principal) {
    if new_owner == Principal::anonymous() {
        ic::trap("Owner cannot be an anonymous");
    }
}

/// Inspect check for `ic_logs` API method.
pub fn inspect_ic_logs(config: SharedConfig) {
    let caller = ic::caller();
    let owner = config.borrow().get_owner();
    inspect_caller_is_owner(owner, caller)
}

/// Inspect check for `set_logger_filter` API method.
pub fn inspect_set_logger_filter(config: SharedConfig) {
    let caller = ic::caller();
    let owner = config.borrow().get_owner();
    inspect_caller_is_owner(owner, caller)
}

/// Inspect check for `set_owner` API method.
pub fn inspect_set_owner(config: SharedConfig) {
    let caller = ic::caller();
    let owner = config.borrow().get_owner();
    inspect_caller_is_owner(owner, caller)
}

/// Inspect check for `set_evm_principal` API method.
pub fn inspect_set_evm_principal(config: SharedConfig) {
    let caller = ic::caller();
    let owner = config.borrow().get_owner();
    inspect_caller_is_owner(owner, caller)
}

/// Inspect check for `set_btf_bridge_contract` API method.
pub fn inspect_set_btf_bridge_contract(config: SharedConfig) {
    let caller = ic::caller();
    let owner = config.borrow().get_owner();
    inspect_caller_is_owner(owner, caller)
}

/// Checks if the caller is the owner.
pub fn inspect_caller_is_owner(owner: Principal, caller: Principal) {
    if ic::caller() != owner {
        log::debug!("Owner only method is called by non-owner. Owner: {owner}. Caller: {caller}");
        ic::trap("Running this method is only allowed for the owner of the canister")
    }
}
