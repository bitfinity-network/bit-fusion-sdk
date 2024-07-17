pub mod scheduler;
pub mod state;

use std::cell::RefCell;
use std::rc::Rc;

use bridge_did::error::{BftResult, Error};
use bridge_utils::evm_bridge::EvmParams;
use bridge_utils::evm_link::EvmLink;
use eth_signer::sign_strategy::TransactionSigner;
use ic_stable_structures::StableBTreeMap;
use ic_storage::IcStorage;
use ic_task_scheduler::scheduler::TaskScheduler;
use ic_task_scheduler::task::ScheduledTask;

use self::scheduler::{BridgeScheduler, ServiceTask};
use self::state::config::ConfigStorage;
use self::state::{SharedConfig, State};
use crate::bridge::{Operation, OperationContext};
use crate::memory::{
    memory_by_id, StableMemory, CONFIG_MEMORY_ID, OPERATIONS_ID_COUNTER_MEMORY_ID,
    OPERATIONS_LOG_MEMORY_ID, OPERATIONS_MAP_MEMORY_ID, OPERATIONS_MEMORY_ID,
    PENDING_TASKS_MEMORY_ID,
};
use crate::operation_store::OperationsMemory;

pub type RuntimeState<Op> = Rc<RefCell<State<Op>>>;

pub struct BridgeRuntime<Op: Operation> {
    state: RuntimeState<Op>,
    scheduler: BridgeScheduler<StableMemory, Op>,
}

impl<Op: Operation> BridgeRuntime<Op> {
    pub fn default(config: SharedConfig) -> Self {
        let tasks_storage = StableBTreeMap::new(memory_by_id(PENDING_TASKS_MEMORY_ID));
        Self {
            state: default_state(config),
            scheduler: BridgeScheduler::new(tasks_storage),
        }
    }

    pub fn update_state(&mut self, f: impl FnOnce(&mut State<Op>)) {
        let mut state = self.state.borrow_mut();
        f(&mut state);
    }

    pub fn run(&mut self) {
        if !self.state.borrow().collecting_logs {
            self.state.borrow_mut().collecting_logs = true;
            let task = scheduler::BridgeTask::Service(ServiceTask::CollectEvmLogs);
            let collect_logs = ScheduledTask::new(task);
            self.scheduler.append_task(collect_logs);
        }

        if !self.state.borrow().refreshing_evm_params {
            self.state.borrow_mut().refreshing_evm_params = true;
            let task = scheduler::BridgeTask::Service(ServiceTask::RefreshEvmParams);
            let refresh_evm_params = ScheduledTask::new(task);
            self.scheduler.append_task(refresh_evm_params);
        }

        let task_execution_result = self.scheduler.run(self.state.clone());

        if let Err(err) = task_execution_result {
            log::error!("task execution failed: {err}",);
        }
    }

    pub fn state(&self) -> &RuntimeState<Op> {
        &self.state
    }
}

impl<Op: Operation> OperationContext for RuntimeState<Op> {
    fn get_evm_link(&self) -> EvmLink {
        self.borrow().config.borrow().get_evm_link()
    }

    fn get_bridge_contract_address(&self) -> BftResult<did::H160> {
        self.borrow()
            .config
            .borrow()
            .get_bft_bridge_contract()
            .ok_or_else(|| Error::Initialization("bft bridge contract not initialized".into()))
    }

    fn get_evm_params(&self) -> BftResult<EvmParams> {
        self.borrow().config.borrow().get_evm_params()
    }

    fn get_signer(&self) -> BftResult<impl TransactionSigner> {
        self.borrow().config.borrow().get_signer()
    }
}

impl IcStorage for ConfigStorage {
    fn get() -> SharedConfig {
        CONFIG_STORAGE.with(|cell| cell.clone())
    }
}

thread_local! {
    pub static CONFIG_STORAGE: SharedConfig =
        Rc::new(RefCell::new(ConfigStorage::default(memory_by_id(CONFIG_MEMORY_ID))));
}

fn operation_storage_memory() -> OperationsMemory<StableMemory> {
    OperationsMemory {
        id_counter: memory_by_id(OPERATIONS_ID_COUNTER_MEMORY_ID),
        incomplete_operations: memory_by_id(OPERATIONS_MEMORY_ID),
        operations_log: memory_by_id(OPERATIONS_LOG_MEMORY_ID),
        operations_map: memory_by_id(OPERATIONS_MAP_MEMORY_ID),
    }
}

fn default_state<Op: Operation>(config: SharedConfig) -> RuntimeState<Op> {
    let state = State::default(operation_storage_memory(), config);
    Rc::new(RefCell::new(state))
}
