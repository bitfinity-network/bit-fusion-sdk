mod deposit;
mod withdraw;

use bridge_canister::bridge::{Operation, OperationAction};
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::{BftResult, Error};
use bridge_did::op_id::OperationId;
use bridge_did::order::MintOrder;
use bridge_utils::bft_events::{
    BurntEventData, MintedEventData, MinterNotificationType, NotifyMinterEventData,
};
use candid::{CandidType, Decode, Deserialize};
use did::H160;
use ic_task_scheduler::task::TaskOptions;
use serde::Serialize;

pub use self::deposit::{Brc20BridgeDepositOp, DepositRequest};
pub use self::withdraw::Brc20BridgeWithdrawOp;
use crate::brc20_info::Brc20Tick;

/// BRC20 bridge operations
#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum Brc20BridgeOp {
    /// Deposit operations
    Deposit(Brc20BridgeDepositOp),
    /// Withdraw operations
    Withdraw(Brc20BridgeWithdrawOp),
}

impl Operation for Brc20BridgeOp {
    async fn progress(self, id: OperationId, ctx: RuntimeState<Self>) -> BftResult<Self> {
        match self {
            Self::Deposit(Brc20BridgeDepositOp::AwaitInputs(deposit)) => {
                log::debug!("Self::AwaitInputs {deposit:?}");
                Brc20BridgeDepositOp::await_inputs(ctx, deposit).await
            }
            Self::Deposit(Brc20BridgeDepositOp::AwaitConfirmations { deposit, utxos }) => {
                log::debug!("Self::AwaitConfirmations {deposit:?} {utxos:?}");
                Brc20BridgeDepositOp::await_confirmations(ctx, deposit, utxos, id.nonce()).await
            }
            Self::Deposit(Brc20BridgeDepositOp::SignMintOrder(mint_order)) => {
                log::debug!("Self::SignMintOrder {mint_order:?}");
                Brc20BridgeDepositOp::sign_mint_order(ctx, id.nonce(), mint_order).await
            }
            Self::Deposit(Brc20BridgeDepositOp::SendMintOrder(mint_order)) => {
                log::debug!("Self::SendMintOrder {mint_order:?}");
                Brc20BridgeDepositOp::send_mint_order(ctx, mint_order).await
            }
            Self::Deposit(Brc20BridgeDepositOp::ConfirmMintOrder { .. }) => {
                Err(Error::FailedToProgress(
                    "ConfirmMintOrder task should progress only on the Minted EVM event".into(),
                ))
            }
            Self::Deposit(Brc20BridgeDepositOp::MintOrderConfirmed { .. }) => Err(
                Error::FailedToProgress("MintOrderConfirmed task cannot be progressed".into()),
            ),
            Self::Withdraw(Brc20BridgeWithdrawOp::CreateInscriptionTxs(payload)) => {
                todo!()
            }
            Self::Withdraw(Brc20BridgeWithdrawOp::SendInscriptionTxs {
                payload,
                commit_tx,
                reveal_tx,
                reveal_utxo,
            }) => {
                todo!()
            }
            Self::Withdraw(Brc20BridgeWithdrawOp::AwaitInscriptionTxs {
                payload,
                reveal_utxo,
            }) => {
                todo!()
            }
            Self::Withdraw(Brc20BridgeWithdrawOp::CreateTransferTx {
                payload,
                reveal_utxo,
            }) => {
                todo!()
            }
            Self::Withdraw(Brc20BridgeWithdrawOp::SendTransferTx { from_address, tx }) => {
                todo!()
            }
            Self::Withdraw(Brc20BridgeWithdrawOp::TransferTxSent { .. }) => Err(
                Error::FailedToProgress("TransferTxSent task cannot be progressed".into()),
            ),
        }
    }

    fn scheduling_options(&self) -> Option<ic_task_scheduler::task::TaskOptions> {
        match self {
            Self::Withdraw(Brc20BridgeWithdrawOp::AwaitInscriptionTxs { .. }) => Some(
                TaskOptions::new()
                    .with_max_retries_policy(20)
                    .with_fixed_backoff_policy(300), // 5 minutes
            ),
            Self::Withdraw(Brc20BridgeWithdrawOp::SendInscriptionTxs { .. })
            | Self::Withdraw(Brc20BridgeWithdrawOp::SendTransferTx { .. })
            | Self::Withdraw(Brc20BridgeWithdrawOp::CreateTransferTx { .. })
            | Self::Withdraw(Brc20BridgeWithdrawOp::CreateInscriptionTxs { .. }) => Some(
                TaskOptions::new()
                    .with_fixed_backoff_policy(2)
                    .with_max_retries_policy(10),
            ),
            Self::Withdraw(Brc20BridgeWithdrawOp::TransferTxSent { .. }) | Self::Deposit(_) => {
                Some(
                    TaskOptions::new()
                        .with_max_retries_policy(10)
                        .with_fixed_backoff_policy(5),
                )
            }
        }
    }

    fn is_complete(&self) -> bool {
        match self {
            Self::Deposit(Brc20BridgeDepositOp::AwaitInputs { .. }) => false,
            Self::Deposit(Brc20BridgeDepositOp::AwaitConfirmations { .. }) => false,
            Self::Deposit(Brc20BridgeDepositOp::SignMintOrder { .. }) => false,
            Self::Deposit(Brc20BridgeDepositOp::SendMintOrder { .. }) => false,
            Self::Deposit(Brc20BridgeDepositOp::ConfirmMintOrder { .. }) => false,
            Self::Deposit(Brc20BridgeDepositOp::MintOrderConfirmed { .. }) => true,
            Self::Withdraw(Brc20BridgeWithdrawOp::CreateInscriptionTxs { .. }) => false,
            Self::Withdraw(Brc20BridgeWithdrawOp::SendInscriptionTxs { .. }) => false,
            Self::Withdraw(Brc20BridgeWithdrawOp::AwaitInscriptionTxs { .. }) => false,
            Self::Withdraw(Brc20BridgeWithdrawOp::CreateTransferTx { .. }) => false,
            Self::Withdraw(Brc20BridgeWithdrawOp::SendTransferTx { .. }) => false,
            Self::Withdraw(Brc20BridgeWithdrawOp::TransferTxSent { .. }) => true,
        }
    }

    fn evm_wallet_address(&self) -> H160 {
        match self {
            Self::Deposit(Brc20BridgeDepositOp::AwaitInputs(DepositRequest {
                dst_address,
                ..
            })) => dst_address.clone(),
            Self::Deposit(Brc20BridgeDepositOp::AwaitConfirmations { deposit, .. }) => {
                deposit.dst_address.clone()
            }
            Self::Deposit(Brc20BridgeDepositOp::SignMintOrder(MintOrder { recipient, .. })) => {
                recipient.clone()
            }
            Self::Deposit(Brc20BridgeDepositOp::SendMintOrder(signed_mint_order)) => {
                signed_mint_order.get_recipient()
            }
            Self::Deposit(Brc20BridgeDepositOp::ConfirmMintOrder {
                signed_mint_order, ..
            }) => signed_mint_order.get_recipient(),
            Self::Deposit(Brc20BridgeDepositOp::MintOrderConfirmed { data }) => {
                data.recipient.clone()
            }
            Self::Withdraw(Brc20BridgeWithdrawOp::CreateInscriptionTxs(payload)) => {
                payload.sender.clone()
            }
            Self::Withdraw(Brc20BridgeWithdrawOp::SendInscriptionTxs { payload, .. }) => {
                payload.sender.clone()
            }
            Self::Withdraw(Brc20BridgeWithdrawOp::AwaitInscriptionTxs { payload, .. }) => {
                payload.sender.clone()
            }
            Self::Withdraw(Brc20BridgeWithdrawOp::CreateTransferTx { payload, .. }) => {
                payload.sender.clone()
            }
            Self::Withdraw(Brc20BridgeWithdrawOp::SendTransferTx { from_address, .. }) => {
                from_address.clone()
            }
            Self::Withdraw(Brc20BridgeWithdrawOp::TransferTxSent { from_address, .. }) => {
                from_address.clone()
            }
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
            update_to: Self::Deposit(Brc20BridgeDepositOp::MintOrderConfirmed { data: event }),
        })
    }

    async fn on_wrapped_token_burnt(
        _ctx: RuntimeState<Self>,
        event: BurntEventData,
    ) -> Option<OperationAction<Self>> {
        log::debug!("on_wrapped_token_burnt {event:?}");
        todo!("withdraw");
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
                    Self::Deposit(Brc20BridgeDepositOp::AwaitInputs(DepositRequest {
                        amount: payload.amount,
                        brc20_tick: payload.brc20_tick,
                        dst_address: payload.dst_address,
                        dst_token: payload.dst_token,
                    })),
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
