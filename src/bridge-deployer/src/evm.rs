//! EVM related utilities for link params.

use bridge_did::evm_link::EvmLink;
use candid::Principal;

use crate::contracts::EvmNetwork;

const MAINNET_PRINCIPAL: &str = "i3jjb-wqaaa-aaaaa-qadrq-cai";
const TESTNET_PRINCIPAL: &str = "4fe7g-7iaaa-aaaak-aegcq-cai";

/// Returns the IC host based on the EVM network.
pub fn ic_host(evm_network: EvmNetwork) -> String {
    match evm_network {
        EvmNetwork::Localhost => "http://localhost:4943".to_string(),
        EvmNetwork::Mainnet => format!("https://{MAINNET_PRINCIPAL}.ic0.app"),
        EvmNetwork::Testnet => format!("https://{TESTNET_PRINCIPAL}.ic0.app"),
    }
}

/// Returns the EVM link based on the EVM network.
pub fn evm_link(evm_network: EvmNetwork) -> EvmLink {
    match evm_network {
        EvmNetwork::Localhost => EvmLink::Http("http://localhost:8545".to_string()),
        EvmNetwork::Mainnet => {
            EvmLink::Ic(Principal::from_text(MAINNET_PRINCIPAL).expect("Invalid principal"))
        }
        EvmNetwork::Testnet => {
            EvmLink::Ic(Principal::from_text(TESTNET_PRINCIPAL).expect("Invalid principal"))
        }
    }
}
