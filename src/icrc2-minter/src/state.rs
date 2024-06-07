use std::cell::RefCell;

use candid::Principal;
pub use config::Config;
use did::H160;
pub use eth_signer::sign_strategy::{SigningStrategy, TransactionSigner};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{default_ic_memory_manager, CellStructure, StableCell, VirtualMemory};
use minter_contract_utils::mint_orders::MintOrders;

use self::log::LoggerConfigService;
use self::signer::SignerInfo;
use crate::constant::{MINT_ORDERS_MEMORY_ID, NONCES_COUNTER_MEMORY_ID};
use crate::memory::MEMORY_MANAGER;

mod config;
pub mod log;
mod signer;

/// State of a minter canister.
pub struct State {
    /// Minter canister configuration.
    pub config: Config,

    /// Transaction signing info.
    pub signer: SignerInfo,

    /// Signed mint orders.
    pub mint_orders: MintOrders<VirtualMemory<DefaultMemoryImpl>>,

    pub logger_config_service: LoggerConfigService,
}

impl Default for State {
    fn default() -> Self {
        let memory_manager = default_ic_memory_manager();
        Self {
            config: Config::default(),
            signer: SignerInfo::default(),
            mint_orders: MintOrders::new(memory_manager.get(MINT_ORDERS_MEMORY_ID)),
            logger_config_service: LoggerConfigService::default(),
        }
    }
}

impl State {
    /// Clear the state and set initial data from settings.
    pub fn reset(&mut self, settings: Settings) {
        self.signer
            .reset(settings.signing_strategy.clone(), 0)
            .expect("failed to set signer");
        self.config.reset(settings);
        self.mint_orders.clear();
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
    pub signing_strategy: SigningStrategy,
    pub fee_charge_contract: H160,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            owner: Principal::anonymous(),
            evm_principal: Principal::anonymous(),
            signing_strategy: SigningStrategy::Local {
                private_key: [218u8; 32],
            },
            fee_charge_contract: H160::default(),
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
