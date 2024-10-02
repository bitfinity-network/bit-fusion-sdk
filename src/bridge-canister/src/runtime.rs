pub mod scheduler;
pub mod service;
pub mod state;

use std::cell::RefCell;
use std::rc::Rc;

use bridge_did::error::{BftResult, Error};
use bridge_did::evm_link::EvmLink;
use bridge_did::op_id::OperationId;
use bridge_utils::evm_bridge::EvmParams;
use eth_signer::sign_strategy::TransactionSigner;
use ic_exports::ic_kit::ic;
use ic_stable_structures::{StableBTreeMap, StableCell};
use ic_storage::IcStorage;
use ic_task_scheduler::scheduler::TaskScheduler;
use ic_task_scheduler::task::ScheduledTask;
use jsonrpc_core::futures;

use self::scheduler::{BridgeTask, ServiceTask, SharedScheduler};
use self::service::{DynService, ServiceOrder};
use self::state::config::ConfigStorage;
use self::state::{SharedConfig, State};
use crate::bridge::{Operation, OperationContext};
use crate::memory::{
    memory_by_id, StableMemory, CONFIG_MEMORY_ID, MEMO_OPERATION_MEMORY_ID,
    OPERATIONS_ID_COUNTER_MEMORY_ID, OPERATIONS_LOG_MEMORY_ID, OPERATIONS_MAP_MEMORY_ID,
    OPERATIONS_MEMORY_ID, PENDING_TASKS_MEMORY_ID, PENDING_TASKS_SEQUENCE_MEMORY_ID,
};
use crate::operation_store::OperationsMemory;

pub type RuntimeState<Op> = Rc<RefCell<State<Op>>>;

/// Bridge Runtime.
/// Stores a state, schedules tasks and executes them.
pub struct BridgeRuntime<Op: Operation> {
    state: RuntimeState<Op>,
    scheduler: SharedScheduler<StableMemory, Op>,
}

impl<Op: Operation> BridgeRuntime<Op> {
    /// Load the state from the stable memory, or initialize it with default values.
    pub fn default(config: SharedConfig) -> Self {
        let tasks_storage = StableBTreeMap::new(memory_by_id(PENDING_TASKS_MEMORY_ID));
        let sequence = StableCell::new(memory_by_id(PENDING_TASKS_SEQUENCE_MEMORY_ID), 1_000_000)
            .expect("Cannot create task sequence cell");
        Self {
            state: default_state(config),
            scheduler: SharedScheduler::new(tasks_storage, sequence),
        }
    }

    /// Updates the state.
    pub fn update_state(&mut self, f: impl FnOnce(&mut State<Op>)) {
        let mut state = self.state.borrow_mut();
        f(&mut state);
    }

    /// Provides access to tasks scheduler.
    pub fn schedule_operation(&self, id: OperationId, operation: Op) {
        let options = operation.scheduling_options().unwrap_or_default();
        let scheduled_task =
            ScheduledTask::with_options(BridgeTask::Operation(id, operation), options);
        self.scheduler.append_task(scheduled_task);
    }

    /// Run the scheduled tasks.
    pub fn run(&mut self) {
        if self.state.borrow().should_collect_evm_logs() {
            self.state.borrow_mut().collecting_logs_ts = Some(ic::time());

            let task = scheduler::BridgeTask::Service(ServiceTask::CollectEvmLogs);
            let collect_logs = ScheduledTask::new(task);
            self.scheduler.append_task(collect_logs);
        }

        if self.state.borrow().should_refresh_evm_params() {
            self.state.borrow_mut().refreshing_evm_params_ts = Some(ic::time());

            let task = scheduler::BridgeTask::Service(ServiceTask::RefreshEvmParams);
            let refresh_evm_params = ScheduledTask::new(task);
            self.scheduler.append_task(refresh_evm_params);
        }

        if !self.state.borrow().should_process_operations() {
            return;
        }

        let services_before_ops = self.list_services(ServiceOrder::BeforeOperations);
        let services_after_ops = self.list_services(ServiceOrder::ConcurrentWithOperations);
        let scheduler = self.scheduler.clone();
        let state = self.state.clone();
        state.borrow_mut().operations_run_ts = Some(ic::time());

        ic::spawn(async move {
            let _guard = drop_guard::guard(state.clone(), |state| {
                state.borrow_mut().operations_run_ts = None
            });

            Self::run_services(services_before_ops).await;

            let task_execution_result = scheduler.run(state);
            if let Err(err) = task_execution_result {
                log::error!("task execution failed: {err}",);
            }

            Self::run_services(services_after_ops).await;
        });
    }

    /// Get the state.
    pub fn state(&self) -> &RuntimeState<Op> {
        &self.state
    }

    /// Get the state.
    pub fn scheduler(&self) -> &SharedScheduler<StableMemory, Op> {
        &self.scheduler
    }

    fn list_services(&self, order: ServiceOrder) -> Vec<DynService> {
        let state = self.state.borrow();
        let services = state.services.borrow();
        services.services(order).values().cloned().collect()
    }

    async fn run_services(services: Vec<DynService>) {
        let mut futures = vec![];
        for service in services {
            let future = Box::pin(async move {
                if let Err(e) = service.run().await {
                    log::warn!("service returned an error: {e}");
                }
            });
            futures.push(future);
        }
        futures::future::join_all(futures).await;
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
        memo_operations_map: memory_by_id(MEMO_OPERATION_MEMORY_ID),
    }
}

pub(crate) fn default_state<Op: Operation>(config: SharedConfig) -> RuntimeState<Op> {
    let state = State::default(operation_storage_memory(), config);
    Rc::new(RefCell::new(state))
}
