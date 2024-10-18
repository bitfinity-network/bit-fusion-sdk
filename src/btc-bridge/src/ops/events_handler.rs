use bridge_canister::runtime::service::fetch_logs::BftBridgeEventHandler;
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::{BftResult, Error};
use bridge_did::event_data::{BurntEventData, MintedEventData, NotifyMinterEventData};
use bridge_did::operations::BtcBridgeOp;
use bridge_did::reason::BtcDeposit;
use candid::Decode;

use super::BtcBridgeOpImpl;
use crate::canister::SharedRuntime;

pub struct BtcEventsHandler {
    runtime: SharedRuntime,
}

impl BtcEventsHandler {
    pub fn new(runtime: SharedRuntime) -> Self {
        Self { runtime }
    }

    fn state(&self) -> RuntimeState<BtcBridgeOpImpl> {
        self.runtime.borrow().state().clone()
    }
}

impl BftBridgeEventHandler for BtcEventsHandler {
    fn on_wrapped_token_minted(&self, event: MintedEventData) -> BftResult<()> {
        let nonce = event.nonce;
        let dst_address = event.recipient.clone();
        let op = BtcBridgeOpImpl(BtcBridgeOp::Erc20MintConfirmed(event));

        self.state()
            .borrow_mut()
            .operations
            .update_by_nonce(&dst_address, nonce, op);

        Ok(())
    }

    fn on_wrapped_token_burnt(&self, event: BurntEventData) -> BftResult<()> {
        log::trace!("wrapped token burnt");
        let memo = event.memo();
        let op = BtcBridgeOpImpl(BtcBridgeOp::WithdrawBtc(event));

        let op_id = self
            .state()
            .borrow_mut()
            .operations
            .new_operation(op.clone(), memo);
        self.runtime.borrow().schedule_operation(op_id, op);
        Ok(())
    }

    fn on_minter_notification(&self, event: NotifyMinterEventData) -> BftResult<()> {
        log::debug!("on_minter_notification {event:?}");

        if let Some(operation_id) = event.try_decode_reschedule_operation_id() {
            self.runtime.borrow().reschedule_operation(operation_id);
            return Ok(());
        }

        let mut btc_deposit = match Decode!(&event.user_data, BtcDeposit) {
            Ok(icrc_burn) => icrc_burn,
            Err(e) => {
                let msg = format!("failed to decode BftBridge notification into BtcDeposit: {e}");
                log::warn!("{msg}");
                return Err(Error::Serialization(msg));
            }
        };

        // Approve tokens only if the burner owns recipient wallet.
        if event.tx_sender != btc_deposit.recipient {
            btc_deposit.approve_after_mint = None;
        }

        let memo = event.memo();

        let op = BtcBridgeOpImpl(BtcBridgeOp::UpdateCkBtcBalance {
            eth_address: btc_deposit.recipient,
        });
        let op_id = self
            .state()
            .borrow_mut()
            .operations
            .new_operation(op.clone(), memo);
        self.runtime.borrow().schedule_operation(op_id, op);

        Ok(())
    }
}
