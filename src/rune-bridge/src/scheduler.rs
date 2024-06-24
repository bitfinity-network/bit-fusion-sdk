use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;

use bitcoin::Address;
use candid::{CandidType, Decode};
use did::H160;
use eth_signer::sign_strategy::TransactionSigner;
use ethers_core::types::Log;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{CellStructure, StableBTreeMap, VirtualMemory};
use ic_task_scheduler::retry::BackoffPolicy;
use ic_task_scheduler::scheduler::{Scheduler, TaskScheduler};
use ic_task_scheduler::task::{InnerScheduledTask, ScheduledTask, Task, TaskOptions};
use ic_task_scheduler::SchedulerError;
use minter_contract_utils::bft_bridge_api::{
    BridgeEvent, BurntEventData, MintedEventData, NotifyMinterEventData,
};
use minter_contract_utils::evm_bridge::EvmParams;
use minter_contract_utils::operation_store::MinterOperationId;
use minter_did::id256::Id256;
use serde::{Deserialize, Serialize};

use crate::canister::get_state;
use crate::core::deposit::RuneDeposit;
use crate::core::withdrawal::Withdrawal;
use crate::rune_info::RuneName;

pub type TasksStorage =
    StableBTreeMap<u32, InnerScheduledTask<RuneBridgeTask>, VirtualMemory<DefaultMemoryImpl>>;

pub type PersistentScheduler = Scheduler<RuneBridgeTask, TasksStorage>;

mod minter_notify;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuneBridgeTask {
    InitEvmState,
    CollectEvmEvents,
    Deposit(MinterOperationId),
    RemoveMintOrder(MintedEventData),
    Withdraw(BurntEventData),
}

impl RuneBridgeTask {
    pub fn into_scheduled(self, options: TaskOptions) -> ScheduledTask<Self> {
        ScheduledTask::with_options(self, options)
    }

    pub async fn init_evm_state() -> Result<(), SchedulerError> {
        let state = get_state();
        let client = state.borrow().get_evm_info().link.get_json_rpc_client();
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

        let client = evm_info.link.get_json_rpc_client();
        let last_block = client.get_block_number().await.into_scheduler_result()?;

        let logs = BridgeEvent::collect_logs(
            &client,
            params.next_block,
            last_block,
            evm_info.bridge_contract.0,
        )
        .await
        .into_scheduler_result()?;

        log::debug!("got {} logs from evm", logs.len());

        if logs.is_empty() {
            return Ok(());
        }

        {
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
        }

        log::trace!("appending logs to tasks");

        scheduler.append_tasks(logs.into_iter().filter_map(Self::task_by_log).collect());

        Ok(())
    }

    async fn deposit(deposit_request_id: MinterOperationId) -> Result<(), SchedulerError> {
        RuneDeposit::get()
            .process_deposit_request(deposit_request_id)
            .await;
        Ok(())
    }

    fn task_by_log(log: Log) -> Option<ScheduledTask<RuneBridgeTask>> {
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
                let mint_order_task = RuneBridgeTask::Withdraw(burnt);
                return Some(mint_order_task.into_scheduled(options));
            }
            Ok(BridgeEvent::Minted(minted)) => {
                log::debug!("Adding RemoveMintOrder task");
                let remove_mint_order_task = RuneBridgeTask::RemoveMintOrder(minted);
                return Some(remove_mint_order_task.into_scheduled(options));
            }
            Ok(BridgeEvent::Notify(event)) => {
                if let Some(notification) = RuneMinterNotification::decode(event) {
                    return match notification {
                        RuneMinterNotification::Deposit(payload) => {
                            let request_id = RuneDeposit::get()
                                .create_deposit_request(payload.dst_address, payload.amounts);

                            let deposit_task = RuneBridgeTask::Deposit(request_id);
                            Some(deposit_task.into_scheduled(TaskOptions::new()))
                        }
                    };
                }
            }
            Err(e) => log::warn!("collected log is incompatible with expected events: {e}"),
        }

        None
    }

    fn remove_mint_order(minted_event: MintedEventData) -> Result<(), SchedulerError> {
        let mut deposit = RuneDeposit::get();
        deposit.complete_mint_request(minted_event.recipient, minted_event.nonce);
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

impl Task for RuneBridgeTask {
    fn execute(
        &self,
        task_scheduler: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
        match self {
            RuneBridgeTask::InitEvmState => Box::pin(Self::init_evm_state()),
            RuneBridgeTask::CollectEvmEvents => Box::pin(Self::collect_evm_events(task_scheduler)),
            RuneBridgeTask::Deposit(request_id) => Box::pin(Self::deposit(*request_id)),
            RuneBridgeTask::RemoveMintOrder(data) => {
                let data = data.clone();
                Box::pin(async move { Self::remove_mint_order(data) })
            }
            RuneBridgeTask::Withdraw(BurntEventData {
                recipient_id,
                amount,
                to_token,
                sender,
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

                let Some(token_id) = Id256::from_slice(to_token) else {
                    return Box::pin(futures::future::err(SchedulerError::TaskExecutionFailed(
                        format!("Failed to decode token id from the value {to_token:?}"),
                    )));
                };

                let Ok(rune_id) = token_id.try_into() else {
                    return Box::pin(futures::future::err(SchedulerError::TaskExecutionFailed(
                        format!("Failed to decode rune id from the token id {to_token:?}"),
                    )));
                };

                let sender = sender.clone();
                Box::pin(async move {
                    let withdrawal = Withdrawal::new(get_state());
                    let tx_id = withdrawal
                        .withdraw(amount, rune_id, sender, address.assume_checked())
                        .await
                        .map_err(|err| SchedulerError::TaskExecutionFailed(format!("{err:?}")))?;

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

pub enum RuneMinterNotification {
    Deposit(RuneDepositRequestData),
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct RuneDepositRequestData {
    pub dst_address: H160,
    pub amounts: Option<HashMap<RuneName, u128>>,
}

impl RuneMinterNotification {
    pub const DEPOSIT_TYPE: u32 = 1;
}

impl RuneMinterNotification {
    fn decode(event_data: NotifyMinterEventData) -> Option<Self> {
        match event_data.notification_type {
            Self::DEPOSIT_TYPE => match Decode!(&event_data.user_data, RuneDepositRequestData) {
                Ok(payload) => Some(Self::Deposit(payload)),
                Err(err) => {
                    log::warn!("Failed to decode deposit request event data: {err:?}");
                    None
                }
            },
            t => {
                log::warn!("Unknown minter notify event type: {t}");
                None
            }
        }
    }
}
