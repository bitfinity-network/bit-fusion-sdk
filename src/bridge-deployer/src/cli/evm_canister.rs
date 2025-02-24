use std::str::FromStr;

use candid::Principal;

use crate::evm::{MAINNET_PRINCIPAL, TESTNET_PRINCIPAL};

/// EVM canister option
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvmCanister {
    Mainnet,
    Testnet,
    Principal(Principal),
}

impl FromStr for EvmCanister {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mainnet" => Ok(Self::Mainnet),
            "testnet" => Ok(Self::Testnet),
            principal => {
                let principal = Principal::from_text(principal)
                    .map_err(|_| format!("Invalid principal: {principal}",))?;
                Ok(Self::Principal(principal))
            }
        }
    }
}

impl EvmCanister {
    /// Get evm principal
    pub fn principal(self) -> Principal {
        match self {
            Self::Mainnet => Principal::from_text(MAINNET_PRINCIPAL).expect("Invalid principal"),
            Self::Testnet => Principal::from_text(TESTNET_PRINCIPAL).expect("Invalid principal"),
            Self::Principal(principal) => principal,
        }
    }
}
