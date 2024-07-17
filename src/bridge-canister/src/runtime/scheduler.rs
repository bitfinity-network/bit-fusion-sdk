use std::future::Future;
use std::pin::Pin;

use bridge_did::error::{BftResult, Error};
use bridge_did::op_id::OperationId;
use bridge_utils::bft_events::BridgeEvent;
use bridge_utils::evm_bridge::EvmParams;
use bridge_utils::query::{self, Query, QueryType, GAS_PRICE_ID, NONCE_ID};
use candid::CandidType;
use did::U256;
use eth_signer::sign_strategy::TransactionSigner;
use ic_stable_structures::StableBTreeMap;
use ic_task_scheduler::scheduler::{Scheduler, TaskScheduler};
use ic_task_scheduler::task::{InnerScheduledTask, ScheduledTask, Task, TaskStatus};
use ic_task_scheduler::SchedulerError;
use jsonrpc_core::Id;
use serde::{Deserialize, Serialize};

use super::RuntimeState;
use crate::bridge::{Operation, OperationAction};

pub type TasksStorage<Mem, Op> = StableBTreeMap<u32, InnerScheduledTask<BridgeTask<Op>>, Mem>;
pub type BridgeScheduler<Mem, Op> = Scheduler<BridgeTask<Op>, TasksStorage<Mem, Op>>;
pub type DynScheduler<Op> = Box<dyn TaskScheduler<BridgeTask<Op>>>;

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

#[derive(Debug, Clone, Serialize, Deserialize, CandidType)]
pub enum BridgeTask<Op> {
    Operation(OperationId, Op),
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

#[derive(Debug, Clone, Serialize, Deserialize, CandidType)]
pub enum ServiceTask {
    CollectEvmLogs,
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
                let log_collection_result =
                    ServiceTask::collect_evm_logs(ctx.clone(), task_scheduler).await;

                ctx.borrow_mut().collecting_logs = false;

                log_collection_result
            }
            ServiceTask::RefreshEvmParams => {
                let evm_params_refreshing_result =
                    ServiceTask::refresh_evm_params(ctx.clone()).await;

                ctx.borrow_mut().refreshing_evm_params = false;

                evm_params_refreshing_result
            }
        }
    }

    async fn collect_evm_logs<Op: Operation>(
        ctx: RuntimeState<Op>,
        task_scheduler: DynScheduler<Op>,
    ) -> BftResult<()> {
        log::trace!("collecting evm events");

        let client = ctx
            .borrow()
            .config
            .borrow()
            .get_evm_link()
            .get_json_rpc_client();
        let Ok(evm_params) = ctx.borrow().config.borrow().get_evm_params() else {
            log::info!("evm parameters are not initilized");
            return Err(Error::Initialization(
                "evm params should be initialized before evm logs collecting".into(),
            ));
        };

        let Some(bridge_contract) = ctx.borrow().config.borrow().get_bft_bridge_contract() else {
            log::warn!("no bft bridge contract set, unable to collect events");
            return Err(Error::Initialization(
                "bft bridge contract address should be initialized before evm logs collecting"
                    .into(),
            ));
        };

        let last_chain_block = match client.get_block_number().await {
            Ok(block) => block,
            Err(e) => {
                log::warn!("failed to get evm block number: {e}");
                return Err(Error::EvmRequestFailed(e.to_string()));
            }
        };
        let last_request_block =
            last_chain_block.min(evm_params.next_block + Self::MAX_LOG_REQUEST_COUNT);

        let logs_result = BridgeEvent::collect_logs(
            &client,
            evm_params.next_block,
            last_request_block,
            bridge_contract.0,
        )
        .await;

        let logs = match logs_result {
            Ok(l) => l,
            Err(e) => {
                log::warn!("failed to collect evm logs: {e}");
                return Err(Error::EvmRequestFailed(e.to_string()));
            }
        };

        log::debug!(
            "Got evm logs between blocks {} and {last_request_block} (last chain block is {last_chain_block}: {logs:?}", 
            evm_params.next_block
        );

        ctx.borrow()
            .config
            .borrow_mut()
            .update_evm_params(|params| params.next_block = last_request_block + 1);

        log::trace!("creating operations according to logs: {logs:?}");

        let events = logs
            .into_iter()
            .filter_map(|log| match BridgeEvent::from_log(log) {
                Ok(l) => Some(l),
                Err(e) => {
                    log::warn!("failed to decode log into event: {e}");
                    None
                }
            });

        for event in events {
            let operation_action = match event {
                BridgeEvent::Burnt(event) => Op::on_wrapped_token_burnt(ctx.clone(), event).await,
                BridgeEvent::Minted(event) => Op::on_wrapped_token_minted(ctx.clone(), event).await,
                BridgeEvent::Notify(event) => Op::on_minter_notification(ctx.clone(), event).await,
            };

            let to_schedule = match operation_action {
                Some(OperationAction::Create(op)) => {
                    let new_op_id = ctx.borrow_mut().operations.new_operation(op.clone());
                    op.scheduling_options().zip(Some((new_op_id, op)))
                }
                Some(OperationAction::Update { nonce, update_to }) => {
                    let Some((operation_id, _)) = ctx
                        .borrow()
                        .operations
                        .get_for_address(&update_to.evm_address(), None, None)
                        .into_iter()
                        .find(|(operation_id, _)| operation_id.nonce() == nonce)
                    else {
                        log::warn!(
                            "operation with dst_address = {} and nonce {} not found",
                            update_to.evm_address(),
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

        Ok(())
    }

    async fn refresh_evm_params<Op: Operation>(state: RuntimeState<Op>) -> BftResult<()> {
        log::trace!("updating evm params");

        let config = state.borrow().config.clone();
        let client = config.borrow().get_evm_link().get_json_rpc_client();
        let initial_params = config.borrow().get_evm_params()?;
        let address = {
            let signer = config.borrow().get_signer()?;
            signer.get_address().await?
        };

        // Update the EvmParams
        log::trace!("updating evm params");
        let responses = query::batch_query(
            &client,
            &[
                QueryType::Nonce {
                    address: address.into(),
                },
                QueryType::GasPrice,
            ],
        )
        .await
        .map_err(|e| Error::EvmRequestFailed(format!("failed to query evm params: {e}")))?;

        let nonce: U256 = responses
            .get_value_by_id(Id::Str(NONCE_ID.into()))
            .map_err(|e| Error::EvmRequestFailed(format!("failed to query nonce: {e}")))?;
        let gas_price: U256 = responses
            .get_value_by_id(Id::Str(GAS_PRICE_ID.into()))
            .map_err(|e| Error::EvmRequestFailed(format!("failed to query gas price: {e}")))?;

        let params = EvmParams {
            nonce: nonce.0.as_u64(),
            gas_price,
            ..initial_params
        };

        config.borrow_mut().update_evm_params(|p| *p = params);

        log::trace!("evm params updated");

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
                .into_scheduler_result()
        })
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
