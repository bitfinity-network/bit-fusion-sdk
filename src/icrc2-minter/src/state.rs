use access_list::AccessList;
pub use eth_signer::sign_strategy::{SigningStrategy, TransactionSigner};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{default_ic_memory_manager, VirtualMemory};

use crate::constant::ACCESS_LIST_MEMORY_ID;

mod access_list;

/// State of a minter canister.
pub struct IcrcState {
    /// Minter canister configuration.
    pub access_list: AccessList<VirtualMemory<DefaultMemoryImpl>>,
}

impl Default for IcrcState {
    fn default() -> Self {
        let memory_manager = default_ic_memory_manager();
        Self {
            access_list: AccessList::new(memory_manager.get(ACCESS_LIST_MEMORY_ID)),
        }
    }
}
