use bridge_canister::runtime::{service::fetch_logs::BftBridgeEventHandler, RuntimeState};
use bridge_did::{
    error::{BftResult, Error},
    event_data::{BurntEventData, MintedEventData, NotifyMinterEventData},
    operations::IcrcBridgeOp,
    reason::Icrc2Burn,
};
use candid::Decode;

use crate::canister::SharedRuntime;

use super::IcrcBridgeOpImpl;

pub struct IcrcEventsHandler {
    runtime: SharedRuntime,
}

impl IcrcEventsHandler {
    pub fn new(runtime: SharedRuntime) -> Self {
        Self { runtime }
    }

    fn state(&self) -> RuntimeState<IcrcBridgeOpImpl> {
        self.runtime.borrow().state().clone()
    }
}

impl BftBridgeEventHandler for IcrcEventsHandler {
    fn on_wrapped_token_minted(&self, event: MintedEventData) -> BftResult<()> {
        log::trace!("wrapped token minted");
        let dst_address = event.recipient.clone();
        let nonce = event.nonce;
        let operation = IcrcBridgeOpImpl(IcrcBridgeOp::WrappedTokenMintConfirmed(event));
        self.state()
            .borrow_mut()
            .operations
            .update_by_nonce(&dst_address, nonce, operation);

        Ok(())
    }

    fn on_wrapped_token_burnt(&self, event: BurntEventData) -> BftResult<()> {
        log::trace!("wrapped token burnt");
        let memo = event.memo();
        let operation = IcrcBridgeOpImpl(IcrcBridgeOp::MintIcrcTokens(event));

        let op_id = self
            .state()
            .borrow_mut()
            .operations
            .new_operation(operation.clone(), memo);
        self.runtime.borrow().schedule_operation(op_id, operation);

        Ok(())
    }

    fn on_minter_notification(&self, event: NotifyMinterEventData) -> BftResult<()> {
        log::debug!("on_minter_notification {event:?}");

        if let Some(operation_id) = event.try_decode_reschedule_operation_id() {
            self.runtime.borrow().reschedule_operation(operation_id);
            return Ok(());
        }

        let mut icrc_burn = match Decode!(&event.user_data, Icrc2Burn) {
            Ok(icrc_burn) => icrc_burn,
            Err(e) => {
                let msg = format!("failed to decode BftBridge notification into Icrc2Burn: {e}");
                return Err(Error::Serialization(msg));
            }
        };

        // Approve tokens only if the burner owns recipient wallet.
        if event.tx_sender != icrc_burn.recipient_address {
            icrc_burn.approve_after_mint = None;
        }

        let memo = event.memo();

        let operation = IcrcBridgeOpImpl(IcrcBridgeOp::BurnIcrc2Tokens(icrc_burn));
        let op_id = self
            .state()
            .borrow_mut()
            .operations
            .new_operation(operation.clone(), memo);
        self.runtime.borrow().schedule_operation(op_id, operation);

        Ok(())
    }
}
