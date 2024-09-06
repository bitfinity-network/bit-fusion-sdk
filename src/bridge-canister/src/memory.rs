use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{IcMemoryManager, MemoryId, VirtualMemory};

pub const SIGNER_MEMORY_ID: MemoryId = MemoryId::new(0);
pub const CONFIG_MEMORY_ID: MemoryId = MemoryId::new(1);
pub const OPERATIONS_ID_COUNTER_MEMORY_ID: MemoryId = MemoryId::new(2);
pub const OPERATIONS_MEMORY_ID: MemoryId = MemoryId::new(3);
pub const OPERATIONS_LOG_MEMORY_ID: MemoryId = MemoryId::new(4);
pub const OPERATIONS_MAP_MEMORY_ID: MemoryId = MemoryId::new(5);
pub const PENDING_TASKS_MEMORY_ID: MemoryId = MemoryId::new(6);
pub const LOG_SETTINGS_MEMORY_ID: MemoryId = MemoryId::new(7);
pub const MEMO_OPERATION_MEMORY_ID: MemoryId = MemoryId::new(8);
pub const PENDING_TASKS_SEQUENCE_MEMORY_ID: MemoryId = MemoryId::new(9);

pub type StableMemory = VirtualMemory<DefaultMemoryImpl>;

thread_local! {
    /// Memory manager.
    pub static MEMORY_MANAGER: IcMemoryManager<DefaultMemoryImpl> = IcMemoryManager::init(DefaultMemoryImpl::default());
}

/// Get memory by id.
pub fn memory_by_id(id: MemoryId) -> StableMemory {
    MEMORY_MANAGER.with(|mm| mm.get(id))
}
