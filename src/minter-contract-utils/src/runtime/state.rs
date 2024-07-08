pub mod config;

use ic_stable_structures::stable_structures::Memory;

use self::config::ConfigStorage;
use crate::bridge::Operation;
use crate::operation_store::OperationStore;
use crate::signer::SignerStorage;

pub struct StateMemory<Mem> {
    pub config_memory: Mem,
    pub signer_memory: Mem,
    pub incomplete_operations: Mem,
    pub operations_log: Mem,
    pub operations_map: Mem,
}

pub struct State<Mem: Memory, Op: Operation> {
    pub config: ConfigStorage<Mem>,
    pub signer: SignerStorage<Mem>,
    pub operations: OperationStore<Mem, Op>,
}

impl<Mem: Memory, Op: Operation> State<Mem, Op> {
    fn default(memory: StateMemory<Mem>) -> Self {
        Self {
            config: ConfigStorage::default(memory.config_memory),
            signer: SignerStorage::default(memory.signer_memory),
            operations: OperationStore::with_memory(
                memory.incomplete_operations,
                memory.operations_log,
                memory.operations_map,
                None,
            ),
        }
    }
}
