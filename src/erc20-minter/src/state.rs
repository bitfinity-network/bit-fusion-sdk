use std::time::Duration;

use bridge_canister::memory::{memory_by_id, StableMemory};
use bridge_canister::runtime::state::config::ConfigStorage;
use bridge_canister::runtime::state::Timestamp;
use bridge_utils::evm_link::EvmLink;
use candid::{CandidType, Principal};
use eth_signer::sign_strategy::SigningStrategy;
use ic_exports::ic_kit::ic;
use ic_stable_structures::{CellStructure, StableCell};
use serde::Deserialize;

use crate::memory::BASE_EVM_CONFIG_MEMORY_ID;

pub const BASE_EVM_DATA_REFRESH_TIMEOUT: Duration = Duration::from_secs(60);

/// Parameters of the Base EVM.
pub struct BaseEvmState {
    pub config: ConfigStorage,
    pub collecting_logs_ts: Option<Timestamp>,
    pub refreshing_evm_params_ts: Option<Timestamp>,
}

impl Default for BaseEvmState {
    fn default() -> Self {
        Self {
            config: ConfigStorage::default(memory_by_id(BASE_EVM_CONFIG_MEMORY_ID)),
            collecting_logs_ts: None,
            refreshing_evm_params_ts: None,
        }
    }
}

impl BaseEvmState {
    /// Reset the state using the given settings.
    pub fn reset(&mut self, settings: BaseEvmSettings) {
        self.config.update(|config| {
            config.owner = Principal::anonymous();
            config.evm_link = settings.evm_link;
            config.signing_strategy = settings.signing_strategy;
            config.evm_params = None;
            config.bft_bridge_contract_address = None;
        })
    }

    /// Checks if the EVM parameters should be refreshed.
    ///
    /// The EVM parameters are refreshed if the `refreshing_evm_params_ts` timestamp is older than the `TASK_LOCK_TIMEOUT` duration, or if the `refreshing_evm_params_ts` is `None`.
    pub fn should_refresh_evm_params(&self) -> bool {
        self.refreshing_evm_params_ts
            .map(|ts| (ts + BASE_EVM_DATA_REFRESH_TIMEOUT.as_nanos() as u64) <= ic::time())
            .unwrap_or(true)
    }

    /// Checks if the EVM logs should be collected.
    ///
    /// The EVM logs are collected if the `collecting_logs_ts` timestamp is older than the `BASE_EVM_DATA_REFRESH_TIMEOUT` duration, or if the `collecting_logs_ts` is `None`.
    pub fn should_collect_evm_logs(&self) -> bool {
        self.collecting_logs_ts
            .map(|ts| (ts + BASE_EVM_DATA_REFRESH_TIMEOUT.as_nanos() as u64) <= ic::time())
            .unwrap_or(true)
    }
}

#[derive(Debug, Clone, Deserialize, CandidType)]
pub struct BaseEvmSettings {
    pub evm_link: EvmLink,
    pub signing_strategy: SigningStrategy,
}
