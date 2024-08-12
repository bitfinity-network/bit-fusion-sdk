use std::future::Future;
use std::pin::Pin;

use bridge_did::error::{BftResult, Error};
use bridge_did::op_id::OperationId;
use bridge_utils::bft_events::BridgeEvent;
use candid::CandidType;
use drop_guard::guard;
use ic_stable_structures::StableBTreeMap;
use ic_task_scheduler::scheduler::{Scheduler, TaskScheduler};
use ic_task_scheduler::task::{InnerScheduledTask, ScheduledTask, Task, TaskStatus};
use ic_task_scheduler::SchedulerError;
use serde::{Deserialize, Serialize};

use super::RuntimeState;
use crate::bridge::{Operation, OperationAction, OperationContext};
use crate::runtime::state::config::ConfigStorage;

pub type TasksStorage<Mem, Op> = StableBTreeMap<u32, InnerScheduledTask<BridgeTask<Op>>, Mem>;
pub type BridgeScheduler<Mem, Op> = Scheduler<BridgeTask<Op>, TasksStorage<Mem, Op>>;
pub type DynScheduler<Op> = Box<dyn TaskScheduler<BridgeTask<Op>>>;

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
            BridgeTask::Operation(id, _) => {
                let Some(operation) = ctx.borrow().operations.get(id) else {
                    log::warn!("Operation#{id} not found.");
                    return Err(Error::OperationNotFound(id));
                };

                log::warn!("Starting Operation#{id}.");

                let new_operation = operation.progress(id, ctx.clone()).await?;
                let scheduling_options = new_operation.scheduling_options();
                ctx.borrow_mut()
                    .operations
                    .update(id, new_operation.clone());

                if let Some(options) = scheduling_options {
                    let scheduled_task =
                        ScheduledTask::with_options(Self::Operation(id, new_operation), options);
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
            .update_evm_params(|params| params.next_block = collected.last_block_nubmer + 1);

        for event in events {
            let operation_action = match event {
                BridgeEvent::Burnt(event) => Op::on_wrapped_token_burnt(ctx.clone(), event).await,
                BridgeEvent::Minted(event) => Op::on_wrapped_token_minted(ctx.clone(), event).await,
                BridgeEvent::Notify(event) => Op::on_minter_notification(ctx.clone(), event).await,
            };

            let to_schedule = match operation_action {
                Some(OperationAction::Create(op)) => {
                    let new_op_id = ctx.borrow_mut().operations.new_operation(op.clone(), None);
                    op.scheduling_options().zip(Some((new_op_id, op)))
                }
                Some(OperationAction::CreateWithId(id, op)) => {
                    ctx.borrow_mut()
                        .operations
                        .new_operation_with_id(id, op.clone(), None);
                    op.scheduling_options().zip(Some((id, op)))
                }
                Some(OperationAction::CreateWithIdAndMemo(id, op, memo)) => {
                    ctx.borrow_mut()
                        .operations
                        .new_operation_with_id(id, op.clone(), Some(memo));
                    op.scheduling_options().zip(Some((id, op)))
                }
                Some(OperationAction::Update { nonce, update_to }) => {
                    let Some((operation_id, _)) = ctx
                        .borrow()
                        .operations
                        .get_for_address(&update_to.evm_wallet_address(), None, None)
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
                .map_err(|e| SchedulerError::TaskExecutionFailed(e.to_string()))
        })
    }
}
