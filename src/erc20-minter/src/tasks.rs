use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use did::{H160, U256};
use eth_signer::sign_strategy::TransactionSigner;
use ethers_core::types::{BlockNumber, Log};
use ic_stable_structures::CellStructure;
use ic_task_scheduler::retry::BackoffPolicy;
use ic_task_scheduler::scheduler::TaskScheduler;
use ic_task_scheduler::task::{ScheduledTask, Task, TaskOptions};
use ic_task_scheduler::SchedulerError;
use jsonrpc_core::Id;
use minter_contract_utils::bft_bridge_api::{self, BridgeEvent, BurntEventData, MintedEventData};
use minter_contract_utils::evm_bridge::{BridgeSide, EvmParams};
use minter_contract_utils::query::{self, Query, QueryType, GAS_PRICE_ID, NONCE_ID};
use minter_did::id256::Id256;
use minter_did::order::MintOrder;
use serde::{Deserialize, Serialize};

use crate::canister::get_state;
use crate::state::State;

type SignedMintOrderData = Vec<u8>;

/// Task for the ERC-20 bridge
#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum BridgeTask {
    InitEvmState(BridgeSide),
    CollectEvmEvents(BridgeSide),
    PrepareMintOrder(BurntEventData, BridgeSide),
    RemoveMintOrder(MintedEventData),
    SendMintTransaction(SignedMintOrderData, BridgeSide),
}

impl Task for BridgeTask {
    fn execute(
        &self,
        scheduler: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
        log::trace!("Running ERC-20 task: {:?}", self);

        let state = get_state();
        match self {
            BridgeTask::InitEvmState(side) => Box::pin(Self::init_evm_state(state, *side)),
            BridgeTask::CollectEvmEvents(side) => {
                Box::pin(Self::collect_evm_events(state, scheduler, *side))
            }
            BridgeTask::PrepareMintOrder(data, side) => Box::pin(Self::prepare_mint_order(
                state,
                scheduler,
                data.clone(),
                *side,
            )),
            BridgeTask::RemoveMintOrder(data) => {
                let data = data.clone();
                Box::pin(async move { Self::remove_mint_order(state, data) })
            }
            BridgeTask::SendMintTransaction(order_data, side) => Box::pin(
                Self::send_mint_transaction(state, order_data.clone(), *side),
            ),
        }
    }
}

impl BridgeTask {
    pub fn into_scheduled(self, options: TaskOptions) -> ScheduledTask<Self> {
        ScheduledTask::with_options(self, options)
    }

    pub async fn init_evm_state(
        state: Rc<RefCell<State>>,
        side: BridgeSide,
    ) -> Result<(), SchedulerError> {
        let client = state
            .borrow()
            .config
            .get_evm_info(side)
            .link
            .get_json_rpc_client();

        let address = {
            let signer = state.borrow().signer.get().clone();
            signer.get_address().await.into_scheduler_result()?
        };

        let evm_params = EvmParams::query(client, address)
            .await
            .into_scheduler_result()?;

        state
            .borrow_mut()
            .config
            .update_evm_params(|old| *old = evm_params, side);

        log::trace!("evm state initialized for side {:?}", side);

        Ok(())
    }

    async fn collect_evm_events(
        state: Rc<RefCell<State>>,
        scheduler: Box<dyn 'static + TaskScheduler<Self>>,
        side: BridgeSide,
    ) -> Result<(), SchedulerError> {
        log::trace!("collecting evm events: side: {side:?}");

        let evm_info = state.borrow().config.get_evm_info(side);
        let Some(params) = evm_info.params else {
            log::warn!("no evm params for side {side} found");
            return Self::init_evm_state(state, side).await;
        };

        let bft_bridge = state
            .borrow()
            .config
            .get_bft_bridge_contract(side)
            .ok_or_else(|| {
                SchedulerError::TaskExecutionFailed("no bft bridge contract set".into())
            })?;

        let client = evm_info.link.get_json_rpc_client();

        let logs = BridgeEvent::collect_logs(
            &client,
            params.next_block.into(),
            BlockNumber::Safe,
            bft_bridge.0,
        )
        .await
        .into_scheduler_result()?;

        log::debug!("got logs from side {side}: {logs:?}");

        // Filter out logs that do not have block number.
        // Such logs are produced when the block is not finalized yet.
        let last_log = logs.iter().take_while(|l| l.block_number.is_some()).last();
        if let Some(last_log) = last_log {
            let next_block_number = last_log.block_number.unwrap().as_u64() + 1;
            state
                .borrow_mut()
                .config
                .update_evm_params(|params| params.next_block = next_block_number, side);
        };

        log::trace!("appending logs to tasks: {side:?}: {logs:?}");

        scheduler.append_tasks(
            logs.into_iter()
                .filter_map(|l| Self::task_by_log(l, side))
                .collect(),
        );

        // Update the EVM params
        Self::update_evm_params(state.clone(), side).await?;

        Ok(())
    }

    async fn prepare_mint_order(
        state: Rc<RefCell<State>>,
        scheduler: Box<dyn 'static + TaskScheduler<Self>>,
        burn_event: BurntEventData,
        burn_side: BridgeSide,
    ) -> Result<(), SchedulerError> {
        log::trace!("preparing mint order: {burn_event:?}");

        let burn_evm_params = state
            .borrow()
            .config
            .get_evm_params(burn_side)
            .into_scheduler_result()?;

        let mint_evm_params = state
            .borrow()
            .config
            .get_evm_params(burn_side.other())
            .into_scheduler_result()?;

        let recipient = Id256::from_slice(&burn_event.recipient_id)
            .and_then(|id| id.to_evm_address().ok())
            .ok_or_else(|| {
                log::error!("failed to decode recipient data: {burn_event:?}");
                SchedulerError::TaskExecutionFailed("failed to decode recipient data".into())
            })?
            .1;

        let dst_token = Id256::from_slice(&burn_event.to_token)
            .and_then(|id| id.to_evm_address().ok())
            .unwrap_or_default()
            .1;

        let sender_chain_id = burn_evm_params.chain_id as u32;
        let recipient_chain_id = mint_evm_params.chain_id as u32;

        let sender = Id256::from_evm_address(&burn_event.sender, sender_chain_id);
        let src_token = Id256::from_evm_address(&burn_event.from_erc20, sender_chain_id);

        fn to_array<const N: usize>(data: &[u8]) -> Result<[u8; N], SchedulerError> {
            data.try_into().into_scheduler_result()
        }

        let nonce = burn_event.operation_id;

        let mint_order = MintOrder {
            amount: burn_event.amount,
            sender,
            src_token,
            recipient,
            dst_token,
            nonce,
            sender_chain_id,
            recipient_chain_id,
            name: to_array(&burn_event.name)?,
            symbol: to_array(&burn_event.symbol)?,
            decimals: burn_event.decimals,
            approve_spender: H160::zero(),
            approve_amount: U256::zero(),
            fee_payer: burn_event.sender,
        };

        let signer = state.borrow().signer.get().clone();
        let signed_mint_order = mint_order
            .encode_and_sign(&signer)
            .await
            .into_scheduler_result()?;

        state
            .borrow_mut()
            .mint_orders
            .insert(sender, src_token, nonce, signed_mint_order);

        // Update the EVM params
        Self::update_evm_params(state.clone(), burn_side).await?;

        let options = TaskOptions::default();
        scheduler.append_task(
            BridgeTask::SendMintTransaction(signed_mint_order.0.to_vec(), burn_side.other())
                .into_scheduled(options),
        );

        log::trace!("Mint order added");

        Ok(())
    }

    fn task_by_log(log: Log, sender_side: BridgeSide) -> Option<ScheduledTask<BridgeTask>> {
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
                let mint_order_task = BridgeTask::PrepareMintOrder(burnt, sender_side);
                return Some(mint_order_task.into_scheduled(options));
            }
            Ok(BridgeEvent::Minted(minted)) => {
                log::debug!("Adding RemoveMintOrder task");
                let remove_mint_order_task = BridgeTask::RemoveMintOrder(minted);
                return Some(remove_mint_order_task.into_scheduled(options));
            }
            Ok(BridgeEvent::WrappedTokenDeployed(_)) => return None,
            Ok(BridgeEvent::Notify(_)) => todo!(),
            Err(e) => log::warn!("collected log is incompatible with expected events: {e}"),
        }

        None
    }

    fn remove_mint_order(
        state: Rc<RefCell<State>>,
        minted_event: MintedEventData,
    ) -> Result<(), SchedulerError> {
        let sender_id = Id256::from_slice(&minted_event.sender_id).ok_or_else(|| {
            SchedulerError::TaskExecutionFailed(
                "failed to decode sender id256 from minted event".into(),
            )
        })?;

        let src_token = Id256::from_slice(&minted_event.from_token).ok_or_else(|| {
            SchedulerError::TaskExecutionFailed(
                "failed to decode token id256 from minted event".into(),
            )
        })?;

        state
            .borrow_mut()
            .mint_orders
            .remove(sender_id, src_token, minted_event.nonce);

        log::trace!("Mint order removed");

        Ok(())
    }

    async fn send_mint_transaction(
        state: Rc<RefCell<State>>,
        order_data: Vec<u8>,
        side: BridgeSide,
    ) -> Result<(), SchedulerError> {
        log::trace!("Sending mint transaction");

        let signer = state.borrow().signer.get().clone();
        let sender = signer.get_address().await.into_scheduler_result()?;

        let evm_info = state.borrow().config.get_evm_info(side);

        let evm_params = state
            .borrow()
            .config
            .get_evm_params(side)
            .into_scheduler_result()?;

        let bft_bridge = &state
            .borrow()
            .config
            .get_bft_bridge_contract(side)
            .ok_or_else(|| {
                log::warn!("failed to send mint transaction: bft bridge is not configured");
                SchedulerError::TaskExecutionFailed("bft bridge is not configured".into())
            })?;

        let client = evm_info.link.get_json_rpc_client();
        let nonce = client
            .get_transaction_count(sender.0, BlockNumber::Latest)
            .await
            .into_scheduler_result()?;

        let mut tx = bft_bridge_api::mint_transaction(
            sender.0,
            bft_bridge.0,
            nonce.into(),
            evm_params.gas_price.into(),
            &order_data,
            evm_params.chain_id as _,
        );

        let signature = signer
            .sign_transaction(&(&tx).into())
            .await
            .into_scheduler_result()?;
        tx.r = signature.r.0;
        tx.s = signature.s.0;
        tx.v = signature.v.0;
        tx.hash = tx.hash();

        let client = evm_info.link.get_json_rpc_client();
        client
            .send_raw_transaction(tx)
            .await
            .into_scheduler_result()?;

        log::trace!("Mint transaction sent");

        Ok(())
    }

    pub async fn update_evm_params(
        state: Rc<RefCell<State>>,
        side: BridgeSide,
    ) -> Result<(), SchedulerError> {
        let evm_info = state.borrow().config.get_evm_info(side);

        let initial_params = state
            .borrow()
            .config
            .get_evm_params(side)
            .into_scheduler_result()?;
        let address = {
            let signer = state.borrow().signer.get().clone();
            signer.get_address().await.into_scheduler_result()?
        };
        // Update the EvmParams
        log::trace!("updating evm params");
        let responses = query::batch_query(
            &evm_info.link.get_json_rpc_client(),
            &[
                QueryType::Nonce {
                    address: address.into(),
                },
                QueryType::GasPrice,
            ],
        )
        .await
        .into_scheduler_result()?;

        let nonce: U256 = responses
            .get_value_by_id(Id::Str(NONCE_ID.into()))
            .into_scheduler_result()?;
        let gas_price: U256 = responses
            .get_value_by_id(Id::Str(GAS_PRICE_ID.into()))
            .into_scheduler_result()?;

        let params = EvmParams {
            nonce: nonce.0.as_u64(),
            gas_price,
            ..initial_params
        };

        state
            .borrow_mut()
            .config
            .update_evm_params(|p| *p = params, side);
        log::trace!("evm params updated");

        Ok(())
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
