pub mod config;

use ic_stable_structures::stable_structures::Memory;

use crate::signer::SignerStorage;

use self::config::ConfigStorage;

pub struct StateMemory<Mem: Memory> {
    pub config_memory: Mem,
    pub signer_memory: Mem,
}

pub struct State<Mem: Memory> {
    pub config: ConfigStorage<Mem>,
    pub signer: SignerStorage<Mem>,
}

impl<Mem: Memory> State<Mem> {
    fn default(memory: StateMemory<Mem>) -> Self {
        Self {
            config: ConfigStorage::default(memory.config_memory),
            signer: SignerStorage::default(memory.signer_memory),
        }
    }
}
