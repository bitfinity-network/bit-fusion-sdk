use bridge_canister::runtime::{service::fetch_logs::BftBridgeEventHandler, RuntimeState};
use bridge_did::{
    error::BftResult,
    event_data::{BurntEventData, MintedEventData, NotifyMinterEventData},
    operations::{Brc20BridgeDepositOp, Brc20BridgeOp, Brc20BridgeWithdrawOp, DepositRequest},
};

use crate::{
    canister::{get_brc20_state, SharedRuntime},
    core::withdrawal,
    ops::{Brc20BridgeOpImpl, Brc20MinterNotification},
};

pub struct Brc20BftEventsHandler {
    runtime: SharedRuntime,
}

impl Brc20BftEventsHandler {
    pub fn new(runtime: SharedRuntime) -> Self {
        Self { runtime }
    }

    fn state(&self) -> RuntimeState<Brc20BridgeOpImpl> {
        self.runtime.borrow().state().clone()
    }
}

impl BftBridgeEventHandler for Brc20BftEventsHandler {
    fn on_wrapped_token_minted(&self, event: MintedEventData) -> BftResult<()> {
        log::debug!(
            "on_wrapped_token_minted nonce {nonce} {event:?}",
            nonce = event.nonce
        );

        let nonce = event.nonce;
        let dst_address = event.recipient.clone();
        let payload = Brc20BridgeOpImpl(Brc20BridgeOp::Deposit(
            Brc20BridgeDepositOp::MintOrderConfirmed { data: event },
        ));

        self.state()
            .borrow_mut()
            .operations
            .update_by_nonce(&dst_address, nonce, payload);

        Ok(())
    }

    fn on_wrapped_token_burnt(&self, event: BurntEventData) -> BftResult<()> {
        log::debug!("on_wrapped_token_burnt {event:?}");
        let memo = event.memo();
        match withdrawal::new_withdraw_payload(event, &get_brc20_state().borrow()) {
            Ok(payload) => {
                let operation =
                    Brc20BridgeOp::Withdraw(Brc20BridgeWithdrawOp::CreateInscriptionTxs(payload))
                        .into();

                self.state()
                    .borrow_mut()
                    .operations
                    .new_operation(operation, memo);
            }
            Err(err) => {
                let msg = format!("Invalid withdrawal data: {err:?}");
                log::warn!("{msg}");
                return Err(bridge_did::error::Error::Custom { code: 1, msg });
            }
        }

        Ok(())
    }

    fn on_minter_notification(&self, event: NotifyMinterEventData) -> BftResult<()> {
        log::debug!("on_minter_notification {event:?}");

        if let Some(operation_id) = event.try_decode_reschedule_operation_id() {
            self.runtime.borrow().reschedule_operation(operation_id);
            return Ok(());
        }

        let memo = event.memo();
        if let Some(notification) = Brc20MinterNotification::decode(event.clone()) {
            match notification {
                Brc20MinterNotification::Deposit(payload) => {
                    let operation =
                        Brc20BridgeOp::Deposit(Brc20BridgeDepositOp::AwaitInputs(DepositRequest {
                            amount: payload.amount,
                            brc20_tick: payload.brc20_tick,
                            dst_address: payload.dst_address,
                            dst_token: payload.dst_token,
                        }))
                        .into();

                    self.state()
                        .borrow_mut()
                        .operations
                        .new_operation(operation, memo);
                }
            }
        } else {
            let msg = format!("Invalid minter notification: {event:?}");
            log::warn!("{msg}");
            return Err(bridge_did::error::Error::FailedToProgress(msg));
        }

        Ok(())
    }
}
