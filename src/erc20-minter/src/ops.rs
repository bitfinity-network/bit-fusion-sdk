use bridge_canister::bridge::{Operation, OperationAction, OperationContext};
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::BftResult;
use bridge_did::id256::Id256;
use bridge_did::op_id::OperationId;
use bridge_did::order::{MintOrder, SignedMintOrder};
use bridge_utils::bft_events::{BurntEventData, MintedEventData, NotifyMinterEventData};
use bridge_utils::evm_bridge::{BridgeSide, EvmParams};
use candid::CandidType;
use did::{H160, H256, U256};
use ic_task_scheduler::task::TaskOptions;
use serde::{Deserialize, Serialize};

use crate::canister::{get_base_evm_config, get_base_evm_state};

/// Erc20 bridge operation.
#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub struct Erc20BridgeOp {
    /// Side of the bridge to perfrom the operation.
    pub side: BridgeSide,

    /// Stage of the operation.
    pub stage: Erc20OpStage,
}

/// Erc20 bridge operation stages.
#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum Erc20OpStage {
    SignMintOrder(MintOrder),
    SendMintTransaction(SignedMintOrder),
    ConfirmMint {
        order: SignedMintOrder,
        tx_hash: Option<H256>,
    },
    TokenMintConfirmed(MintedEventData),
}

impl Operation for Erc20BridgeOp {
    async fn progress(self, _id: OperationId, ctx: RuntimeState<Self>) -> BftResult<Self> {
        let next_stage = match self.side {
            BridgeSide::Base => self.stage.progress(get_base_evm_state()).await?,
            BridgeSide::Wrapped => self.stage.progress(ctx).await?,
        };

        Ok(Self {
            side: self.side,
            stage: next_stage,
        })
    }

    fn is_complete(&self) -> bool {
        match self.stage {
            Erc20OpStage::SignMintOrder(_) => false,
            Erc20OpStage::SendMintTransaction(_) => false,
            Erc20OpStage::ConfirmMint { .. } => false,
            Erc20OpStage::TokenMintConfirmed(_) => true,
        }
    }

    fn evm_wallet_address(&self) -> H160 {
        match (self.side, &self.stage) {
            // If withdrawal, then use sender address.
            (BridgeSide::Base, Erc20OpStage::SignMintOrder(order)) => {
                order.sender.to_evm_address().expect("evm address").1
            }
            (BridgeSide::Base, Erc20OpStage::SendMintTransaction(order)) => {
                order
                    .get_sender_id()
                    .to_evm_address()
                    .expect("evm address")
                    .1
            }
            (BridgeSide::Base, Erc20OpStage::ConfirmMint { order, .. }) => {
                order
                    .get_sender_id()
                    .to_evm_address()
                    .expect("evm address")
                    .1
            }
            (BridgeSide::Base, Erc20OpStage::TokenMintConfirmed(event)) => {
                Id256::from_slice(&event.sender_id)
                    .and_then(|id| id.to_evm_address().ok())
                    .expect("evm address")
                    .1
            }

            // If deposit, use recipient address.
            (BridgeSide::Wrapped, Erc20OpStage::SignMintOrder(order)) => order.recipient.clone(),
            (BridgeSide::Wrapped, Erc20OpStage::SendMintTransaction(order)) => {
                order.get_recipient()
            }
            (BridgeSide::Wrapped, Erc20OpStage::ConfirmMint { order, .. }) => order.get_recipient(),
            (BridgeSide::Wrapped, Erc20OpStage::TokenMintConfirmed(event)) => {
                event.recipient.clone()
            }
        }
    }

    fn scheduling_options(&self) -> Option<TaskOptions> {
        None
    }

    async fn on_wrapped_token_burnt(
        ctx: impl OperationContext,
        event: BurntEventData,
    ) -> Option<OperationAction<Self>> {
        log::trace!("wrapped token burnt. Preparing mint order for other side...");

        // Panic here to make the runtime re-process the events when EVM params will be initialized.
        let wrapped_evm_params = ctx.get_evm_params().expect(
            "on_wrapped_token_burnt should not be called if wrapped evm params are not initialized",
        );
        let base_evm_params = get_base_evm_config().borrow().get_evm_params().expect(
            "on_wrapped_token_burnt should not be called if base evm params are not initialized",
        );

        let nonce = get_base_evm_state().0.borrow_mut().next_nonce();

        let Some(order) =
            mint_order_from_burnt_event(event.clone(), wrapped_evm_params, base_evm_params, nonce)
        else {
            log::warn!("failed to create a mint order for event: {event:?}");
            return None;
        };

        let operation = Self {
            side: BridgeSide::Base,
            stage: Erc20OpStage::SignMintOrder(order),
        };
        let action = OperationAction::CreateWithId(OperationId::new(nonce as _), operation);
        Some(action)
    }

    async fn on_wrapped_token_minted(
        _ctx: impl OperationContext,
        event: MintedEventData,
    ) -> Option<OperationAction<Self>> {
        log::trace!("wrapped token minted. Updating operation to the complete state...");

        let nonce = event.nonce;
        let operation = Self {
            side: BridgeSide::Wrapped,
            stage: Erc20OpStage::TokenMintConfirmed(event),
        };
        let action = OperationAction::Update {
            nonce,
            update_to: operation,
        };
        Some(action)
    }

    async fn on_minter_notification(
        _ctx: impl OperationContext,
        _event: NotifyMinterEventData,
    ) -> Option<OperationAction<Self>> {
        log::info!("got unexpected mint notification event");
        None
    }
}

impl Erc20OpStage {
    /// Returns signed mint order if the stage contains it.
    pub fn get_signed_mint_order(&self) -> Option<&SignedMintOrder> {
        match self {
            Erc20OpStage::SignMintOrder(_) => None,
            Erc20OpStage::SendMintTransaction(order) => Some(order),
            Erc20OpStage::ConfirmMint { order, .. } => Some(order),
            Erc20OpStage::TokenMintConfirmed(_) => None,
        }
    }

    async fn progress(self, ctx: impl OperationContext) -> BftResult<Self> {
        match self {
            Erc20OpStage::SignMintOrder(order) => Self::sign_mint_order(ctx, order).await,
            Erc20OpStage::SendMintTransaction(order) => Self::send_mint_tx(ctx, order).await,
            Erc20OpStage::ConfirmMint { .. } => Err(bridge_did::error::Error::FailedToProgress(
                "Erc20OpStage::ConfirmMint should progress by the event".into(),
            )),
            Erc20OpStage::TokenMintConfirmed(_) => Err(bridge_did::error::Error::FailedToProgress(
                "Erc20OpStage::TokenMintConfirmed should not progress".into(),
            )),
        }
    }

    async fn sign_mint_order(ctx: impl OperationContext, order: MintOrder) -> BftResult<Self> {
        log::trace!("signing mint order: {order:?}");

        let signer = ctx.get_signer()?;
        let signed_mint_order = order.encode_and_sign(&signer).await?;

        let should_send_by_canister = order.fee_payer != H160::zero();
        let next_op = if should_send_by_canister {
            Self::SendMintTransaction(signed_mint_order)
        } else {
            Self::ConfirmMint {
                order: signed_mint_order,
                tx_hash: None,
            }
        };

        Ok(next_op)
    }

    async fn send_mint_tx(ctx: impl OperationContext, order: SignedMintOrder) -> BftResult<Self> {
        log::trace!("sending mint transaction");

        let tx_hash = ctx.send_mint_transaction(&order).await?;

        Ok(Self::ConfirmMint {
            order,
            tx_hash: Some(tx_hash),
        })
    }
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
