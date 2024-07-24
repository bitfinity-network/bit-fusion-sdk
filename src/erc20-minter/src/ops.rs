use bridge_canister::bridge::{Operation, OperationAction, OperationContext};
use bridge_did::error::{BftResult, Error};
use bridge_did::id256::Id256;
use bridge_did::op_id::OperationId;
use bridge_did::order::{self, MintOrder, SignedMintOrder};
use bridge_did::reason::Erc2Burn;
use bridge_utils::bft_events::{BurntEventData, MintedEventData, NotifyMinterEventData};
use bridge_utils::evm_link::address_to_Erc_subaccount;
use candid::{CandidType, Decode, Nat};
use did::{H160, H256, U256};
use ic_exports::ic_kit::RejectionCode;
use ic_task_scheduler::retry::BackoffPolicy;
use ic_task_scheduler::task::TaskOptions;
use serde::{Deserialize, Serialize};
use Erc_client::account::Account;
use Erc_client::transfer::TransferError;

use crate::constant::IC_CHAIN_ID;
use crate::tokens::Erc1::{self, ErcCanisterError};
use crate::tokens::Erc2::{self, Success};

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum ErcBridgeOp {
    // Deposit operations:
    BurnErcTokens(ErcBurn),
    SignMintOrder {
        order: MintOrder,
        is_refund: bool,
    },
    SendMintTransaction {
        src_token: Id256,
        dst_address: H160,
        order: SignedMintOrder,
        is_refund: bool,
    },
    ConfirmMint {
        src_token: Id256,
        dst_address: H160,
        order: SignedMintOrder,
        tx_hash: Option<H256>,
        is_refund: bool,
    },
    WrappedTokenMintConfirmed(MintedEventData),

    // Withdraw operations:
    MintErcTokens(BurntEventData),
    ErcMintConfirmed {
        src_address: H160,
        Erc_tx_id: Nat,
    },
}

impl ErcBridgeOp {
    pub fn get_signed_mint_order(&self, token: &Id256) -> Option<SignedMintOrder> {
        match self {
            Self::SendMintTransaction {
                order, src_token, ..
            } if src_token == token => Some(*order),
            Self::ConfirmMint {
                order, src_token, ..
            } if src_token == token => Some(*order),
            _ => None,
        }
    }
}

impl Operation for ErcBridgeOp {
    async fn progress(self, id: OperationId, ctx: impl OperationContext) -> BftResult<Self> {
        match self {
            ErcBridgeOp::BurnErcTokens(burn_info) => {
                Self::burn_Erc_tokens(ctx, burn_info, id.nonce()).await
            }
            ErcBridgeOp::SignMintOrder { order, is_refund } => {
                Self::sign_mint_order(ctx, order, is_refund).await
            }
            ErcBridgeOp::SendMintTransaction {
                order,
                src_token,
                dst_address,
                is_refund,
            } => Self::send_mint_tx(ctx, order, src_token, dst_address, is_refund).await,
            ErcBridgeOp::ConfirmMint { .. } => Err(Error::FailedToProgress(
                "ConfirmMint task should progress only on the Minted EVM event".into(),
            )),
            ErcBridgeOp::WrappedTokenMintConfirmed(_) => Err(Error::FailedToProgress(
                "WrappedTokenMintConfirmed task should not progress".into(),
            )),
            ErcBridgeOp::MintErcTokens(event) => {
                Self::mint_Erc_tokens(ctx, event, id.nonce()).await
            }
            ErcBridgeOp::ErcMintConfirmed { .. } => Err(Error::FailedToProgress(
                "ErcMintConfirmed task should not progress".into(),
            )),
        }
    }

    fn is_complete(&self) -> bool {
        match self {
            ErcBridgeOp::BurnErcTokens(_) => false,
            ErcBridgeOp::SignMintOrder { .. } => false,
            ErcBridgeOp::SendMintTransaction { .. } => false,
            ErcBridgeOp::ConfirmMint { .. } => false,
            ErcBridgeOp::WrappedTokenMintConfirmed(_) => true,
            ErcBridgeOp::MintErcTokens(_) => false,
            ErcBridgeOp::ErcMintConfirmed { .. } => true,
        }
    }

    fn evm_wallet_address(&self) -> H160 {
        match self {
            ErcBridgeOp::BurnErcTokens(burn) => &burn.recipient_address,
            ErcBridgeOp::SignMintOrder { order, .. } => &order.recipient,
            ErcBridgeOp::SendMintTransaction { dst_address, .. } => dst_address,
            ErcBridgeOp::ConfirmMint { dst_address, .. } => dst_address,
            ErcBridgeOp::WrappedTokenMintConfirmed(event) => &event.recipient,
            ErcBridgeOp::MintErcTokens(event) => &event.sender,
            ErcBridgeOp::ErcMintConfirmed { src_address, .. } => src_address,
        }
        .clone()
    }

    fn scheduling_options(&self) -> Option<TaskOptions> {
        match self {
            ErcBridgeOp::ConfirmMint { .. } => None,
            ErcBridgeOp::WrappedTokenMintConfirmed(_) => None,
            ErcBridgeOp::ErcMintConfirmed { .. } => None,
            _ => Some(
                TaskOptions::new()
                    .with_max_retries_policy(3)
                    .with_backoff_policy(BackoffPolicy::Exponential {
                        secs: 2,
                        multiplier: 4,
                    }),
            ),
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
            update_to: ErcBridgeOp::WrappedTokenMintConfirmed(event),
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

        Some(OperationAction::Create(ErcBridgeOp::BurnErcTokens(
            Erc_burn,
        )))
    }
}

impl ErcBridgeOp {
    async fn burn_Erc_tokens(
        ctx: impl OperationContext,
        burn_info: ErcBurn,
        nonce: u32,
    ) -> BftResult<ErcBridgeOp> {
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
    ) -> BftResult<ErcBridgeOp> {
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
    ) -> BftResult<ErcBridgeOp> {
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
    ) -> BftResult<ErcBridgeOp> {
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
