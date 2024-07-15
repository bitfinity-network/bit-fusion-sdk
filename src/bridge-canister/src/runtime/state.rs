pub mod config;

use std::cell::RefCell;
use std::rc::Rc;

use self::config::ConfigStorage;
use crate::bridge::Operation;
use crate::memory::StableMemory;
use crate::operation_store::{OperationStore, OperationsMemory};

pub type SharedConfig = Rc<RefCell<ConfigStorage>>;

pub struct State<Op: Operation> {
    pub config: SharedConfig,
    pub operations: OperationStore<StableMemory, Op>,
}

impl<Op: Operation> State<Op> {
    pub fn default(memory: OperationsMemory<StableMemory>, config: SharedConfig) -> Self {
        Self {
            config,
            operations: OperationStore::with_memory(memory, None),
        }
    }
}
