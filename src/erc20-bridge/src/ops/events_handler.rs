use bridge_canister::bridge::OperationAction;
use bridge_canister::runtime::service::fetch_logs::BftBridgeEventHandler;
use bridge_canister::runtime::state::SharedConfig;
use bridge_did::bridge_side::BridgeSide;
use bridge_did::event_data::{BurntEventData, MintedEventData, NotifyMinterEventData};
use bridge_did::id256::Id256;
use bridge_did::op_id::OperationId;
use bridge_did::operations::{Erc20BridgeOp, Erc20OpStage};
use bridge_did::order::MintOrder;
use bridge_utils::evm_bridge::EvmParams;
use did::{H160, U256};
use ic_stable_structures::CellStructure;

use crate::canister::SharedNonceCounter;
use crate::ops::Erc20BridgeOpImpl;

pub struct Erc20EventsHandler {
    nonce_counter: SharedNonceCounter,
    side: BridgeSide,

    /// Listen events from this EVM.
    src_evm_config: SharedConfig,

    /// Send transactions to this EVM.
    dst_evm_config: SharedConfig,
}

impl Erc20EventsHandler {
    /// Creates new events handler instance, which listens `src_evm` and
    /// create operations for `dst_evm`.
    pub fn new(
        nonce_counter: SharedNonceCounter,
        side: BridgeSide,
        src_evm_config: SharedConfig,
        dst_evm_config: SharedConfig,
    ) -> Self {
        Self {
            nonce_counter,
            side,
            src_evm_config,
            dst_evm_config,
        }
    }
}

impl BftBridgeEventHandler<Erc20BridgeOpImpl> for Erc20EventsHandler {
    fn on_wrapped_token_minted(
        &self,
        event: MintedEventData,
    ) -> Option<OperationAction<Erc20BridgeOpImpl>> {
        log::trace!("wrapped token minted. Updating operation to the complete state...");

        let nonce = event.nonce;
        let update_to = Erc20BridgeOpImpl(Erc20BridgeOp {
            side: self.side,
            stage: Erc20OpStage::TokenMintConfirmed(event),
        });

        Some(OperationAction::Update { nonce, update_to })
    }

    fn on_wrapped_token_burnt(
        &self,
        event: BurntEventData,
    ) -> Option<OperationAction<Erc20BridgeOpImpl>> {
        log::trace!("Wrapped token burnt. Preparing mint order for other side...");

        // Panic here to make the runtime re-process the events when EVM params will be initialized.
        let src_evm_params = self.src_evm_config.borrow().get_evm_params().expect(
            "on_wrapped_token_burnt should not be called if source evm params are not initialized",
        );
        let dst_evm_params = self.dst_evm_config.borrow().get_evm_params().expect(
            "on_wrapped_token_burnt should not be called if base evm params are not initialized",
        );

        let nonce = {
            let mut counter = self.nonce_counter.borrow_mut();
            let nonce = *counter.get();
            counter
                .set(nonce + 1)
                .expect("failed to update nonce counter");
            nonce
        };

        let Some(order) =
            mint_order_from_burnt_event(event.clone(), src_evm_params, dst_evm_params, nonce)
        else {
            log::warn!("failed to create a mint order for event: {event:?}");
            return None;
        };

        let operation = Erc20BridgeOpImpl(Erc20BridgeOp {
            side: self.side.other(),
            stage: Erc20OpStage::SignMintOrder(order),
        });
        let memo = event.memo();

        let op_id = OperationId::new(nonce as _);
        Some(OperationAction::CreateWithId(op_id, operation, memo))
    }

    fn on_minter_notification(
        &self,
        event: NotifyMinterEventData,
    ) -> Option<OperationAction<Erc20BridgeOpImpl>> {
        log::debug!("on_minter_notification {event:?}");
        None
    }
}

/// Creates mint order based on burnt event.
pub fn mint_order_from_burnt_event(
    event: BurntEventData,
    burn_side_evm_params: EvmParams,
    mint_side_evm_params: EvmParams,
    nonce: u32,
) -> Option<MintOrder> {
    let sender = Id256::from_evm_address(&event.sender, burn_side_evm_params.chain_id);
    let src_token = Id256::from_evm_address(&event.from_erc20, burn_side_evm_params.chain_id);
    let recipient = Id256::from_slice(&event.recipient_id)?
        .to_evm_address()
        .inspect_err(|err| {
            log::info!(
                "Failed to parse recipeint_id {:?}: {}",
                event.recipient_id,
                err
            )
        })
        .ok()?
        .1;
    let dst_token = Id256::from_slice(&event.to_token)?
        .to_evm_address()
        .inspect_err(|err| log::info!("Failed to parse to_token {:?}: {}", event.to_token, err))
        .ok()?
        .1;

    let order = MintOrder {
        amount: event.amount,
        sender,
        src_token,
        recipient,
        dst_token,
        nonce,
        sender_chain_id: burn_side_evm_params.chain_id,
        recipient_chain_id: mint_side_evm_params.chain_id,
        name: to_array(&event.name)?,
        symbol: to_array(&event.symbol)?,
        decimals: event.decimals,
        approve_spender: H160::default(),
        approve_amount: U256::default(),
        fee_payer: event.sender,
    };

    Some(order)
}

fn to_array<const N: usize>(data: &[u8]) -> Option<[u8; N]> {
    match data.try_into() {
        Ok(arr) => Some(arr),
        Err(e) => {
            log::warn!("failed to convert token metadata into array: {e}");
            None
        }
    }
}
