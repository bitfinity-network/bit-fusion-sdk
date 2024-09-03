use candid::{CandidType, Principal};
use eth_signer::sign_strategy::SigningStrategy;
use ic_log::did::LogCanisterSettings;
use serde::Deserialize;

/// Bridge canister initialization data.
#[derive(Debug, Deserialize, CandidType, Clone)]
pub struct BridgeInitData {
    /// Principal of canister's owner.
    pub owner: Principal,

    /// Principal of EVM canister, in which bridge canister will withdraw/deposit tokens.
    pub evm_principal: Principal,

    /// Signing strategy
    pub signing_strategy: SigningStrategy,

    /// Log settings
    #[serde(default)]
    pub log_settings: Option<LogCanisterSettings>,
}
