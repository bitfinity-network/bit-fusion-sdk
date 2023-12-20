use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use ic_task_scheduler::scheduler::TaskScheduler;
use ic_task_scheduler::task::{ScheduledTask, Task, TaskOptions};
use ic_task_scheduler::SchedulerError;
use serde::{Deserialize, Serialize};

use crate::canister::get_state;
use crate::state::{BridgeSide, State};

#[derive(Debug, Serialize, Deserialize)]
pub enum PersistentTask {
    InitEvmState(BridgeSide),
    CollectEvmInfo(BridgeSide),
}

impl Task for PersistentTask {
    fn execute(
        &self,
        _: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
        let state = get_state();
        match self {
            PersistentTask::InitEvmState(side) => Box::pin(Self::init_evm_state(state, *side)),
            PersistentTask::CollectEvmInfo(side) => {
                Box::pin(Self::collect_evm_events(state, *side))
            }
        }
    }
}

impl PersistentTask {
    pub fn into_scheduled(self, options: TaskOptions) -> ScheduledTask<Self> {
        ScheduledTask::with_options(self, options)
    }

    pub async fn init_evm_state(
        state: Rc<RefCell<State>>,
        side: BridgeSide,
    ) -> Result<(), SchedulerError> {
        Self::init_evm_chain_id(state.clone(), side).await?;
        Self::init_evm_next_block(state, side).await?;
        Ok(())
    }

    pub async fn init_evm_chain_id(
        state: Rc<RefCell<State>>,
        side: BridgeSide,
    ) -> Result<(), SchedulerError> {
        let link = {
            let state = state.borrow();
            let info = state.config.get_evm_info(side);

            // If chain id is already set, there is nothing to do.
            // WARN: Changing chain id in runtime may lead to funds loss.
            if info.chain_id.is_some() {
                return Ok(());
            }

            info.link
        };

        let chain_id = link
            .get_client()
            .get_chain_id()
            .await
            .into_scheduler_result()?;
        state.borrow_mut().config.set_evm_chain_id(chain_id, side);
        Ok(())
    }

    pub async fn init_evm_next_block(
        state: Rc<RefCell<State>>,
        side: BridgeSide,
    ) -> Result<(), SchedulerError> {
        let link = {
            let state = state.borrow();
            let info = state.config.get_evm_info(side);

            // If next block is already set, there is nothing to do.
            // WARN: Re-initializing next block in runtime may lead to funds loss.
            if info.next_block.is_some() {
                return Ok(());
            }

            info.link
        };

        let next_block = link
            .get_client()
            .get_block_number()
            .await
            .into_scheduler_result()?;
        state
            .borrow_mut()
            .config
            .set_evm_next_block(next_block, side);
        Ok(())
    }

    async fn collect_evm_events(
        state: Rc<RefCell<State>>,
        side: BridgeSide,
    ) -> Result<(), SchedulerError> {
        if !state.borrow().config.is_initialized(side) {
            return Self::init_evm_state(state, side).await;
        }

        let _client = state.borrow().config.get_evm_info(side).link.get_client();
        todo!("json-rpc client eth_getLogs impl")
    }
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
