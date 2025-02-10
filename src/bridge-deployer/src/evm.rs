//! EVM related utilities for link params.

use std::process::Command;

use bridge_did::evm_link::EvmLink;
use candid::Principal;

use crate::contracts::IcNetwork;

const MAINNET_PRINCIPAL: &str = "i3jjb-wqaaa-aaaaa-qadrq-cai";
const TESTNET_PRINCIPAL: &str = "4fe7g-7iaaa-aaaak-aegcq-cai";

/// Returns the IC host based on the EVM network.
pub fn ic_host(ic_network: IcNetwork) -> String {
    match ic_network {
        IcNetwork::Localhost => format!("http://127.0.0.1:{}", dfx_replica_port()),
        IcNetwork::Mainnet => format!("https://{MAINNET_PRINCIPAL}.ic0.app"),
        IcNetwork::Testnet => format!("https://{TESTNET_PRINCIPAL}.ic0.app"),
    }
}

/// Returns the EVM link based on the EVM network.
pub fn evm_link(
    evm_rpc: Option<String>,
    ic_network: IcNetwork,
    evm_principal: Option<Principal>,
) -> EvmLink {
    match (evm_rpc, ic_network, evm_principal) {
        (Some(evm_rpc), _, _) => EvmLink::Http(evm_rpc),
        (None, evm_network, principal) => {
            EvmLink::Ic(evm_principal_or_default(evm_network, principal))
        }
    }
}

/// Get the EVM principal or default based on the EVM network.
pub fn evm_principal_or_default(
    evm_network: IcNetwork,
    evm_principal: Option<Principal>,
) -> Principal {
    match evm_principal {
        Some(principal) => principal,
        None => match evm_network {
            IcNetwork::Localhost => local_evm_principal(),
            IcNetwork::Mainnet => {
                Principal::from_text(MAINNET_PRINCIPAL).expect("Invalid principal")
            }
            IcNetwork::Testnet => {
                Principal::from_text(TESTNET_PRINCIPAL).expect("Invalid principal")
            }
        },
    }
}

/// Returns local dfx replica port
fn dfx_replica_port() -> u16 {
    dfx_info_port("replica-port")
}

/// Returns local dfx replica port
pub fn dfx_webserver_port() -> u16 {
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
pub fn local_evm_principal() -> Principal {
    let principal = Command::new("dfx")
        .args(["canister", "id", "evm"])
        .output()
        .expect("Failed to get evm canister id")
        .stdout
        .iter()
        .map(|&b| b as char)
        .collect::<String>()
        .trim()
        .to_string();

    if principal.is_empty() {
        panic!("Local evm canister is not found.")
    }

    // Verify the principal
    Principal::from_text(&principal).expect("Invalid principal")
}
