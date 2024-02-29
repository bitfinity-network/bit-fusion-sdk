use candid::{encode_one, Principal};
use ic_cdk::api::management_canister::main::CanisterId;
use pocket_ic::{PocketIc, WasmResult};
use std::fs;

fn load_wasm(path: &str) -> Vec<u8> {
    fs::read(path).expect("Invalid wasm path")
}

#[test]
fn test_hello_canister() {
    let pic = PocketIc::new();
    let canister_id = pic.create_canister();
    pic.add_cycles(canister_id, 2_000_000_000_000_000);

    let wasm_bytes =
        load_wasm("../../target/wasm32-unknown-unknown/release/http_outcall_mock.wasm");
    pic.install_canister(canister_id, wasm_bytes, vec![], None);

    let reply = call_hello_canister(&pic, canister_id, "get_icp_usd_rate");
    match reply {
        WasmResult::Reply(result) => {
            println!("{:?}", String::from_utf8(result));
        }
        WasmResult::Reject(cause) => {
            panic!("{:?}", cause);
        }
    }
}

fn call_hello_canister(pic: &PocketIc, canister_id: CanisterId, method: &str) -> WasmResult {
    pic.update_call(
        canister_id,
        Principal::anonymous(),
        method,
        encode_one(()).unwrap(),
    )
    .expect("Failed to call hello canister")
}
