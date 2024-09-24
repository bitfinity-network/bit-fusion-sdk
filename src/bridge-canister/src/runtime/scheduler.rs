use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use bridge_did::error::{BftResult, Error};
use bridge_did::event_data::*;
use bridge_did::op_id::OperationId;
use bridge_utils::bft_events::BridgeEvent;
use candid::{CandidType, Decode};
use drop_guard::guard;
use ic_stable_structures::stable_structures::Memory;
use ic_stable_structures::{StableBTreeMap, StableCell};
use ic_task_scheduler::scheduler::{Scheduler, TaskScheduler};
use ic_task_scheduler::task::{InnerScheduledTask, ScheduledTask, Task, TaskStatus};
use ic_task_scheduler::SchedulerError;
use serde::{Deserialize, Serialize};

use super::RuntimeState;
use crate::bridge::{Operation, OperationAction, OperationContext, OperationProgress};
use crate::runtime::state::config::ConfigStorage;

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
pub enum BridgeTask<Op> {
    /// Bridge operations defined by user.
    Operation(OperationId, Op),

    /// Bridge operations defined by the runtime itself.
    Service(ServiceTask),
}

impl<Op: Operation> BridgeTask<Op> {
    async fn execute_inner(
        self,
        ctx: RuntimeState<Op>,
        task_scheduler: DynScheduler<Op>,
    ) -> BftResult<()> {
        match self {
            BridgeTask::Operation(op_id, _) => {
                let Some(operation) = ctx.borrow().operations.get(op_id) else {
                    log::warn!("Operation #{op_id} not found.");
                    return Err(Error::OperationNotFound(op_id));
                };

                let ctx_clone = ctx.clone();
                let progress =
                    operation
                        .progress(op_id, ctx.clone())
                        .await
                        .inspect_err(move |err| {
                            ctx_clone
                                .borrow_mut()
                                .operations
                                .update_with_err(op_id, err.to_string())
                        })?;

                let new_op = match progress {
                    OperationProgress::Progress(op) => op,
                    OperationProgress::AddToService(service_id) => {
                        ctx.push_operation_to_service(service_id, op_id)?;
                        return Ok(());
                    }
                };

                let scheduling_options = new_op.scheduling_options();
                ctx.borrow_mut().operations.update(op_id, new_op.clone());

                if let Some(options) = scheduling_options {
                    let scheduled_task =
                        ScheduledTask::with_options(Self::Operation(op_id, new_op), options);
                    task_scheduler.append_task(scheduled_task);
                }

                Ok(())
            }
            BridgeTask::Service(service_task) => service_task.execute(ctx, task_scheduler).await,
        }
    }
}

/// Service tasks, done by the `BridgeRuntime` by default.
#[derive(Debug, Clone, Serialize, Deserialize, CandidType)]
pub enum ServiceTask {
    /// Task to query logs from EVM.
    CollectEvmLogs,

    /// Task to refresh EVM parameters.
    RefreshEvmParams,
}

impl ServiceTask {
    const MAX_LOG_REQUEST_COUNT: u64 = 1000;

    async fn execute<Op: Operation>(
        self,
        ctx: RuntimeState<Op>,
        task_scheduler: DynScheduler<Op>,
    ) -> BftResult<()> {
        match self {
            ServiceTask::CollectEvmLogs => {
                let _lock = guard(ctx.clone(), |s| s.borrow_mut().collecting_logs_ts = None);

                ServiceTask::collect_evm_logs(ctx.clone(), task_scheduler).await
            }
            ServiceTask::RefreshEvmParams => {
                let _lock = guard(ctx.clone(), |s| {
                    s.borrow_mut().refreshing_evm_params_ts = None
                });
                let config = ctx.borrow().config.clone();
                ConfigStorage::refresh_evm_params(config).await
            }
        }
    }

    async fn collect_evm_logs<Op: Operation>(
        ctx: RuntimeState<Op>,
        task_scheduler: DynScheduler<Op>,
    ) -> BftResult<()> {
        let collected = ctx.collect_evm_events(Self::MAX_LOG_REQUEST_COUNT).await?;
        let events = collected.events;

        ctx.borrow()
            .config
            .borrow_mut()
            .update_evm_params(|params| params.next_block = collected.last_block_number + 1);

        for event in events {
            let operation_action = match event {
                BridgeEvent::Burnt(event) => Op::on_wrapped_token_burnt(ctx.clone(), event).await,
                BridgeEvent::Minted(event) => Op::on_wrapped_token_minted(ctx.clone(), event).await,
                BridgeEvent::Notify(event) => {
                    Self::on_minter_notification(ctx.clone(), event, &task_scheduler).await
                }
            };

            let to_schedule = match operation_action {
                Some(OperationAction::Create(op, memo)) => {
                    let new_op_id = ctx.borrow_mut().operations.new_operation(op.clone(), memo);
                    op.scheduling_options().zip(Some((new_op_id, op)))
                }
                Some(OperationAction::CreateWithId(id, op, memo)) => {
                    ctx.borrow_mut()
                        .operations
                        .new_operation_with_id(id, op.clone(), memo);
                    op.scheduling_options().zip(Some((id, op)))
                }
                Some(OperationAction::Update { nonce, update_to }) => {
                    let Some((operation_id, _)) = ctx
                        .borrow()
                        .operations
                        .get_for_address(&update_to.evm_wallet_address(), None)
                        .into_iter()
                        .find(|(operation_id, _)| operation_id.nonce() == nonce)
                    else {
                        log::warn!(
                            "operation with dst_address = {} and nonce {} not found",
                            update_to.evm_wallet_address(),
                            nonce
                        );
                        return Err(Error::OperationNotFound(OperationId::new(nonce as _)));
                    };

                    ctx.borrow_mut()
                        .operations
                        .update(operation_id, update_to.clone());
                    update_to
                        .scheduling_options()
                        .zip(Some((operation_id, update_to)))
                }
                None => None,
            };

            if let Some((options, (op_id, op))) = to_schedule {
                let task = ScheduledTask::with_options(BridgeTask::Operation(op_id, op), options);
                task_scheduler.append_task(task);
            }
        }

        log::debug!("EVM logs collected");
        Ok(())
    }

    async fn on_minter_notification<Op: Operation>(
        ctx: RuntimeState<Op>,
        data: NotifyMinterEventData,
        scheduler: &DynScheduler<Op>,
    ) -> Option<OperationAction<Op>> {
        match data.notification_type {
            MinterNotificationType::RescheduleOperation => {
                let operation_id = match Decode!(&data.user_data, OperationId) {
                    Ok(v) => v,
                    Err(err) => {
                        log::warn!("Failed to decode operation id from reschedule operation request: {err:?}");
                        return None;
                    }
                };

                Self::reschedule_operation(ctx, operation_id, scheduler);
                None
            }
            _ => Op::on_minter_notification(ctx, data).await,
        }
    }

    fn reschedule_operation<Op: Operation>(
        ctx: RuntimeState<Op>,
        operation_id: OperationId,
        scheduler: &DynScheduler<Op>,
    ) {
        let Some(operation) = ctx.borrow().operations.get(operation_id) else {
            log::warn!(
                "Reschedule of operation #{operation_id} is requested but it does not exist"
            );
            return;
        };

        let Some(task_options) = operation.scheduling_options() else {
            log::info!("Reschedule of operation #{operation_id} is requested but no scheduling is required for this operation");
            return;
        };

        let current_task_id = scheduler.find_id(&|op| match op {
            BridgeTask::Operation(id, _) => id == operation_id,
            BridgeTask::Service(_) => false,
        });
        match current_task_id {
            Some(task_id) => {
                scheduler.reschedule(task_id, task_options.clone());
                log::trace!("Updated schedule for operation #{operation_id} task #{task_id} to {task_options:?}");
            }
            None => {
                let task_id = scheduler.append_task(
                    (BridgeTask::Operation(operation_id, operation), task_options).into(),
                );
                log::trace!("Restarted operation #{operation_id} with task id #{task_id}");
            }
        }
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
                    Error::CannotProgress(_) => SchedulerError::Unrecoverable(e.to_string()),
                    _ => SchedulerError::TaskExecutionFailed(e.to_string()),
                })
        })
    }
}

#[cfg(test)]
mod tests {
    use bridge_did::event_data::*;
    use did::H160;
    use ic_exports::ic_kit::MockContext;
    use ic_storage::IcStorage;
    use snapbox::{assert_data_eq, str};

    use super::*;
    use crate::bridge::OperationProgress;
    use crate::runtime::state::config::ConfigStorage;
    use crate::runtime::BridgeRuntime;

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
        ) -> BftResult<OperationProgress<Self>> {
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

        async fn on_wrapped_token_minted(
            _ctx: RuntimeState<Self>,
            _event: MintedEventData,
        ) -> Option<OperationAction<Self>> {
            unimplemented!()
        }

        async fn on_wrapped_token_burnt(
            _ctx: RuntimeState<Self>,
            _event: BurntEventData,
        ) -> Option<OperationAction<Self>> {
            unimplemented!()
        }

        async fn on_minter_notification(
            _ctx: RuntimeState<Self>,
            _event: NotifyMinterEventData,
        ) -> Option<OperationAction<Self>> {
            unimplemented!()
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
            let task = BridgeTask::Operation(id, op);
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
            assert!(log.log()[i]
                .step_result
                .as_ref()
                .unwrap_err()
                .contains(TestOperation::ERR_MESSAGE));
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
            let task = BridgeTask::Operation(id, op);
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

        let task = BridgeTask::Operation(id, op);
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

        let task = BridgeTask::Operation(id, op);
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
