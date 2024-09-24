mod deposit;
mod withdraw;

use bitcoin::Network;
use bridge_canister::bridge::{Operation, OperationAction, OperationProgress};
use bridge_canister::runtime::RuntimeState;
use bridge_did::brc20_info::Brc20Tick;
use bridge_did::error::{BftResult, Error};
use bridge_did::event_data::{
    BurntEventData, MintedEventData, MinterNotificationType, NotifyMinterEventData,
};
use bridge_did::op_id::OperationId;
use bridge_did::operations::{
    Brc20BridgeDepositOp, Brc20BridgeOp, Brc20BridgeWithdrawOp, DepositRequest,
};
use bridge_did::order::MintOrder;
use candid::{CandidType, Decode, Deserialize};
use did::H160;
use ic_task_scheduler::task::TaskOptions;
use serde::Serialize;
use withdraw::Brc20BridgeWithdrawOpImpl;

pub use self::deposit::Brc20BridgeDepositOpImpl;
use crate::canister::get_brc20_state;
use crate::core::withdrawal::new_withdraw_payload;

/// BRC20 bridge operations
#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub struct Brc20BridgeOpImpl(Brc20BridgeOp);

impl From<Brc20BridgeOp> for Brc20BridgeOpImpl {
    fn from(op: Brc20BridgeOp) -> Self {
        Self(op)
    }
}

impl Operation for Brc20BridgeOpImpl {
    async fn progress(
        self,
        id: OperationId,
        ctx: RuntimeState<Self>,
    ) -> BftResult<OperationProgress<Self>> {
        let next_step = match self.0 {
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::AwaitInputs(deposit)) => {
                log::debug!("Brc20BridgeDepositOp::AwaitInputs {deposit:?}");
                Brc20BridgeDepositOpImpl::await_inputs(ctx, deposit).await
            }
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::AwaitConfirmations { deposit, utxos }) => {
                log::debug!("Brc20BridgeDepositOp::AwaitConfirmations {deposit:?} {utxos:?}");
                Brc20BridgeDepositOpImpl::await_confirmations(ctx, deposit, utxos, id.nonce()).await
            }
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::SignMintOrder(mint_order)) => {
                log::debug!("Brc20BridgeDepositOp::SignMintOrder {mint_order:?}");
                Brc20BridgeDepositOpImpl::sign_mint_order(ctx, id.nonce(), mint_order).await
            }
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::SendMintOrder(mint_order)) => {
                log::debug!("Brc20BridgeDepositOp::SendMintOrder {mint_order:?}");
                Brc20BridgeDepositOpImpl::send_mint_order(ctx, mint_order).await
            }
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::ConfirmMintOrder { .. }) => {
                Err(Error::FailedToProgress(
                    "ConfirmMintOrder task should progress only on the Minted EVM event".into(),
                ))
            }
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::MintOrderConfirmed { .. }) => Err(
                Error::FailedToProgress("MintOrderConfirmed task cannot be progressed".into()),
            ),
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::CreateInscriptionTxs(payload)) => {
                log::debug!("Brc20BridgeDepositOp::CreateInscriptionTxs {payload:?}");
                Brc20BridgeWithdrawOpImpl::create_inscription_txs(payload).await
            }
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::SendCommitTx {
                payload,
                commit_tx,
                reveal_tx,
                reveal_utxo,
            }) => {
                log::debug!(
                    "Brc20BridgeDepositOp::SendCommitTx {payload:?} {commit_tx:?} {reveal_tx:?} {reveal_utxo:?}"
                );
                Brc20BridgeWithdrawOpImpl::send_commit_transaction(
                    payload,
                    commit_tx,
                    reveal_tx,
                    reveal_utxo,
                )
                .await
            }
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::SendRevealTx {
                payload,
                reveal_tx,
                reveal_utxo,
            }) => {
                log::debug!(
                    "Brc20BridgeDepositOp::SendRevealTx {payload:?} {reveal_tx:?} {reveal_utxo:?}"
                );
                Brc20BridgeWithdrawOpImpl::send_reveal_transaction(payload, reveal_tx, reveal_utxo)
                    .await
            }
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::AwaitInscriptionTxs {
                payload,
                reveal_utxo,
            }) => {
                log::debug!(
                    "Brc20BridgeDepositOp::AwaitInscriptionTxs {reveal_utxo:?} {payload:?} "
                );
                Brc20BridgeWithdrawOpImpl::await_inscription_transactions(payload, reveal_utxo)
                    .await
            }
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::CreateTransferTx {
                payload,
                reveal_utxo,
            }) => {
                log::debug!("Brc20BridgeDepositOp::CreateTransferTx {payload:?} {reveal_utxo:?}");
                Brc20BridgeWithdrawOpImpl::create_transfer_transaction(payload, reveal_utxo).await
            }
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::SendTransferTx { from_address, tx }) => {
                log::debug!("Brc20BridgeDepositOp::SendTransferTx {from_address:?} {tx:?}");
                Brc20BridgeWithdrawOpImpl::send_transfer_transaction(from_address, tx).await
            }
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::TransferTxSent { .. }) => Err(
                Error::FailedToProgress("TransferTxSent task cannot be progressed".into()),
            ),
        }?;

        Ok(OperationProgress::Progress(next_step))
    }

    fn scheduling_options(&self) -> Option<ic_task_scheduler::task::TaskOptions> {
        match self.0 {
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::AwaitInscriptionTxs { .. }) => {
                let network = {
                    let state_ref = get_brc20_state();
                    let network = state_ref.borrow().network();

                    network
                };

                // On mainnet wait longer for Bitcoin transactions
                match network {
                    Network::Bitcoin => Some(
                        TaskOptions::new()
                            .with_max_retries_policy(20)
                            .with_fixed_backoff_policy(300), // 10 blocks, each 5 minutes
                    ),
                    _ => Some(
                        TaskOptions::new()
                            .with_max_retries_policy(10)
                            .with_fixed_backoff_policy(10),
                    ),
                }
            }
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::SendCommitTx { .. })
            | Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::SendRevealTx { .. })
            | Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::SendTransferTx { .. })
            | Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::CreateTransferTx { .. })
            | Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::CreateInscriptionTxs { .. }) => Some(
                TaskOptions::new()
                    .with_fixed_backoff_policy(2)
                    .with_max_retries_policy(10),
            ),
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::TransferTxSent { .. })
            | Brc20BridgeOp::Deposit(_) => Some(
                TaskOptions::new()
                    .with_max_retries_policy(10)
                    .with_fixed_backoff_policy(5),
            ),
        }
    }

    fn is_complete(&self) -> bool {
        match self.0 {
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::AwaitInputs { .. }) => false,
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::AwaitConfirmations { .. }) => false,
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::SignMintOrder { .. }) => false,
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::SendMintOrder { .. }) => false,
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::ConfirmMintOrder { .. }) => false,
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::MintOrderConfirmed { .. }) => true,
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::CreateInscriptionTxs { .. }) => false,
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::SendCommitTx { .. }) => false,
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::SendRevealTx { .. }) => false,
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::AwaitInscriptionTxs { .. }) => false,
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::CreateTransferTx { .. }) => false,
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::SendTransferTx { .. }) => false,
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::TransferTxSent { .. }) => true,
        }
    }

    fn evm_wallet_address(&self) -> H160 {
        match &self.0 {
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::AwaitInputs(DepositRequest {
                dst_address,
                ..
            })) => dst_address.clone(),
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::AwaitConfirmations {
                deposit, ..
            }) => deposit.dst_address.clone(),
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::SignMintOrder(MintOrder {
                recipient,
                ..
            })) => recipient.clone(),
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::SendMintOrder(signed_mint_order)) => {
                signed_mint_order.reader().get_recipient()
            }
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::ConfirmMintOrder {
                signed_mint_order,
                ..
            }) => signed_mint_order.reader().get_recipient(),
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::MintOrderConfirmed { data }) => {
                data.recipient.clone()
            }
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::CreateInscriptionTxs(payload)) => {
                payload.sender.clone()
            }
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::SendCommitTx { payload, .. }) => {
                payload.sender.clone()
            }
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::SendRevealTx { payload, .. }) => {
                payload.sender.clone()
            }
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::AwaitInscriptionTxs {
                payload, ..
            }) => payload.sender.clone(),
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::CreateTransferTx {
                payload, ..
            }) => payload.sender.clone(),
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::SendTransferTx {
                from_address, ..
            }) => from_address.clone(),
            Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::TransferTxSent {
                from_address, ..
            }) => from_address.clone(),
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
            update_to: Self(Brc20BridgeOp::Deposit(
                Brc20BridgeDepositOp::MintOrderConfirmed { data: event },
            )),
        })
    }

    async fn on_wrapped_token_burnt(
        _ctx: RuntimeState<Self>,
        event: BurntEventData,
    ) -> Option<OperationAction<Self>> {
        log::debug!("on_wrapped_token_burnt {event:?}");
        let memo = event.memo();
        match new_withdraw_payload(event, &get_brc20_state().borrow()) {
            Ok(payload) => Some(OperationAction::Create(
                Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::CreateInscriptionTxs(payload))
                    .into(),
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
        let memo = event.memo();
        if let Some(notification) = Brc20MinterNotification::decode(event.clone()) {
            match notification {
                Brc20MinterNotification::Deposit(payload) => Some(OperationAction::Create(
                    Self(Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::AwaitInputs(
                        DepositRequest {
                            amount: payload.amount,
                            brc20_tick: payload.brc20_tick,
                            dst_address: payload.dst_address,
                            dst_token: payload.dst_token,
                        },
                    ))),
                    memo,
                )),
            }
        } else {
            log::warn!("Invalid minter notification: {event:?}");
            None
        }
    }
}

pub enum Brc20MinterNotification {
    Deposit(Brc20DepositRequestData),
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub struct Brc20DepositRequestData {
    pub amount: u128,
    pub brc20_tick: Brc20Tick,
    pub dst_address: H160,
    pub dst_token: H160,
}

impl Brc20MinterNotification {
    fn decode(event_data: NotifyMinterEventData) -> Option<Self> {
        match event_data.notification_type {
            MinterNotificationType::DepositRequest => {
                match Decode!(&event_data.user_data, Brc20DepositRequestData) {
                    Ok(payload) => Some(Brc20MinterNotification::Deposit(payload)),
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
