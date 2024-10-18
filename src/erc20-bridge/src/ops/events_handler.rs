use bridge_canister::runtime::service::fetch_logs::BftBridgeEventHandler;
use bridge_canister::runtime::state::SharedConfig;
use bridge_canister::runtime::RuntimeState;
use bridge_did::bridge_side::BridgeSide;
use bridge_did::error::{BftResult, Error};
use bridge_did::event_data::{BurntEventData, MintedEventData, NotifyMinterEventData};
use bridge_did::id256::Id256;
use bridge_did::op_id::OperationId;
use bridge_did::operations::{Erc20BridgeOp, Erc20OpStage};
use bridge_did::order::MintOrder;
use bridge_utils::evm_bridge::EvmParams;
use did::{H160, U256};
use ic_stable_structures::CellStructure;

use crate::canister::{SharedNonceCounter, SharedRuntime};
use crate::ops::Erc20BridgeOpImpl;

pub struct Erc20EventsHandler {
    runtime: SharedRuntime,
    nonce_counter: SharedNonceCounter,
    side: BridgeSide,

    /// Listen events from this EVM.
    src_evm_config: SharedConfig,

    /// Send transactions to this EVM.
    dst_evm_config: SharedConfig,
}

impl Erc20EventsHandler {
    pub fn new(
        runtime: SharedRuntime,
        nonce_counter: SharedNonceCounter,
        side: BridgeSide,
        src_evm_config: SharedConfig,
        dst_evm_config: SharedConfig,
    ) -> Self {
        Self {
            runtime,
            nonce_counter,
            side,
            src_evm_config,
            dst_evm_config,
        }
    }

    fn state(&self) -> RuntimeState<Erc20BridgeOpImpl> {
        self.runtime.borrow().state().clone()
    }
}

impl BftBridgeEventHandler for Erc20EventsHandler {
    fn on_wrapped_token_minted(&self, event: MintedEventData) -> BftResult<()> {
        log::trace!("wrapped token minted. Updating operation to the complete state...");

        let nonce = event.nonce;
        let dst_address = event.recipient.clone();
        let operation = Erc20BridgeOpImpl(Erc20BridgeOp {
            side: self.side,
            stage: Erc20OpStage::TokenMintConfirmed(event),
        });

        self.state()
            .borrow_mut()
            .operations
            .update_by_nonce(&dst_address, nonce, operation);

        Ok(())
    }

    fn on_wrapped_token_burnt(&self, event: BurntEventData) -> BftResult<()> {
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
            counter.set(nonce + 1);
            nonce
        };

        let order =
            mint_order_from_burnt_event(event.clone(), src_evm_params, dst_evm_params, nonce)
                .ok_or_else(|| {
                    Error::FailedToProgress(format!(
                        "failed to create a mint order for event: {event:?}"
                    ))
                })?;

        let operation = Erc20BridgeOpImpl(Erc20BridgeOp {
            side: self.side.other(),
            stage: Erc20OpStage::SignMintOrder(order),
        });
        let memo = event.memo();

        let op_id = OperationId::new(nonce as _);
        self.state()
            .borrow_mut()
            .operations
            .new_operation_with_id(op_id, operation.clone(), memo);
        self.runtime.borrow().schedule_operation(op_id, operation);

        Ok(())
    }

    fn on_minter_notification(&self, event: NotifyMinterEventData) -> BftResult<()> {
        log::debug!("on_minter_notification {event:?}");

        if let Some(operation_id) = event.try_decode_reschedule_operation_id() {
            self.runtime.borrow().reschedule_operation(operation_id);
        }

        Ok(())
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
        .ok()?
        .1;
    let dst_token = Id256::from_slice(&event.to_token)?.to_evm_address().ok()?.1;

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
