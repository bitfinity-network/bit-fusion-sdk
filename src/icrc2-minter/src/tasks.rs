use std::cell::RefCell;
use std::future::Future;
use std::pin::Pin;
use std::rc::Rc;

use candid::{Nat, Principal};
use did::H160;
use eth_signer::sign_strategy::TransactionSigner;
use ethers_core::types::{BlockNumber, Log};
use ic_canister::virtual_canister_call;
use ic_exports::icrc_types::icrc1::account::Account;
use ic_exports::icrc_types::icrc2::approve::ApproveError;
use ic_exports::icrc_types::icrc2::transfer_from::TransferFromError;
use ic_task_scheduler::retry::BackoffPolicy;
use ic_task_scheduler::scheduler::TaskScheduler;
use ic_task_scheduler::task::{ScheduledTask, Task, TaskOptions};
use ic_task_scheduler::SchedulerError;
use minter_contract_utils::bft_bridge_api::{self, BridgeEvent, BurntEventData, MintedEventData};
use minter_contract_utils::evm_bridge::EvmParams;
use minter_did::error::Error;
use minter_did::id256::Id256;
use minter_did::order::MintOrder;
use serde::{Deserialize, Serialize};

use crate::constant::IC_CHAIN_ID;
use crate::state::State;
use crate::tokens::icrc1::{self, IcrcTransferDst};
use crate::tokens::icrc2;

type SignedMintOrderData = Vec<u8>;

#[derive(Debug, Serialize, Deserialize)]
pub enum BridgeTask {
    InitEvmInfo,
    CollectEvmEvents,
    PrepareMintOrder(BurntIcrc2Data),
    RemoveMintOrder(MintedEventData),
    SendMintTransaction(SignedMintOrderData),
    MintIcrc2Tokens(BurntEventData),
}

impl Task for BridgeTask {
    fn execute(
        &self,
        scheduler: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> Pin<Box<dyn Future<Output = Result<(), SchedulerError>>>> {
        let state = crate::canister::get_state();
        match self {
            BridgeTask::InitEvmInfo => Box::pin(Self::init_evm_info(state)),
            BridgeTask::CollectEvmEvents => Box::pin(Self::collect_evm_events(state, scheduler)),
            BridgeTask::PrepareMintOrder(data) => {
                Box::pin(Self::prepare_mint_order(state, scheduler, data.clone()))
            }
            BridgeTask::RemoveMintOrder(data) => {
                let data = data.clone();
                Box::pin(async move { Self::remove_mint_order(state, data) })
            }
            BridgeTask::SendMintTransaction(order_data) => {
                Box::pin(Self::send_mint_transaction(state, order_data.clone()))
            }
            BridgeTask::MintIcrc2Tokens(burn_data) => {
                Box::pin(Self::mint_icrc2(state, burn_data.clone()))
            }
        }
    }
}

impl BridgeTask {
    pub fn into_scheduled(self, options: TaskOptions) -> ScheduledTask<Self> {
        ScheduledTask::with_options(self, options)
    }

    pub async fn init_evm_info(state: Rc<RefCell<State>>) -> Result<(), SchedulerError> {
        log::trace!("evm info initialization started");

        let client = state.borrow().config.get_evm_client();
        let address = {
            let signer = state.borrow().signer.get_transaction_signer();
            signer.get_address().await.into_scheduler_result()?
        };

        let evm_params = EvmParams::query(client, address)
            .await
            .into_scheduler_result()?;

        state
            .borrow_mut()
            .config
            .update_evm_params(|p| *p = evm_params);

        log::trace!("evm parameters initialized");

        Ok(())
    }

    async fn collect_evm_events(
        state: Rc<RefCell<State>>,
        scheduler: Box<dyn 'static + TaskScheduler<Self>>,
    ) -> Result<(), SchedulerError> {
        log::trace!("collecting evm events");

        let client = state.borrow().config.get_evm_client();
        let Some(params) = state.borrow().config.get_evm_params() else {
            log::warn!("no evm parameters set, unable to collect events");
            return Err(SchedulerError::TaskExecutionFailed(
                "no evm parameters set".into(),
            ));
        };
        let Some(bridge_contract) = state.borrow().config.get_bft_bridge_contract() else {
            log::warn!("no bft bridge contract set, unable to collect events");
            return Err(SchedulerError::TaskExecutionFailed(
                "no bft bridge contract set".into(),
            ));
        };

        let logs = BridgeEvent::collect_logs(
            &client,
            params.next_block.into(),
            BlockNumber::Safe,
            bridge_contract.0,
        )
        .await
        .into_scheduler_result()?;

        log::debug!("got evm logs: {logs:?}");

        let mut mut_state = state.borrow_mut();

        // Filter out logs that do not have block number.
        // Such logs are produced when the block is not finalized yet.
        let last_log = logs.iter().take_while(|l| l.block_number.is_some()).last();
        if let Some(last_log) = last_log {
            let next_block_number = last_log.block_number.unwrap().as_u64() + 1;
            mut_state
                .config
                .update_evm_params(|params| params.next_block = next_block_number);
        };

        log::trace!("appending logs to tasks: {logs:?}");

        scheduler.append_tasks(logs.into_iter().filter_map(Self::task_by_log).collect());

        Ok(())
    }

    async fn prepare_mint_order(
        state: Rc<RefCell<State>>,
        scheduler: Box<dyn 'static + TaskScheduler<Self>>,
        burnt_data: BurntIcrc2Data,
    ) -> Result<(), SchedulerError> {
        log::trace!("preparing mint order: {burnt_data:?}");

        let Some(evm_params) = state.borrow().config.get_evm_params() else {
            log::warn!("no evm parameters set, unable to prepare mint order");
            return Err(SchedulerError::TaskExecutionFailed(
                "no evm parameters set".into(),
            ));
        };

        let sender_chain_id = IC_CHAIN_ID;
        let recipient_chain_id = evm_params.chain_id as u32;

        let sender = Id256::from(&burnt_data.sender);
        let src_token = Id256::from(&burnt_data.src_token);

        let nonce = burnt_data.operation_id;

        let mint_order = MintOrder {
            amount: burnt_data.amount,
            sender,
            src_token,
            recipient: burnt_data.recipient_address,
            dst_token: H160::default(), // will be selected in the contract.
            nonce,
            sender_chain_id,
            recipient_chain_id,
            name: burnt_data.name,
            symbol: burnt_data.symbol,
            decimals: burnt_data.decimals,
        };

        let signer = state.borrow().signer.get_transaction_signer();
        let signed_mint_order = mint_order
            .encode_and_sign(&signer)
            .await
            .into_scheduler_result()?;

        state
            .borrow_mut()
            .mint_orders
            .insert(sender, src_token, nonce, &signed_mint_order);

        let options = TaskOptions::default();
        scheduler.append_task(
            BridgeTask::SendMintTransaction(signed_mint_order.0.to_vec()).into_scheduled(options),
        );

        log::trace!("Mint order added");

        Ok(())
    }

    fn task_by_log(log: Log) -> Option<ScheduledTask<BridgeTask>> {
        log::trace!("creating task from the log: {log:?}");

        const TASK_RETRY_DELAY_SECS: u32 = 5;

        let options = TaskOptions::default()
            .with_backoff_policy(BackoffPolicy::Fixed {
                secs: TASK_RETRY_DELAY_SECS,
            })
            .with_max_retries_policy(u32::MAX);

        match BridgeEvent::from_log(log).into_scheduler_result() {
            Ok(BridgeEvent::Burnt(burnt)) => {
                log::debug!("Adding MintIcrc2 task");
                let mint_icrc2_task = BridgeTask::MintIcrc2Tokens(burnt);
                return Some(mint_icrc2_task.into_scheduled(options));
            }
            Ok(BridgeEvent::Minted(minted)) => {
                log::debug!("Adding RemoveMintOrder task");
                let remove_mint_order_task = BridgeTask::RemoveMintOrder(minted);
                return Some(remove_mint_order_task.into_scheduled(options));
            }
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
    ) -> Result<(), SchedulerError> {
        log::trace!("Sending mint transaction");

        let signer = state.borrow().signer.get_transaction_signer();
        let sender = signer.get_address().await.into_scheduler_result()?;
        let Some(bridge_contract) = state.borrow().config.get_bft_bridge_contract() else {
            log::warn!("Bridge contract is not set");
            return Err(SchedulerError::TaskExecutionFailed(
                "Bridge contract is not set".into(),
            ));
        };
        let Some(evm_params) = state.borrow().config.get_evm_params() else {
            log::warn!("No evm parameters set");
            return Err(SchedulerError::TaskExecutionFailed(
                "No evm parameters set".into(),
            ));
        };

        let mut tx = bft_bridge_api::mint_transaction(
            sender.0,
            bridge_contract.0,
            evm_params.nonce.into(),
            evm_params.gas_price.into(),
            order_data,
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

        let client = state.borrow().config.get_evm_client();
        client
            .send_raw_transaction(tx)
            .await
            .into_scheduler_result()?;

        state
            .borrow_mut()
            .config
            .update_evm_params(|p| p.nonce += 1);

        log::trace!("Mint transaction sent");

        Ok(())
    }

    async fn mint_icrc2(
        state: Rc<RefCell<State>>,
        minted_event: BurntEventData,
    ) -> Result<(), SchedulerError> {
        log::trace!("Minting Icrc2 tokens");

        let Some(to_token) =
            Id256::from_slice(&minted_event.to_token).and_then(|id| id.try_into().ok())
        else {
            log::warn!("Failed to decode token id256 from erc20 minted event");
            return Err(SchedulerError::TaskExecutionFailed(
                "Failed to decode token id256 from erc20 minted event".into(),
            ));
        };

        let Some(evm_params) = state.borrow().config.get_evm_params() else {
            log::warn!("no evm parameters set, unable to prepare mint order");
            return Err(SchedulerError::TaskExecutionFailed(
                "no evm parameters set".into(),
            ));
        };

        let Some(recipient) =
            Id256::from_slice(&minted_event.recipient_id).and_then(|id| id.try_into().ok())
        else {
            log::warn!("Failed to decode recipient id from minted event");
            return Err(SchedulerError::TaskExecutionFailed(
                "Failed to decode recipient id from minted event".into(),
            ));
        };

        let spender = state.borrow().config.get_spender_principal();
        let spender_subaccount = icrc2::approve_subaccount(
            minted_event.sender,
            minted_event.operation_id,
            evm_params.chain_id as _,
            to_token,
            recipient,
        );

        let spender_account = Account {
            owner: spender,
            subaccount: Some(spender_subaccount),
        };

        // Approve icrc2 transfer for the spender canister to user.

        let amount = Nat::from(&minted_event.amount);
        let approve_result =
            icrc2::approve_mint(to_token, spender_account, amount.clone(), true).await;
        let allowance = match approve_result {
            Ok(suceess) => suceess.amount,
            Err(Error::Icrc2ApproveError(ApproveError::AllowanceChanged { current_allowance })) => {
                current_allowance
            }
            Err(e) => {
                log::warn!("Failed to approve mint: {:?}", e);
                return Err(SchedulerError::TaskExecutionFailed(
                    "Failed to approve mint".into(),
                ));
            }
        };

        log::trace!("Approved icrc2 mint");

        // Ask spender canister to perform the transfer.

        let dst_info = IcrcTransferDst {
            token: to_token,
            recipient,
        };
        let fee = icrc1::get_token_configuration(to_token)
            .await
            .into_scheduler_result()?
            .fee;
        let allowance_without_fee = allowance.clone() - fee.clone();
        let mut transfer_result = virtual_canister_call!(
            spender,
            "finish_icrc2_mint",
            (dst_info.token, dst_info.recipient, spender_subaccount, allowance_without_fee, fee),
            std::result::Result<Nat, TransferFromError>
        )
        .await
        .map_err(|e| SchedulerError::TaskExecutionFailed(format!("{:?}", e)))?;

        log::debug!("transfer result: {:?}", transfer_result);

        if let Err(TransferFromError::BadFee { expected_fee }) = transfer_result {
            // refresh cached token configuration if fee changed
            let _ = icrc1::refresh_token_configuration(dst_info.token).await;

            let allowance_without_fee = allowance - expected_fee.clone();
            transfer_result = virtual_canister_call!(
                spender,
                "finish_icrc2_mint",
                (dst_info, spender_subaccount, allowance_without_fee, expected_fee),
                std::result::Result<Nat, TransferFromError>
            )
            .await
            .map_err(|e| SchedulerError::TaskExecutionFailed(format!("{:?}", e)))?;
        }

        transfer_result.map_err(|e| SchedulerError::TaskExecutionFailed(format!("{:?}", e)))?;

        log::trace!("Finished icrc2 mint");

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BurntIcrc2Data {
    pub sender: Principal,
    pub amount: did::U256,
    pub src_token: Principal,
    pub recipient_address: did::H160,
    pub operation_id: u32,
    pub name: [u8; 32],
    pub symbol: [u8; 16],
    pub decimals: u8,
}
