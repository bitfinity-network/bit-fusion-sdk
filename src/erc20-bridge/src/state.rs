use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use bridge_canister::bridge::OperationContext;
use bridge_canister::memory::{memory_by_id, StableMemory};
use bridge_canister::runtime::state::config::ConfigStorage;
use bridge_canister::runtime::state::SharedConfig;
use bridge_did::error::{BTFResult, Error};
use bridge_did::evm_link::EvmLink;
use bridge_did::init::erc20::{BaseEvmSettings, QueryDelays};
use bridge_utils::evm_bridge::EvmParams;
use candid::Principal;
use eth_signer::sign_strategy::TxSigner;
use ic_stable_structures::{CellStructure, StableCell};

use crate::memory::{BASE_EVM_CONFIG_MEMORY_ID, DELAYS_MEMORY_ID};

pub const BASE_EVM_DATA_REFRESH_TIMEOUT: Duration = Duration::from_secs(60);

/// Parameters of the Base EVM.
pub struct BaseEvmState {
    pub config: SharedConfig,
    pub delays: StableCell<QueryDelays, StableMemory>,
}

impl Default for BaseEvmState {
    fn default() -> Self {
        let config = ConfigStorage::default(memory_by_id(BASE_EVM_CONFIG_MEMORY_ID));
        Self {
            config: Rc::new(RefCell::new(config)),
            delays: StableCell::new(memory_by_id(DELAYS_MEMORY_ID), QueryDelays::default())
                .expect("failed to initialize delays cell"),
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
            config.btf_bridge_contract_address = None;
        });
        self.delays
            .set(settings.delays)
            .expect("failed to set delays");
    }
}

/// Newtype for base EVM state
#[derive(Default, Clone)]
pub struct SharedBaseEvmState(pub Rc<RefCell<BaseEvmState>>);

impl SharedBaseEvmState {
    pub fn query_delays(&self) -> QueryDelays {
        *self.0.borrow().delays.get()
    }
}

impl OperationContext for SharedBaseEvmState {
    fn get_evm_link(&self) -> EvmLink {
        self.0.borrow().config.borrow().get_evm_link()
    }

    fn get_bridge_contract_address(&self) -> BTFResult<did::H160> {
        self.0
            .borrow()
            .config
            .borrow()
            .get_btf_bridge_contract()
            .ok_or_else(|| Error::Initialization("base btf bridge contract not initialized".into()))
    }

    fn get_evm_params(&self) -> BTFResult<EvmParams> {
        self.0.borrow().config.borrow().get_evm_params()
    }

    fn get_signer(&self) -> BTFResult<TxSigner> {
        self.0.borrow().config.borrow().get_signer()
    }
}
