use bridge_canister::bridge::{Operation, OperationAction, OperationContext, OperationProgress};
use bridge_canister::runtime::RuntimeState;
use bridge_did::brc20_info::Brc20Tick;
use bridge_did::error::{BftResult, Error};
use bridge_did::event_data::{
    BurntEventData, MintedEventData, MinterNotificationType, NotifyMinterEventData,
};
use bridge_did::op_id::OperationId;
use bridge_did::operations::{Brc20BridgeDepositOp, Brc20BridgeOp, DepositRequest};
use bridge_did::order::{MintOrder, SignedMintOrder};
use candid::{CandidType, Decode, Deserialize};
use did::H160;
use ic_exports::ic_cdk::api::management_canister::bitcoin::Utxo;
use ic_task_scheduler::task::TaskOptions;
use serde::Serialize;

use crate::core::deposit::Brc20Deposit;

#[derive(Debug, CandidType, Serialize, Deserialize, Clone)]
pub struct Brc20BridgeOpImpl(pub Brc20BridgeOp);

impl Operation for Brc20BridgeOpImpl {
    async fn progress(
        self,
        id: OperationId,
        ctx: RuntimeState<Self>,
    ) -> BftResult<OperationProgress<Self>> {
        let next_step = match self.0 {
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::AwaitInputs(deposit)) => {
                log::debug!("Self::AwaitInputs {deposit:?}");
                Self::await_inputs(ctx, deposit).await
            }
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::AwaitConfirmations { deposit, utxos }) => {
                log::debug!("Self::AwaitConfirmations {deposit:?} {utxos:?}");
                Self::await_confirmations(ctx, deposit, utxos, id.nonce()).await
            }
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::SignMintOrder(mint_order)) => {
                log::debug!("Self::SignMintOrder {mint_order:?}");
                Self::sign_mint_order(ctx, id.nonce(), mint_order).await
            }
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::SendMintOrder(mint_order)) => {
                log::debug!("Self::SendMintOrder {mint_order:?}");
                Self::send_mint_order(ctx, mint_order).await
            }
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::ConfirmMintOrder { .. }) => {
                Err(Error::FailedToProgress(
                    "ConfirmMintOrder task should progress only on the Minted EVM event".into(),
                ))
            }
            Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::MintOrderConfirmed { .. }) => Err(
                Error::FailedToProgress("MintOrderConfirmed task cannot be progressed".into()),
            ),
        };

        Ok(OperationProgress::Progress(next_step?))
    }

    fn scheduling_options(&self) -> Option<ic_task_scheduler::task::TaskOptions> {
        match self.0 {
            Brc20BridgeOp::Deposit(_) => Some(
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

impl Brc20BridgeOpImpl {
    /// Await for deposit inputs
    async fn await_inputs(state: RuntimeState<Self>, request: DepositRequest) -> BftResult<Self> {
        let deposit = Brc20Deposit::get(state.clone())
            .map_err(|err| Error::FailedToProgress(format!("cannot deposit: {err:?}")))?;
        let utxos = deposit
            .get_inputs(&request.dst_address)
            .await
            .map_err(|err| {
                Error::FailedToProgress(format!("cannot find deposit inputs: {err:?}"))
            })?;

        if utxos.is_empty() {
            return Err(Error::FailedToProgress("no inputs".to_string()));
        }

        Ok(Self(Brc20BridgeOp::Deposit(
            Brc20BridgeDepositOp::AwaitConfirmations {
                deposit: request,
                utxos,
            },
        )))
    }

    /// Await for minimum IC confirmations
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

        let deposit = Brc20Deposit::get(state.clone())
            .map_err(|err| Error::FailedToProgress(format!("cannot deposit: {err:?}")))?;
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

        Ok(Self(Brc20BridgeOp::Deposit(
            Brc20BridgeDepositOp::SignMintOrder(unsigned_mint_order),
        )))
    }

    /// Sign the provided mint order
    async fn sign_mint_order(
        ctx: RuntimeState<Self>,
        nonce: u32,
        mut mint_order: MintOrder,
    ) -> BftResult<Self> {
        // update nonce
        mint_order.nonce = nonce;

        let deposit = Brc20Deposit::get(ctx)
            .map_err(|err| Error::FailedToProgress(format!("cannot deposit: {err:?}")))?;
        let signed = deposit
            .sign_mint_order(mint_order)
            .await
            .map_err(|err| Error::FailedToProgress(format!("cannot sign mint order: {err:?}")))?;

        Ok(Self(Brc20BridgeOp::Deposit(
            Brc20BridgeDepositOp::SendMintOrder(signed),
        )))
    }

    /// Send the signed mint order to the bridge
    async fn send_mint_order(ctx: RuntimeState<Self>, order: SignedMintOrder) -> BftResult<Self> {
        let tx_id = ctx.send_mint_transaction(&order).await?;

        Ok(Self(Brc20BridgeOp::Deposit(
            Brc20BridgeDepositOp::ConfirmMintOrder {
                signed_mint_order: order,
                tx_id,
            },
        )))
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
