use std::{future::Future, pin::Pin};

use ic_stable_structures::StableBTreeMap;
use ic_task_scheduler::{
    scheduler::{Scheduler, TaskScheduler},
    task::{InnerScheduledTask, Task, TaskStatus},
    SchedulerError,
};

use crate::operation_store::MinterOperationId;

type TasksStorage<Mem> = StableBTreeMap<u32, InnerScheduledTask<BridgeTask>, Mem>;
type PersistentScheduler<Mem> = Scheduler<BridgeTask, TasksStorage<Mem>>;

fn log_task_execution_error(task: InnerScheduledTask<BridgeTask>) {
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

pub enum BridgeTask {
    Operation(MinterOperationId),
    Service(ServiceTask),
}

pub enum ServiceTask {
    CollectEvmEvents,
}

impl Task for BridgeTask {
    fn execute(
        &self,
        task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
        match self {
            BridgeTask::Operation(id) => todo!(),
            BridgeTask::Service(_) => todo!(),
        }
    }
}
