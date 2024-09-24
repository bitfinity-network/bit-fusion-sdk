use candid::{CandidType, Principal};
use eth_signer::sign_strategy::SigningStrategy;
use ic_log::did::LogCanisterSettings;
use serde::Deserialize;

use crate::evm_link::EvmLink;

/// Bridge canister initialization data.
#[derive(Debug, Deserialize, CandidType, Clone)]
pub struct BridgeInitData {
    /// Principal of canister's owner.
    pub owner: Principal,

    /// Parameters for connecting to the EVM
    pub evm_link: EvmLink,

    /// Signing strategy
    pub signing_strategy: SigningStrategy,

    /// Log settings
    #[serde(default)]
    pub log_settings: Option<LogCanisterSettings>,
}
