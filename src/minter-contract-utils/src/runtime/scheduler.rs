use std::{cell::RefCell, future::Future, marker::PhantomData, pin::Pin, rc::Rc};

use did::H160;
use ic_stable_structures::StableBTreeMap;
use ic_task_scheduler::{
    scheduler::{Scheduler, TaskScheduler},
    task::{InnerScheduledTask, ScheduledTask, Task, TaskStatus},
    SchedulerError,
};

use crate::{bridge2::BftResult, bridge2::Operation, operation_store::MinterOperationId};

use super::RuntimeState;

pub type TasksStorage<Mem> = StableBTreeMap<u32, InnerScheduledTask<BridgeTask>, Mem>;
pub type BridgeScheduler<Mem> = Scheduler<BridgeTask, TasksStorage<Mem>>;

fn log_task_execution_error<Op>(task: InnerScheduledTask<BridgeTask>)
where
    Op: TaskContext + std::clone::Clone,
{
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

#[derive(Debug, Clone)]
pub enum BridgeTask {
    Operation(MinterOperationId),
    Service(ServiceTask),
}

impl BridgeTask {
    async fn execute_inner(
        self,
        mut ctx: RuntimeState,
        task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> BftResult<()> {
        match self {
            BridgeTask::Operation(id) => {
                let operation = ctx.get_operation(id)?;
                let new_operation = operation.progress().await?;
                let scheduling_options = new_operation.scheduling_options();
                ctx.update_operation(id, new_operation)?;
                if let Some(options) = scheduling_options {
                    let scheduled_task = ScheduledTask::with_options(Self::Operation(id), options);
                    task_scheduler.append_task(scheduled_task);
                }

                Ok(())
            }
            BridgeTask::Service(_) => todo!(),
        }
    }
}

#[derive(Debug, Clone)]
pub enum ServiceTask {
    CollectEvmEvents,
}

impl Task for BridgeTask {
    type Ctx = RuntimeState;

    fn execute(
        &self,
        ctx: RuntimeState,
        task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
        let self_clone = self.clone();
        Box::pin(async {
            self_clone
                .execute_inner(ctx, task_scheduler)
                .await
                .into_scheduler_result()
        })
    }
}

pub trait TaskContext {
    type Op: Operation;

    fn get_operation(&self, id: MinterOperationId) -> BftResult<Self::Op>;
    fn get_operation_id_by_address(
        &self,
        address: H160,
        nonce: u32,
    ) -> BftResult<MinterOperationId>;
    fn create_operation(&mut self, op: Self::Op) -> MinterOperationId;
    fn update_operation(&mut self, id: MinterOperationId, op: Self::Op) -> BftResult<()>;
}

trait IntoSchedulerError {
    type Success;

    fn into_scheduler_result(self) -> Result<Self::Success, SchedulerError>;
}

impl<T, E: ToString> IntoSchedulerError for Result<T, E> {
    type Success = T;

    fn into_scheduler_result(self) -> Result<Self::Success, SchedulerError> {
        self.map_err(|e| SchedulerError::TaskExecutionFailed(e.to_string()))
    }
}
