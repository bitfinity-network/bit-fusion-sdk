use bridge_canister::bridge_inspect;
use bridge_did::error::BTFResult;
use ic_exports::ic_cdk;
use ic_exports::ic_cdk::inspect_message;
use ic_exports::ic_kit::ic;

use crate::Icrc2BridgeCanister;

#[inspect_message]
async fn inspect_message() {
    bridge_inspect();
    let check_result = inspect_method(&ic_cdk::api::msg_method_name()).await;

    if let Err(e) = check_result {
        ic::trap(&format!("Call rejected by inspect check: {e:?}"));
    } else {
        ic_cdk::api::accept_message();
    }
}

async fn inspect_method(method: &str) -> BTFResult<()> {
    match method {
        "add_to_whitelist" | "remove_from_whitelist" => {
            let data = ic_cdk::api::msg_arg_data();
            let (principal,) = candid::decode_args(&data).map_err(|_| {
                bridge_did::error::Error::Serialization("Failed to decode arguments".to_string())
            })?;
            Icrc2BridgeCanister::access_control_inspect_message_check(ic::caller(), principal)
        }
        _ => Ok(()),
    }
}
