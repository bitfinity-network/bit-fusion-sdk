pub mod config;
mod task_lock;

use std::cell::RefCell;
use std::rc::Rc;

use task_lock::TaskLock;

use self::config::ConfigStorage;
use crate::bridge::Operation;
use crate::memory::StableMemory;
use crate::operation_store::{OperationStore, OperationsMemory};

pub type SharedConfig = Rc<RefCell<ConfigStorage>>;

/// Bridge Runtime state.
pub struct State<Op: Operation> {
    pub config: SharedConfig,
    pub operations: OperationStore<StableMemory, Op>,
    pub collecting_logs: TaskLock,
    pub refreshing_evm_params: TaskLock,
}

impl<Op: Operation> State<Op> {
    /// Load the state from the stable memory, or initialize it with default values.
    pub fn default(memory: OperationsMemory<StableMemory>, config: SharedConfig) -> Self {
        Self {
            config,
            operations: OperationStore::with_memory(memory, None),
            collecting_logs: TaskLock::default(),
            refreshing_evm_params: TaskLock::default(),
        }
    }
}
