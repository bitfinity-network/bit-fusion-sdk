use bridge_canister::bridge::{Operation, OperationAction, OperationContext};
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::{BftResult, Error};
use bridge_did::id256::Id256;
use bridge_did::op_id::OperationId;
use bridge_did::order::{self, MintOrder, SignedMintOrder};
use bridge_utils::bft_events::{BurntEventData, MintedEventData, NotifyMinterEventData};
use bridge_utils::evm_bridge::BridgeSide;
use candid::{CandidType, Decode, Nat};
use did::{H160, H256};
use ic_task_scheduler::retry::BackoffPolicy;
use ic_task_scheduler::task::TaskOptions;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub struct Erc20BridgeOp {
    side: BridgeSide,
    stage: Erc20OpStage,
}

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
    async fn progress(self, id: OperationId, ctx: RuntimeState<Self>) -> BftResult<Self> {
        let next_stage = match self.side {
            BridgeSide::Base => self.stage.progress(self.base_context()).await?,
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
        match (self.side, self.stage) {
            (BridgeSide::Base, Erc20OpStage::SignMintOrder(order)) => {
                order.sender.to_evm_address().expect("evm address").1
            }
            (BridgeSide::Base, Erc20OpStage::SendMintTransaction(order)) => {
                order.sender().to_evm_address().expect("evm address").1
            }
            (BridgeSide::Base, Erc20OpStage::ConfirmMint { order, tx_hash }) => todo!(),
            (BridgeSide::Base, Erc20OpStage::TokenMintConfirmed(_)) => todo!(),
            (BridgeSide::Wrapped, Erc20OpStage::SignMintOrder(_)) => todo!(),
            (BridgeSide::Wrapped, Erc20OpStage::SendMintTransaction(_)) => todo!(),
            (BridgeSide::Wrapped, Erc20OpStage::ConfirmMint { order, tx_hash }) => todo!(),
            (BridgeSide::Wrapped, Erc20OpStage::TokenMintConfirmed(_)) => todo!(),
        }
    }

    fn scheduling_options(&self) -> Option<TaskOptions> {
        match self {
            Erc20BridgeOp::SignWrappedMintOrder { order } => todo!(),
            Erc20BridgeOp::SendWrappedMintTransaction { order } => todo!(),
            Erc20BridgeOp::ConfirmWrappedMint { order, tx_hash } => todo!(),
            Erc20BridgeOp::WrappedTokenMintConfirmed(_) => todo!(),
            Erc20BridgeOp::SignBaseMintOrder { order } => todo!(),
            Erc20BridgeOp::SendBaseMintTransaction { order } => todo!(),
            Erc20BridgeOp::ConfirmBaseMint { order, tx_hash } => todo!(),
            Erc20BridgeOp::BaseTokenMintConfirmed(_) => todo!(),
        }
    }

    async fn on_wrapped_token_burnt(
        _ctx: impl OperationContext,
        event: BurntEventData,
    ) -> Option<OperationAction<Self>> {
        log::trace!("wrapped token burnt");
        Some(OperationAction::Create(Self::MintErcTokens(event)))
    }

    async fn on_wrapped_token_minted(
        _ctx: impl OperationContext,
        event: MintedEventData,
    ) -> Option<OperationAction<Self>> {
        log::trace!("wrapped token minted");
        Some(OperationAction::Update {
            nonce: event.nonce,
            update_to: Erc20BridgeOp::WrappedTokenMintConfirmed(event),
        })
    }

    async fn on_minter_notification(
        _ctx: impl OperationContext,
        event: NotifyMinterEventData,
    ) -> Option<OperationAction<Self>> {
        log::trace!(
            "got minter notification with type: {}",
            event.notification_type
        );
        let mut Erc_burn = match Decode!(&event.user_data, ErcBurn) {
            Ok(Erc_burn) => Erc_burn,
            Err(e) => {
                log::warn!("failed to decode BftBridge notification into ErcBurn: {e}");
                return None;
            }
        };

        // Approve tokens only if the burner owns recipient wallet.
        if event.tx_sender != Erc_burn.recipient_address {
            Erc_burn.approve_after_mint = None;
        }

        Some(OperationAction::Create(Erc20BridgeOp::BurnErcTokens(
            Erc_burn,
        )))
    }
}

impl Erc20BridgeOp {
    async fn sign_wrapped_mint_order(
        ctx: impl OperationContext,
        order: MintOrder,
    ) -> BftResult<Erc20BridgeOp> {
        let signer = ctx.get_signer()?;
        let signed_mint_order = order.encode_and_sign(&signer).await?;

        let should_send_by_canister = order.fee_payer != H160::zero();
        let next_op = if should_send_by_canister {
            Self::SendMintTransaction {
                src_token: order.src_token,
                dst_address: order.recipient,
                order: signed_mint_order,
                is_refund,
            }
        } else {
            Self::ConfirmMint {
                src_token: order.src_token,
                dst_address: order.recipient,
                order: signed_mint_order,
                tx_hash: None,
                is_refund,
            }
        };

        Ok(next_op)
    }

    async fn send_mint_tx(
        ctx: impl OperationContext,
        order: SignedMintOrder,
        src_token: Id256,
        dst_address: H160,
        is_refund: bool,
    ) -> BftResult<Erc20BridgeOp> {
        let tx_hash = ctx.send_mint_transaction(&order).await?;

        Ok(Self::ConfirmMint {
            src_token,
            dst_address,
            order,
            tx_hash: Some(tx_hash),
            is_refund,
        })
    }

    async fn mint_Erc_tokens(
        ctx: impl OperationContext,
        event: BurntEventData,
        nonce: u32,
    ) -> BftResult<Erc20BridgeOp> {
        log::trace!("Minting Erc20 tokens");

        let evm_params = ctx.get_evm_params()?;

        let Some(to_token) = Id256::from_slice(&event.to_token).and_then(|id| id.try_into().ok())
        else {
            log::warn!("Failed to decode token id256 from erc20 minted event");
            return Err(Error::Serialization(
                "failed to decode token id256 from erc20 minted event".into(),
            ));
        };

        let Some(recipient) =
            Id256::from_slice(&event.recipient_id).and_then(|id| id.try_into().ok())
        else {
            log::warn!("Failed to decode recipient id from minted event");
            return Err(Error::Serialization(
                "Failed to decode recipient id from minted event".into(),
            ));
        };

        // Transfer Erc20 tokens to the recipient.
        let amount = Nat::from(&event.amount);

        let mint_result = Erc::mint(to_token, recipient, amount.clone(), true).await;
    }
}

pub enum ErrorCodes {
    ErcMetadataRequestFailed = 0,
    ErcBurnFailed = 1,
    ErcMintFailed = 2,
}
