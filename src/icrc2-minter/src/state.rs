use access_list::AccessList;
use candid::Principal;
pub use config::Config;
pub use eth_signer::sign_strategy::{SigningStrategy, TransactionSigner};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{default_ic_memory_manager, VirtualMemory};

use crate::constant::ACCESS_LIST_MEMORY_ID;

mod access_list;
mod config;
pub mod log;

/// State of a minter canister.
pub struct State {
    /// Minter canister configuration.
    pub access_list: AccessList<VirtualMemory<DefaultMemoryImpl>>,
}

impl Default for State {
    fn default() -> Self {
        let memory_manager = default_ic_memory_manager();
        Self {
            access_list: AccessList::new(memory_manager.get(ACCESS_LIST_MEMORY_ID)),
        }
    }
}

/// State settings.
#[derive(Debug, Clone)]
pub struct Settings {
    pub owner: Principal,
    pub evm_principal: Principal,
    pub signing_strategy: SigningStrategy,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            owner: Principal::anonymous(),
            evm_principal: Principal::anonymous(),
            signing_strategy: SigningStrategy::Local {
                private_key: [218u8; 32],
            },
        }
    }
}
