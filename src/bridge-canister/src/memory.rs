use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{IcMemoryManager, MemoryId};

pub const CONFIG_MEMORY_ID: MemoryId = MemoryId::new(140);
pub const TX_SIGNER_MEMORY_ID: MemoryId = MemoryId::new(141);
pub const LOG_SETTINGS_MEMORY_ID: MemoryId = MemoryId::new(142);

thread_local! {
    pub static MEMORY_MANAGER: IcMemoryManager<DefaultMemoryImpl> = IcMemoryManager::init(DefaultMemoryImpl::default());
}
