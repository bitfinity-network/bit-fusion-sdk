mod mint_order_handler;
mod mint_tx_handler;

use std::collections::HashMap;

use bridge_canister::bridge::{Operation, OperationAction, OperationProgress};
use bridge_canister::runtime::service::ServiceId;
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::{BftResult, Error};
use bridge_did::event_data::*;
use bridge_did::op_id::OperationId;
use bridge_did::operations::{RuneBridgeDepositOp, RuneBridgeOp, RuneBridgeWithdrawOp};
use bridge_did::runes::{DidTransaction, RuneName, RuneToWrap, RuneWithdrawalPayload};
use candid::{CandidType, Decode, Deserialize};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use ic_task_scheduler::task::TaskOptions;
use serde::Serialize;

pub use self::mint_order_handler::RuneMintOrderHandler;
pub use self::mint_tx_handler::RuneMintTxHandler;
use crate::canister::{get_rune_state, get_runtime};
use crate::core::deposit::RuneDeposit;
use crate::core::rune_inputs::RuneInputProvider;
use crate::core::utxo_handler::UtxoHandler;
use crate::core::withdrawal::{RuneWithdrawalPayloadImpl, Withdrawal};

pub const SIGN_MINT_ORDER_SERVICE_ID: ServiceId = 0;
pub const SEND_MINT_TX_SERVICE_ID: ServiceId = 1;

#[derive(Debug, Serialize, Deserialize, CandidType, Clone, PartialEq, Eq)]
pub struct RuneBridgeOpImpl(pub RuneBridgeOp);

impl Operation for RuneBridgeOpImpl {
    async fn progress(
        self,
        id: OperationId,
        ctx: RuntimeState<Self>,
    ) -> BftResult<OperationProgress<Self>> {
        let next_step = match self.0 {
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::AwaitInputs {
                dst_address,
                dst_tokens,
                requested_amounts,
            }) => {
                let input_provider = RuneDeposit::get(ctx.clone()).map_err(|err| {
                    Error::FailedToProgress(format!("cannot get deposit: {err:?}"))
                })?;
                log::debug!(
                    "RuneBridgeOp::AwaitInputs {dst_address} {dst_tokens:?} {requested_amounts:?}"
                );
                Self::await_inputs(
                    ctx.clone(),
                    &input_provider,
                    dst_address,
                    dst_tokens,
                    requested_amounts,
                )
                .await
            }
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::AwaitConfirmations {
                dst_address,
                utxo,
                runes_to_wrap,
            }) => {
                let input_provider = RuneDeposit::get(ctx.clone()).map_err(|err| {
                    Error::FailedToProgress(format!("cannot get deposit: {err:?}"))
                })?;
                log::debug!(
                    "RuneBridgeOp::AwaitConfirmations {dst_address} {utxo:?} {runes_to_wrap:?}"
                );
                Self::await_confirmations(
                    ctx.clone(),
                    &input_provider,
                    dst_address,
                    utxo,
                    runes_to_wrap,
                )
                .await
            }
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::SignMintOrder(mut mint_order)) => {
                log::debug!("RuneBridgeOp::SignMintOrder {mint_order:?}");
                // set mint order nonce to new operation id
                mint_order.nonce = id.nonce();
                log::debug!(
                    "RuneBridgeOp::SignMintOrder nonce updated to {}",
                    mint_order.nonce
                );
                let new_op = RuneBridgeOpImpl(RuneBridgeOp::Deposit(
                    RuneBridgeDepositOp::SignMintOrder(mint_order),
                ));
                // update the mint order
                ctx.borrow_mut().operations.update(id, new_op.clone());

                return Ok(OperationProgress::AddToService(SIGN_MINT_ORDER_SERVICE_ID));
            }
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::SendMintOrder(order)) => {
                log::debug!("RuneBridgeOp::SendMintOrder {order:?}");

                return Ok(OperationProgress::AddToService(SEND_MINT_TX_SERVICE_ID));
            }
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::ConfirmMintOrder { .. }) => {
                Err(Error::FailedToProgress(
                    "ConfirmMintOrder task should progress only on the Minted EVM event".into(),
                ))
            }
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::MintOrderConfirmed { .. }) => Err(
                Error::FailedToProgress("MintOrderConfirmed task cannot be progressed".into()),
            ),
            RuneBridgeOp::Withdraw(RuneBridgeWithdrawOp::CreateTransaction { payload }) => {
                log::debug!("RuneBridgeOp::CreateTransaction {payload:?}");
                Self::create_withdrawal_transaction(payload).await
            }
            RuneBridgeOp::Withdraw(RuneBridgeWithdrawOp::SendTransaction {
                from_address,
                transaction,
            }) => {
                log::debug!("RuneBridgeOp::SendTransaction {from_address} {transaction:?}");
                Self::send_transaction(from_address, transaction).await
            }
            RuneBridgeOp::Withdraw(RuneBridgeWithdrawOp::TransactionSent { .. }) => Err(
                Error::FailedToProgress("TransactionSent task cannot be progressed".into()),
            ),
            RuneBridgeOp::OperationSplit {
                wallet_address,
                new_operation_ids,
            } => {
                log::debug!("RuneBridgeOp::OperationSplit {wallet_address} {new_operation_ids:?}");
                Self::schedule_operation_split(ctx, new_operation_ids).await
            }
        };
        Ok(OperationProgress::Progress(next_step?))
    }

    fn is_complete(&self) -> bool {
        match self.0 {
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::AwaitInputs { .. }) => false,
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::AwaitConfirmations { .. }) => false,
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::SignMintOrder(_)) => false,
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::SendMintOrder(_)) => false,
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::ConfirmMintOrder { .. }) => false,
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::MintOrderConfirmed { .. }) => true,
            RuneBridgeOp::Withdraw(RuneBridgeWithdrawOp::CreateTransaction { .. }) => false,
            RuneBridgeOp::Withdraw(RuneBridgeWithdrawOp::SendTransaction { .. }) => false,
            RuneBridgeOp::Withdraw(RuneBridgeWithdrawOp::TransactionSent { .. }) => true,
            RuneBridgeOp::OperationSplit { .. } => false,
        }
    }

    fn evm_wallet_address(&self) -> H160 {
        match &self.0 {
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::AwaitInputs { dst_address, .. }) => {
                dst_address.clone()
            }
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::AwaitConfirmations {
                dst_address, ..
            }) => dst_address.clone(),
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::SignMintOrder(mint_order)) => {
                mint_order.recipient.clone()
            }
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::SendMintOrder(order)) => {
                order.reader().get_recipient()
            }
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::ConfirmMintOrder { order, .. }) => {
                order.reader().get_recipient()
            }
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::MintOrderConfirmed { data }) => {
                data.recipient.clone()
            }
            RuneBridgeOp::Withdraw(RuneBridgeWithdrawOp::CreateTransaction { payload }) => {
                payload.sender.clone()
            }
            RuneBridgeOp::Withdraw(RuneBridgeWithdrawOp::SendTransaction {
                from_address, ..
            }) => from_address.clone(),
            RuneBridgeOp::Withdraw(RuneBridgeWithdrawOp::TransactionSent {
                from_address, ..
            }) => from_address.clone(),
            RuneBridgeOp::OperationSplit { wallet_address, .. } => wallet_address.clone(),
        }
    }

    fn scheduling_options(&self) -> Option<ic_task_scheduler::task::TaskOptions> {
        match self.0 {
            RuneBridgeOp::Withdraw(RuneBridgeWithdrawOp::SendTransaction { .. })
            | RuneBridgeOp::Withdraw(RuneBridgeWithdrawOp::CreateTransaction { .. }) => Some(
                TaskOptions::new()
                    .with_fixed_backoff_policy(2)
                    .with_max_retries_policy(10),
            ),
            RuneBridgeOp::Deposit(_)
            | RuneBridgeOp::Withdraw(_)
            | RuneBridgeOp::OperationSplit { .. } => Some(
                TaskOptions::new()
                    .with_max_retries_policy(10)
                    .with_fixed_backoff_policy(5),
            ),
        }
    }

    async fn on_wrapped_token_minted(
        _ctx: RuntimeState<Self>,
        event: MintedEventData,
    ) -> Option<OperationAction<Self>> {
        log::debug!(
            "on_wrapped_token_minted nonce {nonce} {event:?}",
            nonce = event.nonce
        );

        Some(OperationAction::Update {
            nonce: event.nonce,
            update_to: Self(RuneBridgeOp::Deposit(
                RuneBridgeDepositOp::MintOrderConfirmed { data: event },
            )),
        })
    }

    async fn on_wrapped_token_burnt(
        _ctx: RuntimeState<Self>,
        event: BurntEventData,
    ) -> Option<OperationAction<Self>> {
        log::debug!("on_wrapped_token_burnt {event:?}");
        let memo = event.memo();
        match RuneWithdrawalPayloadImpl::new(event, &get_rune_state().borrow()) {
            Ok(payload) => Some(OperationAction::Create(
                Self(RuneBridgeOp::Withdraw(
                    RuneBridgeWithdrawOp::CreateTransaction { payload: payload.0 },
                )),
                memo,
            )),
            Err(err) => {
                log::warn!("Invalid withdrawal data: {err:?}");
                None
            }
        }
    }

    async fn on_minter_notification(
        _ctx: RuntimeState<Self>,
        event: NotifyMinterEventData,
    ) -> Option<OperationAction<Self>> {
        log::debug!("on_minter_notification {event:?}");

        match event.notification_type {
            MinterNotificationType::DepositRequest => {
                match Decode!(&event.user_data, RuneDepositRequestData) {
                    Ok(data) => Some(OperationAction::Create(
                        Self(RuneBridgeOp::Deposit(RuneBridgeDepositOp::AwaitInputs {
                            dst_address: data.dst_address,
                            dst_tokens: data.dst_tokens,
                            requested_amounts: data.amounts,
                        })),
                        event.memo(),
                    )),
                    _ => {
                        log::warn!(
                            "Invalid encoded deposit request: {}",
                            hex::encode(&event.user_data)
                        );
                        None
                    }
                }
            }
            _ => {
                log::warn!(
                    "Unsupported minter notification type: {:?}",
                    event.notification_type
                );
                None
            }
        }
    }
}

impl RuneBridgeOpImpl {
    fn split(state: RuntimeState<Self>, wallet_address: H160, operations: Vec<Self>) -> Self {
        let mut state = state.borrow_mut();
        let ids = operations
            .into_iter()
            .map(|op| state.operations.new_operation(op, None))
            .collect();
        Self(RuneBridgeOp::OperationSplit {
            wallet_address,
            new_operation_ids: ids,
        })
    }

    fn split_or_update(
        state: RuntimeState<Self>,
        wallet_address: H160,
        mut operations: Vec<Self>,
    ) -> Self {
        debug_assert!(
            !operations.is_empty(),
            "operations list must contain at least one operation"
        );

        if operations.len() > 1 {
            Self::split(state, wallet_address, operations)
        } else {
            operations.remove(0)
        }
    }

    async fn schedule_operation_split(
        ctx: RuntimeState<Self>,
        operation_ids: Vec<OperationId>,
    ) -> BftResult<Self> {
        let state = ctx.borrow();

        let mut operations = operation_ids
            .into_iter()
            .filter_map(|id| state.operations.get(id).map(|op| (id, op)))
            .collect::<Vec<_>>();

        log::debug!("Splitting operation: {operations:?}");

        let (_, next_op) = operations.pop().ok_or(Error::FailedToProgress(
            "no operations to split".to_string(),
        ))?;

        // schedule the rest of the operations
        for (id, operation) in operations {
            get_runtime().borrow_mut().schedule_operation(id, operation);
        }

        Ok(next_op)
    }

    async fn await_inputs(
        state: RuntimeState<Self>,
        input_provider: &impl RuneInputProvider,
        dst_address: H160,
        dst_tokens: HashMap<RuneName, H160>,
        requested_amounts: Option<HashMap<RuneName, u128>>,
    ) -> BftResult<Self> {
        let inputs = input_provider
            .get_inputs(&dst_address)
            .await
            .map_err(|err| {
                Error::FailedToProgress(format!("failed to get deposit inputs: {err}"))
            })?;

        if inputs.is_empty() {
            return Err(Error::FailedToProgress("no inputs".to_string()));
        }

        if let Some(requested) = &requested_amounts {
            let actual = inputs.rune_amounts();
            if actual != *requested {
                let can_be_fixed = actual.iter().all(|(name, amount)| {
                    requested.get(name).cloned().unwrap_or_default() >= *amount
                });
                return if can_be_fixed {
                    Err(Error::FailedToProgress(format!(
                        "requested amounts {requested:?} are not equal actual amounts {actual:?}"
                    )))
                } else {
                    Err(Error::CannotProgress(format!(
                        "requested amounts {requested:?} cannot be equal actual amounts {actual:?}"
                    )))
                };
            }
        }

        let mut operations = vec![];
        for input in inputs.inputs.iter() {
            let infos = input_provider
                .get_rune_infos(&input.runes)
                .await
                .ok_or_else(|| Error::FailedToProgress("rune info not found".into()))?;
            let mut runes_to_wrap = vec![];
            for (rune_info, amount) in infos.into_iter() {
                let dst_token =
                    dst_tokens
                        .get(&rune_info.name())
                        .ok_or(Error::FailedToProgress(format!(
                            "wrapped token address for rune {} not found",
                            rune_info.name()
                        )))?;
                runes_to_wrap.push(RuneToWrap {
                    rune_info,
                    amount,
                    wrapped_address: dst_token.clone(),
                });
            }

            operations.push(Self(RuneBridgeOp::Deposit(
                RuneBridgeDepositOp::AwaitConfirmations {
                    dst_address: dst_address.clone(),
                    utxo: input.utxo.clone(),
                    runes_to_wrap,
                },
            )));
        }

        Ok(Self::split_or_update(
            state.clone(),
            dst_address,
            operations,
        ))
    }

    async fn await_confirmations(
        ctx: RuntimeState<Self>,
        utxo_handler: &impl UtxoHandler,
        dst_address: H160,
        utxo: Utxo,
        runes_to_wrap: Vec<RuneToWrap>,
    ) -> BftResult<Self> {
        utxo_handler
            .check_confirmations(&dst_address, &utxo)
            .await
            .map_err(|err| Error::FailedToProgress(err.to_string()))?;

        let mint_orders = utxo_handler
            .deposit(&utxo, &dst_address, runes_to_wrap)
            .await
            .map_err(|err| Error::FailedToProgress(err.to_string()))?;

        let operations = mint_orders
            .into_iter()
            .map(|mint_order| {
                Self(RuneBridgeOp::Deposit(RuneBridgeDepositOp::SignMintOrder(
                    mint_order,
                )))
            })
            .collect();

        Ok(Self::split_or_update(ctx, dst_address, operations))
    }

    async fn create_withdrawal_transaction(payload: RuneWithdrawalPayload) -> BftResult<Self> {
        let withdraw = Withdrawal::get()
            .map_err(|err| Error::FailedToProgress(format!("cannot get withdraw: {err:?}")))?;
        let from_address = payload.sender.clone();
        let transaction = withdraw
            .create_withdrawal_transaction(payload)
            .await
            .map_err(|err| {
                Error::FailedToProgress(format!("cannot create withdrawal transaction: {err:?}"))
            })?;

        Ok(Self(RuneBridgeOp::Withdraw(
            RuneBridgeWithdrawOp::SendTransaction {
                from_address,
                transaction: transaction.into(),
            },
        )))
    }

    async fn send_transaction(from_address: H160, transaction: DidTransaction) -> BftResult<Self> {
        let withdraw = Withdrawal::get()
            .map_err(|err| Error::FailedToProgress(format!("cannot get withdraw: {err:?}")))?;
        withdraw
            .send_transaction(transaction.clone().into())
            .await
            .map_err(|err| {
                Error::FailedToProgress(format!("failed to send transaction: {err:?}"))
            })?;

        Ok(Self(RuneBridgeOp::Withdraw(
            RuneBridgeWithdrawOp::TransactionSent {
                from_address,
                transaction,
            },
        )))
    }
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct RuneDepositRequestData {
    pub dst_address: H160,
    pub dst_tokens: HashMap<RuneName, H160>,
    pub amounts: Option<HashMap<RuneName, u128>>,
}

#[cfg(test)]
mod tests;
