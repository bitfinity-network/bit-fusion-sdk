use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use bridge_canister::bridge::OperationContext;
use bridge_canister::memory::{memory_by_id, StableMemory};
use bridge_canister::runtime::state::config::ConfigStorage;
use bridge_canister::runtime::state::Timestamp;
use bridge_did::error::{BftResult, Error};
use bridge_utils::evm_bridge::EvmParams;
use bridge_utils::evm_link::EvmLink;
use candid::{CandidType, Principal};
use drop_guard::guard;
use eth_signer::sign_strategy::{SigningStrategy, TransactionSigner};
use ic_exports::ic_kit::ic;
use ic_stable_structures::{CellStructure, StableCell};
use serde::Deserialize;

use crate::memory::{BASE_EVM_CONFIG_MEMORY_ID, NONCE_COUNTER_MEMORY_ID};

pub const BASE_EVM_DATA_REFRESH_TIMEOUT: Duration = Duration::from_secs(60);

/// Parameters of the Base EVM.
pub struct BaseEvmState {
    pub config: Rc<RefCell<ConfigStorage>>,
    pub nonce: StableCell<u32, StableMemory>,
    pub collecting_logs_ts: Option<Timestamp>,
    pub refreshing_evm_params_ts: Option<Timestamp>,
}

impl Default for BaseEvmState {
    fn default() -> Self {
        let config = ConfigStorage::default(memory_by_id(BASE_EVM_CONFIG_MEMORY_ID));
        Self {
            config: Rc::new(RefCell::new(config)),
            nonce: StableCell::new(memory_by_id(NONCE_COUNTER_MEMORY_ID), 0)
                .expect("failed to initialize nonce counter"),
            collecting_logs_ts: None,
            refreshing_evm_params_ts: None,
        }
    }
}

impl BaseEvmState {
    /// Reset the state using the given settings.
    pub fn reset(&mut self, settings: BaseEvmSettings) {
        self.config.borrow_mut().update(|config| {
            config.owner = Principal::anonymous();
            config.evm_link = settings.evm_link;
            config.signing_strategy = settings.signing_strategy;
            config.evm_params = None;
            config.bft_bridge_contract_address = None;
        })
    }

    /// Returns unique nonce value.
    pub fn next_nonce(&mut self) -> u32 {
        let value = *self.nonce.get();
        self.nonce.set(value + 1).expect("failed to update nonce");
        value
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

/// Newtype for base EVM state
#[derive(Default, Clone)]
pub struct SharedEvmState(pub Rc<RefCell<BaseEvmState>>);

impl SharedEvmState {
    pub async fn refresh_base_evm_params(self) {
        let _lock = guard(self.0.clone(), |s| s.borrow_mut().collecting_logs_ts = None);
        let config = self.0.borrow().config.clone();
        if let Err(e) = ConfigStorage::refresh_evm_params(config).await {
            log::warn!("failed to refresh base EVM params: {e}");
        };
    }
}

impl OperationContext for SharedEvmState {
    fn get_evm_link(&self) -> EvmLink {
        self.0.borrow().config.borrow().get_evm_link()
    }

    fn get_bridge_contract_address(&self) -> BftResult<did::H160> {
        self.0
            .borrow()
            .config
            .borrow()
            .get_bft_bridge_contract()
            .ok_or_else(|| Error::Initialization("base bft bridge contract not initialized".into()))
    }

    fn get_evm_params(&self) -> BftResult<EvmParams> {
        self.0.borrow().config.borrow().get_evm_params()
    }

    fn get_signer(&self) -> BftResult<impl TransactionSigner> {
        self.0.borrow().config.borrow().get_signer()
    }

    fn increment_nonce(&self) {
        self.0
            .borrow()
            .config
            .borrow_mut()
            .update_evm_params(|p| p.nonce += 1);
    }
}

#[derive(Debug, Clone, Deserialize, CandidType)]
pub struct BaseEvmSettings {
    pub evm_link: EvmLink,
    pub signing_strategy: SigningStrategy,
}
