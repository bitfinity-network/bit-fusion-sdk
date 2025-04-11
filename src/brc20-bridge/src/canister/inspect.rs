#[cfg(feature = "export-api")]
use bridge_canister::bridge_inspect;
use bridge_canister::inspect::inspect_caller_is_owner;
use bridge_canister::runtime::state::SharedConfig;
#[cfg(feature = "export-api")]
use bridge_canister::runtime::state::config::ConfigStorage;
#[cfg(feature = "export-api")]
use ic_exports::ic_cdk;
#[cfg(feature = "export-api")]
use ic_exports::ic_cdk::{api, inspect_message};
use ic_exports::ic_kit::ic;
#[cfg(feature = "export-api")]
use ic_storage::IcStorage;

#[cfg(feature = "export-api")]
#[inspect_message]
async fn inspect_message() {
    bridge_inspect();
    inspect_method(&api::call::method_name());

    api::call::accept_message();
}

pub fn inspect_is_owner(config: SharedConfig) {
    let caller = ic::caller();
    let owner = config.borrow().get_owner();
    inspect_caller_is_owner(owner, caller)
}

#[cfg(feature = "export-api")]
fn inspect_method(method: &str) {
    let config = ConfigStorage::get();
    match method {
        method if method.starts_with("admin_") => inspect_is_owner(config),
        _ => {}
    }
}
