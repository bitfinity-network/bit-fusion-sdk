use bitcoin::Address;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;

use did::H160;
use eth_signer::sign_strategy::TransactionSigner;
use ethers_core::types::{BlockNumber, Log};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{CellStructure, StableUnboundedMap, VirtualMemory};
use ic_task_scheduler::retry::BackoffPolicy;
use ic_task_scheduler::scheduler::{Scheduler, TaskScheduler};
use ic_task_scheduler::task::{ScheduledTask, Task, TaskOptions};
use ic_task_scheduler::SchedulerError;
use minter_contract_utils::bft_bridge_api::{BridgeEvent, BurntEventData, MintedEventData};
use minter_contract_utils::evm_bridge::EvmParams;
use minter_did::id256::Id256;
use serde::{Deserialize, Serialize};

use crate::canister::get_state;

pub type TasksStorage =
    StableUnboundedMap<u32, ScheduledTask<BtcTask>, VirtualMemory<DefaultMemoryImpl>>;

pub type PersistentScheduler = Scheduler<BtcTask, TasksStorage>;

#[derive(Debug, Serialize, Deserialize)]
pub enum BtcTask {
    InitEvmState,
    CollectEvmEvents,
    RemoveMintOrder(MintedEventData),
    MintBtc(BurntEventData),
    MintErc20(H160),
}

impl BtcTask {
    pub fn into_scheduled(self, options: TaskOptions) -> ScheduledTask<Self> {
        ScheduledTask::with_options(self, options)
    }

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

    fn task_by_log(log: Log) -> Option<ScheduledTask<BtcTask>> {
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
                let mint_order_task = BtcTask::MintBtc(burnt);
                return Some(mint_order_task.into_scheduled(options));
            }
            Ok(BridgeEvent::Minted(minted)) => {
                log::debug!("Adding RemoveMintOrder task");
                let remove_mint_order_task = BtcTask::RemoveMintOrder(minted);
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
}

impl Task for BtcTask {
    fn execute(
        &self,
        task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
        match self {
            BtcTask::InitEvmState => Box::pin(Self::init_evm_state()),
            BtcTask::CollectEvmEvents => Box::pin(Self::collect_evm_events(task_scheduler)),
            BtcTask::RemoveMintOrder(data) => {
                let data = data.clone();
                Box::pin(async move { Self::remove_mint_order(data) })
            }
            BtcTask::MintErc20(address) => {
                let _address = address.clone();
                Box::pin(async move {
                    todo!()
                    //
                    // log::info!("ERC20 mint result from scheduler: {result:?}");
                    //
                    // Ok(())
                })
            }
            BtcTask::MintBtc(BurntEventData {
                recipient_id,
                amount,
                ..
            }) => {
                log::info!("ERC20 burn event received");

                let amount = amount.0.as_u128();

                let Ok(address_string) = String::from_utf8(recipient_id.clone()) else {
                    return Box::pin(futures::future::err(SchedulerError::TaskExecutionFailed(
                        format!(
                            "Failed to decode recipient address from raw data: {recipient_id:?}"
                        ),
                    )));
                };

                let Ok(address) = Address::from_str(&address_string) else {
                    return Box::pin(futures::future::err(SchedulerError::TaskExecutionFailed(
                        format!("Failed to decode recipient address from string: {address_string}"),
                    )));
                };

                Box::pin(async move {
                    let tx_id =
                        crate::ops::withdraw(&get_state(), amount, address.assume_checked())
                            .await
                            .map_err(|err| {
                                SchedulerError::TaskExecutionFailed(format!("{err:?}"))
                            })?;

                    log::info!("Created withdrawal transaction: {tx_id}",);

                    Ok(())
                })
            }
        }
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
