use bridge_did::init::RuneBridgeConfig;
use ic_stable_structures::stable_structures::Memory;
use ic_stable_structures::{CellStructure, MemoryId, MemoryManager, StableCell};

use crate::memory::CONFIG_MEMORY_ID;

pub struct RuneBridgeConfigStorage<M: Memory> {
    config: StableCell<RuneBridgeConfig, M>,
}

impl<M> RuneBridgeConfigStorage<M>
where
    M: Memory,
{
    pub fn new(memory: &dyn MemoryManager<M, MemoryId>) -> Self {
        Self {
            config: StableCell::new(memory.get(CONFIG_MEMORY_ID), RuneBridgeConfig::default())
                .expect("stable memory config initialization failed"),
        }
    }

    pub fn get(&self) -> &RuneBridgeConfig {
        self.config.get()
    }

    pub fn set(&mut self, config: RuneBridgeConfig) {
        self.config.set(config).expect("failed to set config");
    }

    pub fn with_borrow_mut<F>(&mut self, f: F)
    where
        F: FnOnce(&mut RuneBridgeConfig),
    {
        let mut config = self.config.get().clone();

        f(&mut config);

        self.set(config);
    }
}
