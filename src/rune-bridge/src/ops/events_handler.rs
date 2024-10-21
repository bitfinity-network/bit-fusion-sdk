use std::cell::RefCell;
use std::rc::Rc;

use bridge_canister::bridge::OperationAction;
use bridge_canister::runtime::service::fetch_logs::BftBridgeEventHandler;
use bridge_did::event_data::{
    BurntEventData, MintedEventData, MinterNotificationType, NotifyMinterEventData,
};
use bridge_did::operations::{RuneBridgeDepositOp, RuneBridgeOp, RuneBridgeWithdrawOp};
use candid::Decode;

use super::RuneBridgeOpImpl;
use crate::core::withdrawal::RuneWithdrawalPayloadImpl;
use crate::ops::RuneDepositRequestData;
use crate::state::RuneState;

pub struct RuneEventsHandler {
    rune_state: Rc<RefCell<RuneState>>,
}

impl RuneEventsHandler {
    pub fn new(rune_state: Rc<RefCell<RuneState>>) -> Self {
        Self { rune_state }
    }
}

impl BftBridgeEventHandler<RuneBridgeOpImpl> for RuneEventsHandler {
    fn on_wrapped_token_minted(
        &self,
        event: MintedEventData,
    ) -> Option<OperationAction<RuneBridgeOpImpl>> {
        let nonce = event.nonce;
        log::debug!("on_wrapped_token_minted nonce {nonce} {event:?}",);

        let update_to = RuneBridgeOpImpl(RuneBridgeOp::Deposit(
            RuneBridgeDepositOp::MintOrderConfirmed { data: event },
        ));
        Some(OperationAction::Update { nonce, update_to })
    }

    fn on_wrapped_token_burnt(
        &self,
        event: BurntEventData,
    ) -> Option<OperationAction<RuneBridgeOpImpl>> {
        log::debug!("on_wrapped_token_burnt {event:?}");
        let memo = event.memo();
        match RuneWithdrawalPayloadImpl::new(event, &self.rune_state.borrow()) {
            Ok(payload) => {
                let operation = RuneBridgeOpImpl(RuneBridgeOp::Withdraw(
                    RuneBridgeWithdrawOp::CreateTransaction { payload: payload.0 },
                ));
                Some(OperationAction::Create(operation, memo))
            }
            Err(err) => {
                log::warn!("Invalid withdrawal data: {err:?}");
                None
            }
        }
    }

    fn on_minter_notification(
        &self,
        event: NotifyMinterEventData,
    ) -> Option<OperationAction<RuneBridgeOpImpl>> {
        log::debug!("on_minter_notification {event:?}");

        match event.notification_type {
            MinterNotificationType::DepositRequest => {
                match Decode!(&event.user_data, RuneDepositRequestData) {
                    Ok(data) => {
                        let operation = RuneBridgeOpImpl(RuneBridgeOp::Deposit(
                            RuneBridgeDepositOp::AwaitInputs {
                                dst_address: data.dst_address,
                                dst_tokens: data.dst_tokens,
                                requested_amounts: data.amounts,
                            },
                        ));
                        Some(OperationAction::Create(operation, event.memo()))
                    }
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
