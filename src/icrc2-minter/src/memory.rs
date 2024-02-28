use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::IcMemoryManager;

thread_local! {
    pub static MEMORY_MANAGER: IcMemoryManager<DefaultMemoryImpl> = IcMemoryManager::init(DefaultMemoryImpl::default());
}
