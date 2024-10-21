//! EVM related utilities for link params.

use std::process::Command;

use bridge_did::evm_link::EvmLink;
use candid::Principal;

use crate::contracts::EvmNetwork;

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
pub fn evm_link(evm_network: EvmNetwork, evm_principal: Option<Principal>) -> EvmLink {
    match evm_network {
        EvmNetwork::Localhost => EvmLink::Ic(evm_principal.unwrap_or_else(local_evm_principal)),
        EvmNetwork::Mainnet => EvmLink::Ic(evm_principal.unwrap_or_else(|| {
            Principal::from_text(MAINNET_PRINCIPAL).expect("Invalid principal")
        })),
        EvmNetwork::Testnet => EvmLink::Ic(evm_principal.unwrap_or_else(|| {
            Principal::from_text(TESTNET_PRINCIPAL).expect("Invalid principal")
        })),
    }
}

/// Returns local dfx replica port
fn dfx_replica_port() -> u16 {
    dfx_info_port("replica-port")
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
fn local_evm_principal() -> Principal {
    let principal = Command::new("dfx")
        .args(["canister", "id", "evm_testnet"])
        .output()
        .expect("Failed to get evm_testnet canister id")
        .stdout
        .iter()
        .map(|&b| b as char)
        .collect::<String>()
        .trim()
        .to_string();

    // Verify the principal
    Principal::from_text(&principal).expect("Invalid principal")
}
