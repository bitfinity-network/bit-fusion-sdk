use bridge_canister::bridge::OperationAction;
use bridge_canister::runtime::service::fetch_logs::BftBridgeEventHandler;
use bridge_did::event_data::{BurntEventData, MintedEventData, NotifyMinterEventData};
use bridge_did::operations::IcrcBridgeOp;
use bridge_did::reason::Icrc2Burn;
use candid::Decode;

use super::IcrcBridgeOpImpl;

pub struct IcrcEventsHandler;

impl BftBridgeEventHandler<IcrcBridgeOpImpl> for IcrcEventsHandler {
    fn on_wrapped_token_minted(
        &self,
        event: MintedEventData,
    ) -> Option<OperationAction<IcrcBridgeOpImpl>> {
        log::trace!("wrapped token minted");
        let nonce = event.nonce;
        let update_to = IcrcBridgeOpImpl(IcrcBridgeOp::WrappedTokenMintConfirmed(event));
        Some(OperationAction::Update { nonce, update_to })
    }

    fn on_wrapped_token_burnt(
        &self,
        event: BurntEventData,
    ) -> Option<OperationAction<IcrcBridgeOpImpl>> {
        log::trace!("wrapped token burnt");
        let memo = event.memo();
        let operation = IcrcBridgeOpImpl(IcrcBridgeOp::MintIcrcTokens(event));

        Some(OperationAction::Create(operation, memo))
    }

    fn on_minter_notification(
        &self,
        event: NotifyMinterEventData,
    ) -> Option<OperationAction<IcrcBridgeOpImpl>> {
        log::debug!("on_minter_notification {event:?}");

        let mut icrc_burn = match Decode!(&event.user_data, Icrc2Burn) {
            Ok(icrc_burn) => icrc_burn,
            Err(e) => {
                log::warn!("failed to decode BftBridge notification into Icrc2Burn: {e}");
                return None;
            }
        };

        // Approve tokens only if the burner owns recipient wallet.
        if event.tx_sender != icrc_burn.recipient_address {
            icrc_burn.approve_after_mint = None;
        }

        let memo = event.memo();
        let operation = IcrcBridgeOpImpl(IcrcBridgeOp::BurnIcrc2Tokens(icrc_burn));
        Some(OperationAction::Create(operation, memo))
    }
}
