use std::collections::HashMap;

use bridge_canister::bridge::{Operation, OperationAction, OperationContext};
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::{BftResult, Error};
use bridge_did::op_id::OperationId;
use bridge_did::order::{MintOrder, SignedMintOrder};
use bridge_utils::bft_events::{
    BurntEventData, MintedEventData, MinterNotificationType, NotifyMinterEventData,
};
use candid::{CandidType, Decode, Deserialize};
use did::{H160, H256};
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use ic_task_scheduler::task::TaskOptions;
use serde::Serialize;

use crate::canister::{get_rune_state, get_runtime};
use crate::core::deposit::RuneDeposit;
use crate::core::withdrawal::{DidTransaction, RuneWithdrawalPayload, Withdrawal};
use crate::rune_info::{RuneInfo, RuneName};

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum RuneBridgeOp {
    // Deposit
    AwaitInputs {
        dst_address: H160,
        dst_tokens: HashMap<RuneName, H160>,
        requested_amounts: Option<HashMap<RuneName, u128>>,
    },
    AwaitConfirmations {
        dst_address: H160,
        utxo: Utxo,
        runes_to_wrap: Vec<RuneToWrap>,
    },
    SignMintOrder {
        dst_address: H160,
        mint_order: MintOrder,
    },
    SendMintOrder {
        dst_address: H160,
        order: SignedMintOrder,
    },
    ConfirmMintOrder {
        dst_address: H160,
        order: SignedMintOrder,
        tx_id: H256,
    },
    MintOrderConfirmed {
        data: MintedEventData,
    },

    // Withdraw
    CreateTransaction {
        payload: RuneWithdrawalPayload,
    },
    SendTransaction {
        from_address: H160,
        transaction: DidTransaction,
    },
    TransactionSent {
        from_address: H160,
        transaction: DidTransaction,
    },

    OperationSplit {
        wallet_address: H160,
        new_operation_ids: Vec<OperationId>,
    },
}

#[derive(Debug, Clone, CandidType, Serialize, Deserialize)]
pub struct RuneToWrap {
    rune_info: RuneInfo,
    amount: u128,
    wrapped_address: H160,
}

impl Operation for RuneBridgeOp {
    async fn progress(self, id: OperationId, ctx: RuntimeState<Self>) -> BftResult<Self> {
        match self {
            RuneBridgeOp::AwaitInputs {
                dst_address,
                dst_tokens,
                requested_amounts,
            } => {
                log::debug!(
                    "RuneBridgeOp::AwaitInputs {dst_address} {dst_tokens:?} {requested_amounts:?}"
                );
                Self::await_inputs(ctx, dst_address, dst_tokens, requested_amounts).await
            }
            RuneBridgeOp::AwaitConfirmations {
                dst_address,
                utxo,
                runes_to_wrap,
            } => {
                log::debug!(
                    "RuneBridgeOp::AwaitConfirmations {dst_address} {utxo:?} {runes_to_wrap:?}"
                );
                Self::await_confirmations(ctx, dst_address, utxo, runes_to_wrap).await
            }
            RuneBridgeOp::SignMintOrder {
                dst_address,
                mint_order,
            } => {
                log::debug!("RuneBridgeOp::SignMintOrder {dst_address} {mint_order:?}");
                Self::sign_mint_order(ctx, id.nonce(), dst_address, mint_order).await
            }
            RuneBridgeOp::SendMintOrder { dst_address, order } => {
                log::debug!("RuneBridgeOp::SendMintOrder {dst_address} {order:?}");
                Self::send_mint_order(ctx, dst_address, order).await
            }
            RuneBridgeOp::ConfirmMintOrder { .. } => Err(Error::FailedToProgress(
                "ConfirmMintOrder task should progress only on the Minted EVM event".into(),
            )),
            RuneBridgeOp::MintOrderConfirmed { .. } => Err(Error::FailedToProgress(
                "MintOrderConfirmed task cannot be progressed".into(),
            )),
            RuneBridgeOp::CreateTransaction { payload } => {
                log::debug!("RuneBridgeOp::CreateTransaction {payload:?}");
                Self::create_withdrawal_transaction(payload).await
            }
            RuneBridgeOp::SendTransaction {
                from_address,
                transaction,
            } => {
                log::debug!("RuneBridgeOp::SendTransaction {from_address} {transaction:?}");
                Self::send_transaction(from_address, transaction).await
            }
            RuneBridgeOp::TransactionSent { .. } => Err(Error::FailedToProgress(
                "TransactionSent task cannot be progressed".into(),
            )),
            RuneBridgeOp::OperationSplit {
                wallet_address,
                new_operation_ids,
            } => {
                log::debug!("RuneBridgeOp::OperationSplit {wallet_address} {new_operation_ids:?}");
                Self::schedule_operation_split(ctx, new_operation_ids).await
            }
        }
    }

    fn scheduling_options(&self) -> Option<ic_task_scheduler::task::TaskOptions> {
        match self {
            Self::SendTransaction { .. } | Self::CreateTransaction { .. } => Some(
                TaskOptions::new()
                    .with_fixed_backoff_policy(2)
                    .with_max_retries_policy(10),
            ),
            Self::AwaitInputs { .. }
            | Self::AwaitConfirmations { .. }
            | Self::SignMintOrder { .. }
            | Self::SendMintOrder { .. }
            | Self::ConfirmMintOrder { .. }
            | Self::MintOrderConfirmed { .. }
            | Self::TransactionSent { .. }
            | Self::OperationSplit { .. } => Some(
                TaskOptions::new()
                    .with_max_retries_policy(10)
                    .with_fixed_backoff_policy(5),
            ),
        }
    }

    fn is_complete(&self) -> bool {
        match self {
            RuneBridgeOp::AwaitInputs { .. } => false,
            RuneBridgeOp::AwaitConfirmations { .. } => false,
            RuneBridgeOp::SignMintOrder { .. } => false,
            RuneBridgeOp::SendMintOrder { .. } => false,
            RuneBridgeOp::ConfirmMintOrder { .. } => false,
            RuneBridgeOp::MintOrderConfirmed { .. } => true,
            RuneBridgeOp::CreateTransaction { .. } => false,
            RuneBridgeOp::SendTransaction { .. } => false,
            RuneBridgeOp::TransactionSent { .. } => true,
            RuneBridgeOp::OperationSplit { .. } => false,
        }
    }

    fn evm_wallet_address(&self) -> H160 {
        match self {
            RuneBridgeOp::AwaitInputs { dst_address, .. } => dst_address.clone(),
            RuneBridgeOp::AwaitConfirmations { dst_address, .. } => dst_address.clone(),
            RuneBridgeOp::SignMintOrder { dst_address, .. } => dst_address.clone(),
            RuneBridgeOp::SendMintOrder { dst_address, .. } => dst_address.clone(),
            RuneBridgeOp::ConfirmMintOrder { dst_address, .. } => dst_address.clone(),
            RuneBridgeOp::MintOrderConfirmed { data } => data.recipient.clone(),
            RuneBridgeOp::CreateTransaction { payload } => payload.sender.clone(),
            RuneBridgeOp::SendTransaction { from_address, .. } => from_address.clone(),
            RuneBridgeOp::TransactionSent { from_address, .. } => from_address.clone(),
            RuneBridgeOp::OperationSplit { wallet_address, .. } => wallet_address.clone(),
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
            update_to: Self::MintOrderConfirmed { data: event },
        })
    }

    async fn on_wrapped_token_burnt(
        _ctx: RuntimeState<Self>,
        event: BurntEventData,
    ) -> Option<OperationAction<Self>> {
        log::debug!("on_wrapped_token_burnt {event:?}");
        let memo = event.memo();
        match RuneWithdrawalPayload::new(event, &get_rune_state().borrow()) {
            Ok(payload) => Some(OperationAction::Create(
                Self::CreateTransaction { payload },
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
        if let Some(notification) = RuneMinterNotification::decode(event.clone()) {
            match notification {
                RuneMinterNotification::Deposit(payload) => Some(OperationAction::Create(
                    Self::AwaitInputs {
                        dst_address: payload.dst_address,
                        dst_tokens: payload.dst_tokens,
                        requested_amounts: payload.amounts,
                    },
                    event.memo(),
                )),
            }
        } else {
            log::warn!("Invalid minter notification: {event:?}");
            None
        }
    }
}

impl RuneBridgeOp {
    fn split(state: RuntimeState<Self>, wallet_address: H160, operations: Vec<Self>) -> Self {
        let mut state = state.borrow_mut();
        let ids = operations
            .into_iter()
            .map(|op| state.operations.new_operation(op, None))
            .collect();
        Self::OperationSplit {
            wallet_address,
            new_operation_ids: ids,
        }
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
        dst_address: H160,
        dst_tokens: HashMap<RuneName, H160>,
        requested_amounts: Option<HashMap<RuneName, u128>>,
    ) -> BftResult<Self> {
        let deposit = RuneDeposit::get(state.clone());
        let inputs = deposit.get_inputs(&dst_address).await.map_err(|err| {
            Error::FailedToProgress(format!("cannot find deposit inputs: {err:?}"))
        })?;

        if inputs.is_empty() {
            return Err(Error::FailedToProgress("no inputs".to_string()));
        }

        if let Some(requested) = &requested_amounts {
            let actual = inputs.rune_amounts();
            if actual != *requested {
                return Err(Error::FailedToProgress(format!(
                    "requested amounts {requested:?} are not equal actual amounts {actual:?}"
                )));
            }
        }

        let mut operations = vec![];
        for input in inputs.inputs.iter() {
            let infos = deposit
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

            operations.push(Self::AwaitConfirmations {
                dst_address: dst_address.clone(),
                utxo: input.utxo.clone(),
                runes_to_wrap,
            });
        }

        Ok(Self::split_or_update(
            state.clone(),
            dst_address,
            operations,
        ))
    }

    async fn await_confirmations(
        ctx: RuntimeState<Self>,
        dst_address: H160,
        utxo: Utxo,
        runes_to_wrap: Vec<RuneToWrap>,
    ) -> BftResult<Self> {
        let deposit = RuneDeposit::get(ctx.clone());
        deposit
            .check_confirmations(&dst_address, &[utxo.clone()])
            .await
            .map_err(|err| Error::FailedToProgress(format!("inputs are not confirmed: {err:?}")))?;

        let deposit_runes = runes_to_wrap.iter().map(|rune| rune.rune_info).collect();
        deposit
            .deposit(&utxo, &dst_address, deposit_runes)
            .await
            .map_err(|err| Error::FailedToProgress(format!("{err:?}")))?;

        let operations = runes_to_wrap
            .into_iter()
            .map(|to_wrap| {
                let mint_order = deposit.create_unsigned_mint_order(
                    &dst_address,
                    &to_wrap.wrapped_address,
                    to_wrap.amount,
                    to_wrap.rune_info,
                    0,
                );
                Self::SignMintOrder {
                    dst_address: dst_address.clone(),
                    mint_order,
                }
            })
            .collect();

        Ok(Self::split_or_update(ctx, dst_address, operations))
    }

    async fn sign_mint_order(
        ctx: RuntimeState<Self>,
        nonce: u32,
        dst_address: H160,
        mut mint_order: MintOrder,
    ) -> BftResult<Self> {
        // update nonce
        mint_order.nonce = nonce;

        let deposit = RuneDeposit::get(ctx);
        let signed = deposit
            .sign_mint_order(mint_order)
            .await
            .map_err(|err| Error::FailedToProgress(format!("cannot sign mint order: {err:?}")))?;

        Ok(Self::SendMintOrder {
            dst_address,
            order: signed,
        })
    }

    async fn send_mint_order(
        ctx: RuntimeState<Self>,
        dst_address: H160,
        order: SignedMintOrder,
    ) -> BftResult<Self> {
        let tx_id = ctx.send_mint_transaction(&order).await?;
        Ok(Self::ConfirmMintOrder {
            dst_address,
            order,
            tx_id,
        })
    }

    async fn create_withdrawal_transaction(payload: RuneWithdrawalPayload) -> BftResult<Self> {
        let withdraw = Withdrawal::get();
        let from_address = payload.sender.clone();
        let transaction = withdraw
            .create_withdrawal_transaction(payload)
            .await
            .map_err(|err| {
                Error::FailedToProgress(format!("cannot create withdrawal transaction: {err:?}"))
            })?;

        Ok(Self::SendTransaction {
            from_address,
            transaction: transaction.into(),
        })
    }

    async fn send_transaction(from_address: H160, transaction: DidTransaction) -> BftResult<Self> {
        let withdraw = Withdrawal::get();
        withdraw
            .send_transaction(transaction.clone().into())
            .await
            .map_err(|err| {
                Error::FailedToProgress(format!("failed to send transaction: {err:?}"))
            })?;

        Ok(Self::TransactionSent {
            from_address,
            transaction,
        })
    }
}

pub enum RuneMinterNotification {
    Deposit(RuneDepositRequestData),
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct RuneDepositRequestData {
    pub dst_address: H160,
    pub dst_tokens: HashMap<RuneName, H160>,
    pub amounts: Option<HashMap<RuneName, u128>>,
}

impl RuneMinterNotification {
    fn decode(event_data: NotifyMinterEventData) -> Option<Self> {
        match event_data.notification_type {
            MinterNotificationType::DepositRequest => {
                match Decode!(&event_data.user_data, RuneDepositRequestData) {
                    Ok(payload) => Some(Self::Deposit(payload)),
                    Err(err) => {
                        log::warn!("Failed to decode deposit request event data: {err:?}");
                        None
                    }
                }
            }
            t => {
                log::warn!("Unknown minter notify event type: {t}");
                None
            }
        }
    }
}
