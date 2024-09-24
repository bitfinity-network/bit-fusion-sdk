use bridge_did::init::brc20::Brc20BridgeConfig;
use ic_stable_structures::stable_structures::Memory;
use ic_stable_structures::{CellStructure, MemoryId, MemoryManager, StableCell};

use crate::memory::CONFIG_MEMORY_ID;

pub struct Brc20BridgeConfigStorage<M: Memory> {
    config: StableCell<Brc20BridgeConfig, M>,
}

impl<M> Brc20BridgeConfigStorage<M>
where
    M: Memory,
{
    pub fn new(memory: &dyn MemoryManager<M, MemoryId>) -> Self {
        Self {
            config: StableCell::new(memory.get(CONFIG_MEMORY_ID), Brc20BridgeConfig::default())
                .expect("stable memory config initialization failed"),
        }
    }

    pub fn get(&self) -> &Brc20BridgeConfig {
        self.config.get()
    }

    pub fn set(&mut self, config: Brc20BridgeConfig) {
        self.config.set(config).expect("failed to set config");
    }

    pub fn with_borrow_mut<F>(&mut self, f: F)
    where
        F: FnOnce(&mut Brc20BridgeConfig),
    {
        let mut config = self.config.get().clone();

        f(&mut config);

        self.set(config);
    }
}
