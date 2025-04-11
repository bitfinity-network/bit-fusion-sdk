use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use bridge_did::error::{BTFResult, Error};
use bridge_did::op_id::OperationId;
use candid::CandidType;
use ic_stable_structures::stable_structures::Memory;
use ic_stable_structures::{StableBTreeMap, StableCell};
use ic_task_scheduler::SchedulerError;
use ic_task_scheduler::scheduler::{Scheduler, TaskScheduler};
use ic_task_scheduler::task::{InnerScheduledTask, ScheduledTask, Task, TaskStatus};
use serde::{Deserialize, Serialize};

use super::RuntimeState;
use crate::bridge::{Operation, OperationProgress};

pub type TasksStorage<Mem, Op> = StableBTreeMap<u64, InnerScheduledTask<BridgeTask<Op>>, Mem>;
pub type BridgeScheduler<Mem, Op> =
    Scheduler<BridgeTask<Op>, TasksStorage<Mem, Op>, StableCell<u64, Mem>>;
pub type DynScheduler<Op> = Box<dyn TaskScheduler<BridgeTask<Op>>>;

/// Newtype for `Rc<Scheduler>`.
#[derive(Clone)]
pub struct SharedScheduler<Mem, Op>(Rc<BridgeScheduler<Mem, Op>>)
where
    Mem: Memory + 'static,
    Op: Operation;

impl<Mem, Op> SharedScheduler<Mem, Op>
where
    Mem: Memory + 'static,
    Op: Operation,
{
    pub fn new(
        tasks_storage: TasksStorage<Mem, Op>,
        sequence: StableCell<u64, Mem>,
    ) -> SharedScheduler<Mem, Op> {
        Self(Rc::new(BridgeScheduler::new(tasks_storage, sequence)))
    }

    pub fn run(&self, state: RuntimeState<Op>) -> Result<usize, SchedulerError> {
        self.0.run(state)
    }
}

impl<Mem, Op> TaskScheduler<BridgeTask<Op>> for SharedScheduler<Mem, Op>
where
    Mem: Memory + 'static,
    Op: Operation,
{
    fn append_task(&self, task: ScheduledTask<BridgeTask<Op>>) -> u64 {
        self.0.append_task(task)
    }

    fn append_tasks(&self, tasks: Vec<ScheduledTask<BridgeTask<Op>>>) -> Vec<u64> {
        self.0.append_tasks(tasks)
    }

    fn get_task(&self, task_id: u64) -> Option<InnerScheduledTask<BridgeTask<Op>>> {
        self.0.get_task(task_id)
    }

    fn find_id(&self, filter: &dyn Fn(BridgeTask<Op>) -> bool) -> Option<u64> {
        self.0.find_id(filter)
    }

    fn reschedule(&self, task_id: u64, options: ic_task_scheduler::task::TaskOptions) {
        self.0.reschedule(task_id, options)
    }
}

/// Logs errors that occur during task execution.
///
/// This function is intended to be used as the `on_error` callback for
/// `ic_task_scheduler::Scheduler`.
pub fn log_task_execution_error<Op: Operation>(task: InnerScheduledTask<BridgeTask<Op>>) {
    match task.status() {
        TaskStatus::Failed {
            timestamp_secs,
            error,
        } => {
            log::error!(
                "task #{} execution failed: {error} at {timestamp_secs}",
                task.id()
            )
        }
        TaskStatus::TimeoutOrPanic { timestamp_secs } => {
            log::error!("task #{} panicked at {timestamp_secs}", task.id())
        }
        status_change => {
            log::trace!("task #{} status changed: {status_change:?}", task.id())
        }
    };
}

/// Task type used by `BridgeRuntime`.
#[derive(Debug, Clone, Serialize, Deserialize, CandidType)]
pub struct BridgeTask<Op> {
    pub op_id: OperationId,
    pub operation: Op,
}

impl<Op: Operation> BridgeTask<Op> {
    pub fn new(op_id: OperationId, operation: Op) -> Self {
        Self { op_id, operation }
    }

    async fn execute_inner(
        self,
        ctx: RuntimeState<Op>,
        task_scheduler: DynScheduler<Op>,
    ) -> BTFResult<()> {
        let Some(operation) = ctx.borrow().operations.get(self.op_id) else {
            log::warn!("Operation #{} not found.", { self.op_id });
            return Err(Error::OperationNotFound(self.op_id));
        };

        let ctx_clone = ctx.clone();
        let progress = operation
            .progress(self.op_id, ctx.clone())
            .await
            .inspect_err(move |err| {
                ctx_clone
                    .borrow_mut()
                    .operations
                    .update_with_err(self.op_id, err.to_string())
            })?;

        let new_op = match progress {
            OperationProgress::Progress(op) => op,
            OperationProgress::AddToService(service_id) => {
                ctx.borrow()
                    .push_operation_to_service(service_id, self.op_id)?;
                return Ok(());
            }
        };

        let scheduling_options = new_op.scheduling_options();
        ctx.borrow_mut()
            .operations
            .update(self.op_id, new_op.clone());

        if let Some(options) = scheduling_options {
            let scheduled_task = ScheduledTask::with_options(
                Self {
                    op_id: self.op_id,
                    operation: new_op,
                },
                options,
            );
            task_scheduler.append_task(scheduled_task);
        }

        Ok(())
    }
}

impl<Op: Operation> Task for BridgeTask<Op> {
    type Ctx = RuntimeState<Op>;

    fn execute(
        &self,
        ctx: RuntimeState<Op>,
        task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
        let self_clone = self.clone();
        Box::pin(async {
            self_clone
                .execute_inner(ctx, task_scheduler)
                .await
                .map_err(|e| match e {
                    Error::CannotProgress(_) => {
                        log::trace!("Unrecoverable error during task execution: {e}");
                        SchedulerError::Unrecoverable(e.to_string())
                    }
                    _ => {
                        log::trace!("Error during task execution: {e}");
                        SchedulerError::TaskExecutionFailed(e.to_string())
                    }
                })
        })
    }
}

#[cfg(test)]
mod tests {
    use did::H160;
    use ic_exports::ic_kit::MockContext;
    use ic_storage::IcStorage;
    use snapbox::{assert_data_eq, str};

    use super::*;
    use crate::bridge::OperationProgress;
    use crate::runtime::BridgeRuntime;
    use crate::runtime::state::config::ConfigStorage;

    #[derive(Debug, CandidType, Serialize, Deserialize, Clone, Eq, PartialEq)]
    struct TestOperation {
        successful: bool,
        successful_runs: usize,
        recoverable: bool,
    }

    impl TestOperation {
        const ERR_MESSAGE: &'static str = "test error";

        fn new_err() -> Self {
            Self {
                successful: false,
                successful_runs: 0,
                recoverable: true,
            }
        }

        fn new_ok() -> Self {
            Self {
                successful: true,
                successful_runs: 0,
                recoverable: true,
            }
        }

        fn new_unrecoverable() -> Self {
            Self {
                successful: false,
                successful_runs: 0,
                recoverable: false,
            }
        }
    }

    impl Operation for TestOperation {
        async fn progress(
            self,
            _id: OperationId,
            _ctx: RuntimeState<Self>,
        ) -> BTFResult<OperationProgress<Self>> {
            if self.successful {
                Ok(OperationProgress::Progress(Self {
                    successful_runs: self.successful_runs + 1,
                    successful: self.successful,
                    recoverable: self.recoverable,
                }))
            } else if self.recoverable {
                Err(Error::FailedToProgress(Self::ERR_MESSAGE.to_string()))
            } else {
                Err(Error::CannotProgress(Self::ERR_MESSAGE.to_string()))
            }
        }

        fn is_complete(&self) -> bool {
            false
        }

        fn evm_wallet_address(&self) -> H160 {
            H160::from_slice(&[1; 20])
        }
    }

    #[tokio::test]
    async fn operation_errors_are_stored_in_log() {
        MockContext::new().inject();

        let runtime: BridgeRuntime<TestOperation> = BridgeRuntime::default(ConfigStorage::get());
        let ctx = runtime.state.clone();
        let op = TestOperation::new_err();
        let id = ctx.borrow_mut().operations.new_operation(op.clone(), None);

        const COUNT: usize = 5;
        for _ in 0..COUNT {
            let op = ctx.borrow().operations.get(id).unwrap();
            let task = BridgeTask::new(id, op);
            task.execute_inner(ctx.clone(), Box::new(runtime.scheduler.clone()))
                .await
                .unwrap_err();
        }

        let log = ctx
            .borrow()
            .operations
            .get_log(id)
            .expect("operation is not in the log");
        assert_eq!(log.log().len(), COUNT + 1);
        assert_eq!(log.log()[0].step_result, Ok(op));

        for i in 1..COUNT + 1 {
            assert!(
                log.log()[i]
                    .step_result
                    .as_ref()
                    .unwrap_err()
                    .contains(TestOperation::ERR_MESSAGE)
            );
        }
    }

    #[tokio::test]
    async fn operation_steps_are_stored_in_log() {
        MockContext::new().inject();

        let runtime: BridgeRuntime<TestOperation> = BridgeRuntime::default(ConfigStorage::get());
        let ctx = runtime.state.clone();
        let op = TestOperation::new_ok();
        let id = ctx.borrow_mut().operations.new_operation(op.clone(), None);

        const COUNT: usize = 5;
        for _ in 0..COUNT {
            let op = ctx.borrow().operations.get(id).unwrap();
            let task = BridgeTask::new(id, op);
            task.execute_inner(ctx.clone(), Box::new(runtime.scheduler.clone()))
                .await
                .unwrap();
        }

        let log = ctx
            .borrow()
            .operations
            .get_log(id)
            .expect("operation is not in the log");
        assert_eq!(log.log().len(), COUNT + 1);
        assert_eq!(log.log()[0].step_result, Ok(op));

        for i in 0..COUNT + 1 {
            assert_eq!(
                log.log()[i].step_result.as_ref().unwrap(),
                &TestOperation {
                    successful: true,
                    successful_runs: i,
                    recoverable: true,
                }
            );
        }
    }

    #[tokio::test]
    async fn execute_correctly_converts_recoverable_error() {
        MockContext::new().inject();

        let runtime: BridgeRuntime<TestOperation> = BridgeRuntime::default(ConfigStorage::get());
        let ctx = runtime.state.clone();
        let op = TestOperation::new_err();
        let id = ctx.borrow_mut().operations.new_operation(op.clone(), None);

        let task = BridgeTask::new(id, op);
        let err = task
            .execute(ctx.clone(), Box::new(runtime.scheduler.clone()))
            .await
            .unwrap_err();

        assert!(matches!(err, SchedulerError::TaskExecutionFailed(_)));

        assert_data_eq!(
            err.to_string(),
            str!["TaskExecutionFailed: operation failed to progress: test error"]
        )
    }

    #[tokio::test]
    async fn execute_correctly_converts_unrecoverable_error() {
        MockContext::new().inject();

        let runtime: BridgeRuntime<TestOperation> = BridgeRuntime::default(ConfigStorage::get());
        let ctx = runtime.state.clone();
        let op = TestOperation::new_unrecoverable();
        let id = ctx.borrow_mut().operations.new_operation(op.clone(), None);

        let task = BridgeTask::new(id, op);
        let err = task
            .execute(ctx.clone(), Box::new(runtime.scheduler.clone()))
            .await
            .unwrap_err();

        assert!(matches!(err, SchedulerError::Unrecoverable(_)));

        assert_data_eq!(
            err.to_string(),
            str!["Unrecoverable task error: operation cannot progress: test error"]
        )
    }
}
