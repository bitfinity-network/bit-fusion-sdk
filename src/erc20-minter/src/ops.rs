use bridge_canister::bridge::{Operation, OperationAction, OperationContext};
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::{BftResult, Error};
use bridge_did::id256::Id256;
use bridge_did::op_id::OperationId;
use bridge_did::order::{self, MintOrder, SignedMintOrder};
use bridge_utils::bft_events::{BurntEventData, MintedEventData, NotifyMinterEventData};
use candid::{CandidType, Decode, Nat};
use did::{H160, H256};
use ic_task_scheduler::retry::BackoffPolicy;
use ic_task_scheduler::task::TaskOptions;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum Erc20BridgeOp {
    // Deposit operations:
    SignWrappedMintOrder {
        order: MintOrder,
    },
    SendWrappedMintTransaction {
        order: SignedMintOrder,
    },
    ConfirmWrappedMint {
        order: SignedMintOrder,
        tx_hash: Option<H256>,
    },
    WrappedTokenMintConfirmed(MintedEventData),

    // Withdraw operations:
    SignBaseMintOrder {
        order: MintOrder,
    },
    SendBaseMintTransaction {
        order: SignedMintOrder,
    },
    ConfirmBaseMint {
        order: SignedMintOrder,
        tx_hash: Option<H256>,
    },
    BaseTokenMintConfirmed(MintedEventData),
}

impl Operation for Erc20BridgeOp {
    async fn progress(self, id: OperationId, ctx: RuntimeState<Self>) -> BftResult<Self> {
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

    fn is_complete(&self) -> bool {
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

    fn evm_wallet_address(&self) -> H160 {
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
    async fn burn_Erc_tokens(
        ctx: impl OperationContext,
        burn_info: ErcBurn,
        nonce: u32,
    ) -> BftResult<Erc20BridgeOp> {
        log::trace!("burning Erc tokens due to: {burn_info:?}");

        let evm_params = ctx.get_evm_params()?;

        let caller_account = Account {
            owner: burn_info.sender,
            subaccount: burn_info.from_subaccount,
        };

        let token_info = Erc1::query_token_info_or_read_from_cache(burn_info.erc_token_principal)
            .await
            .ok_or(Error::Custom {
                code: ErrorCodes::ErcMetadataRequestFailed as _,
                msg: "failed to query Erc token metadata".into(),
            })?;

        log::trace!("got token info: {token_info:?}");

        let name = order::fit_str_to_array(&token_info.name);
        let symbol = order::fit_str_to_array(&token_info.symbol);

        let spender_subaccount = address_to_Erc_subaccount(&burn_info.recipient_address.0);
        Erc::burn(
            burn_info.erc_token_principal,
            caller_account,
            Some(spender_subaccount),
            (&burn_info.amount).into(),
            true,
        )
        .await
        .map_err(|e| Error::Custom {
            code: ErrorCodes::ErcBurnFailed as _,
            msg: format!("failed to burn Erc token: {e}"),
        })?;

        log::trace!("transferred Erc tokens to the bridge account");

        let sender_chain_id = IC_CHAIN_ID;
        let recipient_chain_id = evm_params.chain_id;

        let sender = Id256::from(&burn_info.sender);
        let src_token = Id256::from(&burn_info.erc_token_principal);

        let fee_payer = burn_info.fee_payer.unwrap_or_default();

        let (approve_spender, approve_amount) = burn_info
            .approve_after_mint
            .map(|approve| (approve.approve_spender, approve.approve_amount))
            .unwrap_or_default();

        let order = MintOrder {
            amount: burn_info.amount,
            sender,
            src_token,
            recipient: burn_info.recipient_address,
            dst_token: burn_info.erc20_token_address,
            nonce,
            sender_chain_id,
            recipient_chain_id,
            name,
            symbol,
            decimals: token_info.decimals,
            approve_spender,
            approve_amount,
            fee_payer,
        };

        log::debug!("prepared mint order: {:?}", order);

        Ok(Self::SignMintOrder {
            order,
            is_refund: false,
        })
    }

    async fn sign_mint_order(
        ctx: impl OperationContext,
        order: MintOrder,
        is_refund: bool,
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
