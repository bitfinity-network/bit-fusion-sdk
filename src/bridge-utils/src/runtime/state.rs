pub mod config;

use std::cell::RefCell;
use std::rc::Rc;

use ic_stable_structures::stable_structures::Memory;

use self::config::ConfigStorage;
use crate::bridge::Operation;
use crate::operation_store::OperationStore;
use crate::signer::SignerStorage;

pub type SharedConfig<Mem> = Rc<RefCell<ConfigStorage<Mem>>>;

pub struct StateMemory<Mem> {
    pub signer_memory: Mem,
    pub incomplete_operations: Mem,
    pub operations_log: Mem,
    pub operations_map: Mem,
}

pub struct State<Mem: Memory, Op: Operation> {
    pub config: SharedConfig<Mem>,
    pub signer: SignerStorage<Mem>,
    pub operations: OperationStore<Mem, Op>,
}

impl<Mem: Memory, Op: Operation> State<Mem, Op> {
    pub fn default(memory: StateMemory<Mem>, config: SharedConfig<Mem>) -> Self {
        Self {
            config,
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
