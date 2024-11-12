use bridge_canister::bridge::OperationAction;
use bridge_canister::runtime::service::fetch_logs::BtfBridgeEventHandler;
use bridge_did::event_data::{BurntEventData, MintedEventData, NotifyMinterEventData};
use bridge_did::operations::BtcBridgeOp;
use bridge_did::reason::BtcDeposit;
use candid::Decode;

use super::BtcBridgeOpImpl;

pub struct BtcEventsHandler;

impl BtfBridgeEventHandler<BtcBridgeOpImpl> for BtcEventsHandler {
    fn on_wrapped_token_minted(
        &self,
        event: MintedEventData,
    ) -> Option<OperationAction<BtcBridgeOpImpl>> {
        let nonce = event.nonce;
        let update_to = BtcBridgeOpImpl(BtcBridgeOp::Erc20MintConfirmed(event));

        Some(OperationAction::Update { nonce, update_to })
    }

    fn on_wrapped_token_burnt(
        &self,
        event: BurntEventData,
    ) -> Option<OperationAction<BtcBridgeOpImpl>> {
        log::trace!("wrapped token burnt");
        let memo = event.memo();
        let op = BtcBridgeOpImpl(BtcBridgeOp::WithdrawBtc(event));
        Some(OperationAction::Create(op, memo))
    }

    fn on_minter_notification(
        &self,
        event: NotifyMinterEventData,
    ) -> Option<OperationAction<BtcBridgeOpImpl>> {
        log::debug!("on_minter_notification {event:?}");

        let mut btc_deposit = match Decode!(&event.user_data, BtcDeposit) {
            Ok(icrc_burn) => icrc_burn,
            Err(e) => {
                log::warn!("failed to decode Btfbridge notification into BtcDeposit: {e}");
                return None;
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
        Some(OperationAction::Create(op, memo))
    }
}
