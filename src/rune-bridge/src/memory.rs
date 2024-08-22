use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{IcMemoryManager, MemoryId};

pub const CONFIG_MEMORY_ID: MemoryId = MemoryId::new(10);
pub const PENDING_TASKS_MEMORY_ID: MemoryId = MemoryId::new(11);
pub const SIGNER_MEMORY_ID: MemoryId = MemoryId::new(12);
pub const LOGGER_SETTINGS_MEMORY_ID: MemoryId = MemoryId::new(13);
pub const BURN_REQUEST_MEMORY_ID: MemoryId = MemoryId::new(14);
pub const LEDGER_MEMORY_ID: MemoryId = MemoryId::new(15);
pub const USED_UTXOS_REGISTRY_MEMORY_ID: MemoryId = MemoryId::new(16);
pub const RUNE_INFO_BY_UTXO_MEMORY_ID: MemoryId = MemoryId::new(17);
pub const OPERATIONS_MEMORY_ID: MemoryId = MemoryId::new(18);
pub const OPERATIONS_LOG_MEMORY_ID: MemoryId = MemoryId::new(19);
pub const OPERATIONS_MAP_MEMORY_ID: MemoryId = MemoryId::new(20);
pub const OPERATIONS_COUNTER_MEMORY_ID: MemoryId = MemoryId::new(21);

thread_local! {
    pub static MEMORY_MANAGER: IcMemoryManager<DefaultMemoryImpl> = IcMemoryManager::init(DefaultMemoryImpl::default());
}
