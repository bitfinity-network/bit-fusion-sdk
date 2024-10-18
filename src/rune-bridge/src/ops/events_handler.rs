use std::{cell::RefCell, rc::Rc};

use bridge_canister::runtime::{service::fetch_logs::BftBridgeEventHandler, RuntimeState};
use bridge_did::{
    error::{BftResult, Error},
    event_data::{BurntEventData, MintedEventData, MinterNotificationType, NotifyMinterEventData},
    operations::{RuneBridgeDepositOp, RuneBridgeOp, RuneBridgeWithdrawOp},
};
use candid::Decode;

use crate::{
    canister::SharedRuntime, core::withdrawal::RuneWithdrawalPayloadImpl,
    ops::RuneDepositRequestData, state::RuneState,
};

use super::RuneBridgeOpImpl;

pub struct RuneEventsHandler {
    runtime: SharedRuntime,
    rune_state: Rc<RefCell<RuneState>>,
}

impl RuneEventsHandler {
    pub fn new(runtime: SharedRuntime, rune_state: Rc<RefCell<RuneState>>) -> Self {
        Self {
            runtime,
            rune_state,
        }
    }

    fn state(&self) -> RuntimeState<RuneBridgeOpImpl> {
        self.runtime.borrow().state().clone()
    }
}

impl BftBridgeEventHandler for RuneEventsHandler {
    fn on_wrapped_token_minted(&self, event: MintedEventData) -> BftResult<()> {
        let nonce = event.nonce;
        let dst_address = event.recipient.clone();
        log::debug!("on_wrapped_token_minted nonce {nonce} {event:?}",);
        let operation = RuneBridgeOpImpl(RuneBridgeOp::Deposit(
            RuneBridgeDepositOp::MintOrderConfirmed { data: event },
        ));
        self.state()
            .borrow_mut()
            .operations
            .update_by_nonce(&dst_address, nonce, operation);

        Ok(())
    }

    fn on_wrapped_token_burnt(&self, event: BurntEventData) -> BftResult<()> {
        log::debug!("on_wrapped_token_burnt {event:?}");
        let memo = event.memo();
        match RuneWithdrawalPayloadImpl::new(event, &self.rune_state.borrow()) {
            Ok(payload) => {
                let operation = RuneBridgeOpImpl(RuneBridgeOp::Withdraw(
                    RuneBridgeWithdrawOp::CreateTransaction { payload: payload.0 },
                ));
                let op_id = self
                    .state()
                    .borrow_mut()
                    .operations
                    .new_operation(operation.clone(), memo);
                self.runtime.borrow().schedule_operation(op_id, operation);
            }
            Err(err) => {
                return Err(Error::FailedToProgress(format!(
                    "Invalid withdrawal data: {err:?}"
                )));
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
                        let op_id = self
                            .state()
                            .borrow_mut()
                            .operations
                            .new_operation(operation.clone(), event.memo());
                        self.runtime.borrow().schedule_operation(op_id, operation);
                    }
                    _ => {
                        return Err(Error::Serialization(format!(
                            "Invalid encoded deposit request: {}",
                            hex::encode(&event.user_data)
                        )));
                    }
                }
            }
            _ => {
                return Err(Error::Serialization(format!(
                    "Unsupported minter notification type: {:?}",
                    event.notification_type
                )));
            }
        }

        Ok(())
    }
}
