use std::cell::RefCell;
use std::time::Duration;

use candid::Principal;
pub use config::Config;
use did::{H160, U256};
pub use eth_signer::sign_strategy::{SigningStrategy, TransactionSigner};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{default_ic_memory_manager, CellStructure, StableCell, VirtualMemory};
use minter_contract_utils::mint_orders::MintOrders;

use self::log::LoggerConfigService;
use self::operation_points::OperationPoints;
use self::signer::SignerInfo;
use crate::constant::{
    DEFAULT_CHAIN_ID, DEFAULT_GAS_PRICE, MINT_ORDERS_MEMORY_ID, NONCES_COUNTER_MEMORY_ID,
};
use crate::memory::MEMORY_MANAGER;

mod config;
pub mod log;
pub mod operation_points;
mod signer;

/// State of a minter canister.
pub struct State {
    /// Minter canister configuration.
    pub config: Config,

    /// Transaction signing info.
    pub signer: SignerInfo,

    /// Signed mint orders.
    pub mint_orders: MintOrders<VirtualMemory<DefaultMemoryImpl>>,

    /// Operation points. Used as fee for expensive operations.
    pub operation_points: OperationPoints,

    pub logger_config_service: LoggerConfigService,
}

impl Default for State {
    fn default() -> Self {
        let config = Config::default();
        let memory_manager = default_ic_memory_manager();
        Self {
            config: Config::default(),
            signer: SignerInfo::default(),
            mint_orders: MintOrders::new(&memory_manager, MINT_ORDERS_MEMORY_ID),
            logger_config_service: LoggerConfigService::default(),
            operation_points: OperationPoints::new(config.get_owner()),
        }
    }
}

impl State {
    /// Clear the state and set initial data from settings.
    pub fn reset(&mut self, settings: Settings) {
        self.operation_points = OperationPoints::new(settings.owner);
        self.signer
            .reset(settings.signing_strategy.clone(), settings.chain_id)
            .expect("failed to set signer");
        self.config.reset(settings);
        self.mint_orders.clear();
        self.operation_points.clear();
        NONCES_COUNTER
            .with(|cell| cell.borrow_mut().set(0))
            .expect("failed to reset nonce counter");
    }

    /// Returns unique nonce and increases the counter.
    pub fn next_nonce(&mut self) -> u32 {
        NONCES_COUNTER.with(|cell| {
            let mut cell = cell.borrow_mut();
            let nonce = *cell.get();
            cell.set(nonce + 1).expect("failed to update nonce counter");
            nonce
        })
    }
}

thread_local! {
    static NONCES_COUNTER: RefCell<StableCell<u32, VirtualMemory<DefaultMemoryImpl>>> =
        RefCell::new(StableCell::new(MEMORY_MANAGER.with(|mm| mm.get(NONCES_COUNTER_MEMORY_ID)), 0)
            .expect("failed to initialize nonces cell"));
}

/// State settings.
#[derive(Debug, Clone)]
pub struct Settings {
    pub owner: Principal,
    pub evm_principal: Principal,
    pub evm_gas_price: U256,
    pub signing_strategy: SigningStrategy,
    pub chain_id: u32,
    pub bft_bridge_contract: Option<H160>,
    pub spender_principal: Principal,
    pub process_transactions_results_interval: Option<Duration>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            owner: Principal::anonymous(),
            evm_principal: Principal::anonymous(),
            evm_gas_price: DEFAULT_GAS_PRICE.into(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            chain_id: DEFAULT_CHAIN_ID,
            bft_bridge_contract: None,
            spender_principal: Principal::anonymous(),
            process_transactions_results_interval: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use ic_exports::ic_kit::MockContext;

    use super::*;

    #[test]
    fn nonce_counter_works() {
        MockContext::new().inject();
        let mut state = State::default();
        let nonces: HashSet<_> = (0..20).map(|_| state.next_nonce()).collect();
        assert_eq!(nonces.len(), 20)
    }
}
