use std::cell::RefCell;
use std::rc::Rc;

use bridge_canister::bridge::OperationAction;
use bridge_canister::runtime::service::fetch_logs::BtfBridgeEventHandler;
use bridge_did::event_data::{BurntEventData, MintedEventData, NotifyMinterEventData};
use bridge_did::operations::{
    Brc20BridgeDepositOp, Brc20BridgeOp, Brc20BridgeWithdrawOp, DepositRequest,
};

use crate::core::withdrawal;
use crate::ops::{Brc20BridgeOpImpl, Brc20MinterNotification};
use crate::state::Brc20State;

/// Describes event processing logic.
pub struct Brc20BtfEventsHandler {
    brc20_state: Rc<RefCell<Brc20State>>,
}

impl Brc20BtfEventsHandler {
    pub fn new(brc20_state: Rc<RefCell<Brc20State>>) -> Self {
        Self { brc20_state }
    }
}

impl BtfBridgeEventHandler<Brc20BridgeOpImpl> for Brc20BtfEventsHandler {
    fn on_wrapped_token_minted(
        &self,
        event: MintedEventData,
    ) -> Option<OperationAction<Brc20BridgeOpImpl>> {
        log::debug!(
            "on_wrapped_token_minted nonce {nonce} {event:?}",
            nonce = event.nonce
        );

        let nonce = event.nonce;
        let update_to = Brc20BridgeOpImpl(Brc20BridgeOp::Deposit(
            Brc20BridgeDepositOp::MintOrderConfirmed { data: event },
        ));

        Some(OperationAction::Update { nonce, update_to })
    }

    fn on_wrapped_token_burnt(
        &self,
        event: BurntEventData,
    ) -> Option<OperationAction<Brc20BridgeOpImpl>> {
        log::debug!("on_wrapped_token_burnt {event:?}");
        let memo = event.memo();
        let op = match withdrawal::new_withdraw_payload(event, &self.brc20_state.borrow()) {
            Ok(payload) => Brc20BridgeOpImpl(Brc20BridgeOp::Withdraw(
                Brc20BridgeWithdrawOp::CreateInscriptionTxs(payload),
            )),
            Err(err) => {
                log::warn!("Invalid withdrawal data: {err:?}");
                return None;
            }
        };

        Some(OperationAction::Create(op, memo))
    }

    fn on_minter_notification(
        &self,
        event: NotifyMinterEventData,
    ) -> Option<OperationAction<Brc20BridgeOpImpl>> {
        log::debug!("on_minter_notification {event:?}");

        let memo = event.memo();
        let Some(notification) = Brc20MinterNotification::decode(event.clone()) else {
            log::warn!("Invalid minter notification: {event:?}");
            return None;
        };

        match notification {
            Brc20MinterNotification::Deposit(payload) => {
                let operation = Brc20BridgeOpImpl(Brc20BridgeOp::Deposit(
                    Brc20BridgeDepositOp::AwaitInputs(DepositRequest {
                        amount: payload.amount,
                        brc20_tick: payload.brc20_tick,
                        dst_address: payload.dst_address,
                        dst_token: payload.dst_token,
                    }),
                ));

                Some(OperationAction::Create(operation, memo))
            }
        }
    }
}
