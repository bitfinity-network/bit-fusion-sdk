//! EVM related utilities for link params.

use std::process::Command;

use bridge_did::evm_link::EvmLink;
use candid::Principal;

use crate::contracts::EvmNetwork;

const CARGO_MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");
const MAINNET_PRINCIPAL: &str = "i3jjb-wqaaa-aaaaa-qadrq-cai";
const TESTNET_PRINCIPAL: &str = "4fe7g-7iaaa-aaaak-aegcq-cai";

/// Returns the IC host based on the EVM network.
pub fn ic_host(evm_network: EvmNetwork) -> String {
    match evm_network {
        EvmNetwork::Localhost => format!("http://127.0.0.1:{}", dfx_replica_port()),
        EvmNetwork::Mainnet => format!("https://{MAINNET_PRINCIPAL}.ic0.app"),
        EvmNetwork::Testnet => format!("https://{TESTNET_PRINCIPAL}.ic0.app"),
    }
}

/// Returns the EVM link based on the EVM network.
pub fn evm_link(evm_network: EvmNetwork) -> EvmLink {
    match evm_network {
        EvmNetwork::Localhost => EvmLink::Http(format!(
            "http://127.0.0.1:{}/?canisterId={}",
            dfx_webserver_port(),
            local_evm_principal()
        )),
        EvmNetwork::Mainnet => {
            EvmLink::Ic(Principal::from_text(MAINNET_PRINCIPAL).expect("Invalid principal"))
        }
        EvmNetwork::Testnet => {
            EvmLink::Ic(Principal::from_text(TESTNET_PRINCIPAL).expect("Invalid principal"))
        }
    }
}

/// Returns local dfx replica port
fn dfx_replica_port() -> u16 {
    dfx_info_port("replica-port")
}

/// Returns local dfx replica port
fn dfx_webserver_port() -> u16 {
    dfx_info_port("webserver-port")
}

/// Returns the port of the dfx service
fn dfx_info_port(service: &str) -> u16 {
    Command::new("dfx")
        .args(["info", service])
        .output()
        .expect("Failed to get dfx port")
        .stdout
        .iter()
        .map(|&b| b as char)
        .collect::<String>()
        .trim()
        .parse::<u16>()
        .expect("Failed to parse dfx port")
}

/// Returns the local EVM principal
fn local_evm_principal() -> String {
    let principal = Command::new("dfx")
        .args(["canister", "id", "evm_rpc"])
        .current_dir(CARGO_MANIFEST_DIR)
        .output()
        .expect("Failed to get evm_rpc canister id")
        .stdout
        .iter()
        .map(|&b| b as char)
        .collect::<String>()
        .trim()
        .to_string();

    // Verify the principal
    Principal::from_text(&principal)
        .expect("Invalid principal")
        .to_text()
}
