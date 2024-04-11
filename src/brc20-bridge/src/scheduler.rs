#![allow(dead_code)]
use std::future::Future;
use std::pin::Pin;

use did::H160;
use eth_signer::sign_strategy::TransactionSigner;
use ethers_core::types::{BlockNumber, Log};
use ic_stable_structures::CellStructure;
use ic_task_scheduler::retry::BackoffPolicy;
use ic_task_scheduler::scheduler::TaskScheduler;
use ic_task_scheduler::task::{ScheduledTask, Task, TaskOptions};
use ic_task_scheduler::SchedulerError;
use minter_contract_utils::bft_bridge_api::{BridgeEvent, BurntEventData, MintedEventData};
use minter_contract_utils::evm_bridge::EvmParams;
use minter_did::id256::Id256;
use serde::{Deserialize, Serialize};

use crate::canister::get_state;

#[derive(Debug, Serialize, Deserialize)]
pub enum Brc20Task {
    InitEvmState,
    CollectEvmEvents,
    RemoveMintOrder(MintedEventData),
    MintErc20(H160),
    MintBrc20(BurntEventData),
}

impl Brc20Task {
    pub async fn init_evm_state() -> Result<(), SchedulerError> {
        let state = get_state();
        let client = state.borrow().get_evm_info().link.get_client();
        let address = {
            let signer = state.borrow().signer().get().clone();
            signer.get_address().await.into_scheduler_result()?
        };

        let evm_params = EvmParams::query(client, address)
            .await
            .into_scheduler_result()?;

        state
            .borrow_mut()
            .update_evm_params(|old| *old = Some(evm_params));

        log::trace!("Evm state is initialized");

        Ok(())
    }

    async fn collect_evm_events(
        scheduler: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> Result<(), SchedulerError> {
        log::trace!("collecting evm events");

        let state = get_state();
        let evm_info = state.borrow().get_evm_info();
        let Some(params) = evm_info.params else {
            log::warn!("no evm params initialized");
            return Ok(());
        };

        let client = evm_info.link.get_client();

        let logs = BridgeEvent::collect_logs(
            &client,
            params.next_block.into(),
            BlockNumber::Safe,
            evm_info.bridge_contract.0,
        )
        .await
        .into_scheduler_result()?;

        log::debug!("got {} logs from evm", logs.len());

        if logs.is_empty() {
            return Ok(());
        }

        let mut mut_state = state.borrow_mut();

        // Filter out logs that do not have block number.
        // Such logs are produced when the block is not finalized yet.
        let last_log = logs.iter().take_while(|l| l.block_number.is_some()).last();
        if let Some(last_log) = last_log {
            let next_block_number = last_log.block_number.unwrap().as_u64() + 1;
            mut_state.update_evm_params(|to_update| {
                *to_update = Some(EvmParams {
                    next_block: next_block_number,
                    ..params
                })
            });
        };

        log::trace!("appending logs to tasks");

        scheduler.append_tasks(logs.into_iter().filter_map(Self::task_by_log).collect());

        Ok(())
    }

    fn task_by_log(log: Log) -> Option<ScheduledTask<Brc20Task>> {
        log::trace!("creating task from the log: {log:?}");

        const TASK_RETRY_DELAY_SECS: u32 = 5;

        let options = TaskOptions::default()
            .with_backoff_policy(BackoffPolicy::Fixed {
                secs: TASK_RETRY_DELAY_SECS,
            })
            .with_max_retries_policy(u32::MAX);

        match BridgeEvent::from_log(log).into_scheduler_result() {
            Ok(BridgeEvent::Burnt(burnt)) => {
                log::debug!("Adding PrepareMintOrder task");
                let mint_order_task = Brc20Task::MintBrc20(burnt);
                return Some(mint_order_task.into_scheduled(options));
            }
            Ok(BridgeEvent::Minted(minted)) => {
                log::debug!("Adding RemoveMintOrder task");
                let remove_mint_order_task = Brc20Task::RemoveMintOrder(minted);
                return Some(remove_mint_order_task.into_scheduled(options));
            }
            Err(e) => log::warn!("collected log is incompatible with expected events: {e}"),
        }

        None
    }

    fn remove_mint_order(minted_event: MintedEventData) -> Result<(), SchedulerError> {
        let state = get_state();
        let sender_id = Id256::from_slice(&minted_event.sender_id).ok_or_else(|| {
            SchedulerError::TaskExecutionFailed(
                "failed to decode sender id256 from minted event".into(),
            )
        })?;

        state
            .borrow_mut()
            .mint_orders_mut()
            .remove(sender_id, minted_event.nonce);

        log::trace!("Mint order removed");

        Ok(())
    }

    pub fn into_scheduled(self, options: TaskOptions) -> ScheduledTask<Self> {
        ScheduledTask::with_options(self, options)
    }
}

impl Task for Brc20Task {
    fn execute(
        &self,
        _task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
        todo!()
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
