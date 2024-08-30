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

use crate::brc20_info::Brc20Tick;
use crate::core::deposit::Brc20Deposit;

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub struct DepositRequest {
    amount: u128,
    brc20_tick: Brc20Tick,
    dst_address: H160,
    dst_token: H160,
}

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum Brc20BridgeOp {
    // Deposit
    AwaitInputs(DepositRequest),
    AwaitConfirmations {
        deposit: DepositRequest,
        utxos: Vec<Utxo>,
    },
    SignMintOrder(MintOrder),
    SendMintOrder(SignedMintOrder),
    ConfirmMintOrder {
        signed_mint_order: SignedMintOrder,
        tx_id: H256,
    },
    MintOrderConfirmed {
        data: MintedEventData,
    },
}

impl Operation for Brc20BridgeOp {
    async fn progress(self, id: OperationId, ctx: RuntimeState<Self>) -> BftResult<Self> {
        match self {
            Self::AwaitInputs(deposit) => {
                log::debug!("Self::AwaitInputs {deposit:?}");
                Self::await_inputs(ctx, deposit).await
            }
            Self::AwaitConfirmations { deposit, utxos } => {
                log::debug!("Self::AwaitConfirmations {deposit:?} {utxos:?}");
                Self::await_confirmations(ctx, deposit, utxos, id.nonce()).await
            }
            Self::SignMintOrder(mint_order) => {
                log::debug!("Self::SignMintOrder {mint_order:?}");
                Self::sign_mint_order(ctx, id.nonce(), mint_order).await
            }
            Self::SendMintOrder(mint_order) => {
                log::debug!("Self::SendMintOrder {mint_order:?}");
                Self::send_mint_order(ctx, mint_order).await
            }
            Self::ConfirmMintOrder { .. } => Err(Error::FailedToProgress(
                "ConfirmMintOrder task should progress only on the Minted EVM event".into(),
            )),
            Self::MintOrderConfirmed { .. } => Err(Error::FailedToProgress(
                "MintOrderConfirmed task cannot be progressed".into(),
            )),
        }
    }

    fn scheduling_options(&self) -> Option<ic_task_scheduler::task::TaskOptions> {
        match self {
            Self::AwaitInputs { .. }
            | Self::AwaitConfirmations { .. }
            | Self::SignMintOrder { .. }
            | Self::SendMintOrder { .. }
            | Self::ConfirmMintOrder { .. }
            | Self::MintOrderConfirmed { .. } => Some(
                TaskOptions::new()
                    .with_max_retries_policy(10)
                    .with_fixed_backoff_policy(5),
            ),
        }
    }

    fn is_complete(&self) -> bool {
        match self {
            Self::AwaitInputs { .. } => false,
            Self::AwaitConfirmations { .. } => false,
            Self::SignMintOrder { .. } => false,
            Self::SendMintOrder { .. } => false,
            Self::ConfirmMintOrder { .. } => false,
            Self::MintOrderConfirmed { .. } => true,
        }
    }

    fn evm_wallet_address(&self) -> H160 {
        match self {
            Self::AwaitInputs(DepositRequest { dst_address, .. }) => dst_address.clone(),
            Self::AwaitConfirmations { deposit, .. } => deposit.dst_address.clone(),
            Self::SignMintOrder(MintOrder { recipient, .. }) => recipient.clone(),
            Self::SendMintOrder(signed_mint_order) => signed_mint_order.get_recipient(),
            Self::ConfirmMintOrder {
                signed_mint_order, ..
            } => signed_mint_order.get_recipient(),
            Self::MintOrderConfirmed { data } => data.recipient.clone(),
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
        todo!("withdraw");
    }

    async fn on_minter_notification(
        _ctx: RuntimeState<Self>,
        event: NotifyMinterEventData,
    ) -> Option<OperationAction<Self>> {
        log::debug!("on_minter_notification {event:?}");
        if let Some(notification) = Brc20MinterNotification::decode(event.clone()) {
            match notification {
                Brc20MinterNotification::Deposit(payload) => {
                    Some(OperationAction::Create(Self::AwaitInputs(DepositRequest {
                        amount: payload.amount,
                        brc20_tick: payload.brc20_tick,
                        dst_address: payload.dst_address,
                        dst_token: payload.dst_token,
                    })))
                }
            }
        } else {
            log::warn!("Invalid minter notification: {event:?}");
            None
        }
    }
}

impl Brc20BridgeOp {
    async fn await_inputs(state: RuntimeState<Self>, request: DepositRequest) -> BftResult<Self> {
        let deposit = Brc20Deposit::get(state.clone());
        let utxos = deposit
            .get_inputs(&request.dst_address)
            .await
            .map_err(|err| {
                Error::FailedToProgress(format!("cannot find deposit inputs: {err:?}"))
            })?;

        if utxos.is_empty() {
            return Err(Error::FailedToProgress("no inputs".to_string()));
        }

        Ok(Self::AwaitConfirmations {
            deposit: request,
            utxos,
        })
    }

    async fn await_confirmations(
        state: RuntimeState<Self>,
        deposit_request: DepositRequest,
        utxos: Vec<Utxo>,
        nonce: u32,
    ) -> BftResult<Self> {
        let DepositRequest {
            amount,
            brc20_tick,
            dst_address,
            dst_token,
        } = deposit_request;

        let deposit = Brc20Deposit::get(state.clone());
        deposit
            .check_confirmations(&dst_address, &utxos)
            .await
            .map_err(|err| Error::FailedToProgress(format!("inputs are not confirmed: {err:?}")))?;

        // check balance
        let brc20_balance = deposit
            .get_brc20_balance(&dst_address, &brc20_tick)
            .await
            .map_err(|err| Error::FailedToProgress(format!("cannot get brc20 balance: {err:?}")))?;

        let brc20_info =
            deposit
                .get_brc20_info(&brc20_tick)
                .await
                .ok_or(Error::FailedToProgress(format!(
                    "cannot get brc20 info for {brc20_tick}"
                )))?;

        if amount > brc20_balance {
            return Err(Error::FailedToProgress(format!(
                "requested amount {amount} is bigger than actual balance {brc20_balance}"
            )));
        }

        let unsigned_mint_order =
            deposit.create_unsigned_mint_order(&dst_address, &dst_token, amount, brc20_info, nonce);

        Ok(Self::SignMintOrder(unsigned_mint_order))
    }

    async fn sign_mint_order(
        ctx: RuntimeState<Self>,
        nonce: u32,
        mut mint_order: MintOrder,
    ) -> BftResult<Self> {
        // update nonce
        mint_order.nonce = nonce;

        let deposit = Brc20Deposit::get(ctx);
        let signed = deposit
            .sign_mint_order(mint_order)
            .await
            .map_err(|err| Error::FailedToProgress(format!("cannot sign mint order: {err:?}")))?;

        Ok(Self::SendMintOrder(signed))
    }

    async fn send_mint_order(ctx: RuntimeState<Self>, order: SignedMintOrder) -> BftResult<Self> {
        let tx_id = ctx.send_mint_transaction(&order).await?;
        Ok(Self::ConfirmMintOrder {
            signed_mint_order: order,
            tx_id,
        })
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
