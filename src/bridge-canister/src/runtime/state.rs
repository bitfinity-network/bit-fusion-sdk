pub mod config;

use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use bridge_did::error::BftResult;
use bridge_did::op_id::OperationId;
use ic_exports::ic_kit::ic;

use self::config::ConfigStorage;
use super::service::{ServiceId, Services};
use crate::bridge::Operation;
use crate::memory::StableMemory;
use crate::operation_store::{OperationStore, OperationsMemory};

const SYS_TASK_LOCK_TIMEOUT: Duration = Duration::from_secs(60);
const SCHEDULER_RUN_LOCK_TIMEOUT: Duration = Duration::from_secs(60);

pub type SharedConfig = Rc<RefCell<ConfigStorage>>;
pub type SharedServices = Rc<RefCell<Services>>;

pub type Timestamp = u64;

/// Bridge Runtime state.
pub struct State<Op: Operation> {
    pub config: SharedConfig,
    pub operations: OperationStore<StableMemory, Op>,
    pub collecting_logs_ts: Option<Timestamp>,
    pub refreshing_evm_params_ts: Option<Timestamp>,
    pub operations_run_ts: Option<Timestamp>,
    pub services: SharedServices,
}

impl<Op: Operation> State<Op> {
    /// Load the state from the stable memory, or initialize it with default values.
    pub fn default(memory: OperationsMemory<StableMemory>, config: SharedConfig) -> Self {
        Self {
            config,
            operations: OperationStore::with_memory(memory, None),
            collecting_logs_ts: None,
            refreshing_evm_params_ts: None,
            operations_run_ts: None,
            services: Default::default(),
        }
    }

    /// Checks if the EVM parameters should be refreshed.
    ///
    /// The EVM parameters are refreshed if the `refreshing_evm_params_ts` timestamp
    /// is older than the `TASK_LOCK_TIMEOUT` duration,
    /// or if the `refreshing_evm_params_ts` is `None`.
    pub fn should_refresh_evm_params(&self) -> bool {
        self.refreshing_evm_params_ts
            .map(|ts| (ts + SYS_TASK_LOCK_TIMEOUT.as_nanos() as u64) <= ic::time())
            .unwrap_or(true)
    }

    /// Checks if the EVM logs should be collected.
    ///
    /// The EVM logs are collected if the `collecting_logs_ts` timestamp
    /// is older than the `TASK_LOCK_TIMEOUT` duration,
    /// or if the `collecting_logs_ts` is `None`.
    pub fn should_collect_evm_logs(&self) -> bool {
        self.collecting_logs_ts
            .map(|ts| (ts + SYS_TASK_LOCK_TIMEOUT.as_nanos() as u64) <= ic::time())
            .unwrap_or(true)
    }

    /// Checks if the scheduled operations and services ready to run.
    ///
    /// The EVM logs are collected if the `operations_run_ts` timestamp
    /// is older than the `OPERATIONS_RUN_TIMEOUT` duration,
    /// or if the `operations_run_ts` is `None`.
    pub fn should_process_operations(&self) -> bool {
        self.operations_run_ts
            .map(|ts| (ts + SCHEDULER_RUN_LOCK_TIMEOUT.as_nanos() as u64) <= ic::time())
            .unwrap_or(true)
    }

    /// Adds the given operation to the given service processing.
    pub fn push_operation_to_service(
        &self,
        service: ServiceId,
        operation_id: OperationId,
    ) -> BftResult<()> {
        self.services
            .borrow_mut()
            .push_operation(service, operation_id)
    }
}

#[cfg(test)]
mod tests {
    use bridge_did::error::BftResult;
    use bridge_did::op_id::OperationId;
    use candid::CandidType;
    use ic_exports::ic_kit::MockContext;
    use ic_stable_structures::MemoryId;
    use serde::{Deserialize, Serialize};

    use super::*;
    use crate::bridge::OperationProgress;
    use crate::memory::memory_by_id;
    use crate::runtime::{default_state, RuntimeState};

    #[derive(Clone, Deserialize, Debug, Serialize, CandidType)]
    pub struct TestOp;

    impl Operation for TestOp {
        async fn progress(
            self,
            _: OperationId,
            _: RuntimeState<Self>,
        ) -> BftResult<OperationProgress<Self>> {
            unimplemented!()
        }

        fn is_complete(&self) -> bool {
            unimplemented!()
        }

        fn evm_wallet_address(&self) -> did::H160 {
            unimplemented!()
        }

        async fn on_wrapped_token_minted(
            _ctx: RuntimeState<Self>,
            _event: bridge_did::event_data::MintedEventData,
        ) -> Option<crate::bridge::OperationAction<Self>> {
            unimplemented!()
        }

        async fn on_wrapped_token_burnt(
            _ctx: RuntimeState<Self>,
            _event: bridge_did::event_data::BurntEventData,
        ) -> Option<crate::bridge::OperationAction<Self>> {
            unimplemented!()
        }

        async fn on_minter_notification(
            _ctx: RuntimeState<Self>,
            _event: bridge_did::event_data::NotifyMinterEventData,
        ) -> Option<crate::bridge::OperationAction<Self>> {
            unimplemented!()
        }
    }

    const MEMORY_ID: MemoryId = MemoryId::new(1);

    fn create_test_state() -> Rc<RefCell<State<TestOp>>> {
        default_state(Rc::new(RefCell::new(ConfigStorage::default(memory_by_id(
            MEMORY_ID,
        )))))
    }

    #[test]
    fn test_should_refresh_evm_params() {
        let context = MockContext::new().inject();
        let state = create_test_state();

        assert!(state.borrow().should_refresh_evm_params());

        let time = ic::time();
        state.borrow_mut().refreshing_evm_params_ts = Some(time);
        assert!(!state.borrow().should_refresh_evm_params());

        context.add_time(SYS_TASK_LOCK_TIMEOUT.as_nanos() as u64 + 1);
        assert!(state.borrow().should_refresh_evm_params());
    }

    #[test]
    fn test_should_collect_evm_logs() {
        let context = MockContext::new().inject();
        let state = create_test_state();

        assert!(state.borrow().should_collect_evm_logs());

        let time = ic::time();
        state.borrow_mut().collecting_logs_ts = Some(time);
        assert!(!state.borrow().should_collect_evm_logs());

        context.add_time(SYS_TASK_LOCK_TIMEOUT.as_nanos() as u64 + 1);
        assert!(state.borrow().should_collect_evm_logs());
    }
}
