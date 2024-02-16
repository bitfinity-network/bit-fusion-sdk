use candid::CandidType;
use serde::{Deserialize, Serialize};

pub mod bft_bridge_api;
pub mod build_data;
pub mod mint_orders;
pub mod uniswap_api;
pub mod wrapped_token_api;

/// Determined side of the bridge.
#[derive(Debug, Copy, Clone, Serialize, Deserialize, CandidType, PartialEq, Eq)]
pub enum BridgeSide {
    Base = 0,
    Wrapped = 1,
}

impl BridgeSide {
    pub fn other(self) -> Self {
        match self {
            Self::Base => Self::Wrapped,
            Self::Wrapped => Self::Base,
        }
    }
}
