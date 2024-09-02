//! Memory ids for bridge
//!
//! DO NOT USE ANY MEMORY ID BELOW 10, since used by the sdk

use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{IcMemoryManager, MemoryId};

pub const CONFIG_MEMORY_ID: MemoryId = MemoryId::new(10);
pub const MASTER_KEY_MEMORY_ID: MemoryId = MemoryId::new(12);
pub const PENDING_TASKS_MEMORY_ID: MemoryId = MemoryId::new(13);
pub const LEDGER_MEMORY_ID: MemoryId = MemoryId::new(14);
pub const USED_UTXOS_REGISTRY_MEMORY_ID: MemoryId = MemoryId::new(15);

thread_local! {
    pub static MEMORY_MANAGER: IcMemoryManager<DefaultMemoryImpl> = IcMemoryManager::init(DefaultMemoryImpl::default());
}
