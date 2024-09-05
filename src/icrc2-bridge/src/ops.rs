use bridge_canister::bridge::{Operation, OperationAction, OperationContext};
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::{BftResult, Error};
use bridge_did::id256::Id256;
use bridge_did::op_id::OperationId;
use bridge_did::order::{self, EncodedMintOrder, MintOrder};
use bridge_did::reason::Icrc2Burn;
use bridge_utils::bft_events::{BurntEventData, MintedEventData, NotifyMinterEventData};
use bridge_utils::evm_link::address_to_icrc_subaccount;
use candid::{CandidType, Decode, Nat};
use did::{H160, H256, U256};
use ic_exports::ic_kit::RejectionCode;
use ic_task_scheduler::retry::BackoffPolicy;
use ic_task_scheduler::task::TaskOptions;
use icrc_client::account::Account;
use icrc_client::transfer::TransferError;
use serde::{Deserialize, Serialize};

use crate::constant::IC_CHAIN_ID;
use crate::tokens::icrc1::{self, IcrcCanisterError};
use crate::tokens::icrc2::{self, Success};

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum IcrcBridgeOp {
    // Deposit operations:
    BurnIcrc2Tokens(Icrc2Burn),
    SignMintOrder {
        order: MintOrder,
        is_refund: bool,
    },
    SendMintTransaction {
        order: EncodedMintOrder,
        is_refund: bool,
    },
    ConfirmMint {
        order: EncodedMintOrder,
        tx_hash: Option<H256>,
        is_refund: bool,
    },
    WrappedTokenMintConfirmed(MintedEventData),

    // Withdraw operations:
    MintIcrcTokens(BurntEventData),
    IcrcMintConfirmed {
        src_address: H160,
        icrc_tx_id: Nat,
    },
}

impl IcrcBridgeOp {
    pub fn get_signed_mint_order(&self, token: &Id256) -> Option<EncodedMintOrder> {
        match self {
            Self::SendMintTransaction { order, .. } if &order.get_src_token_id() == token => {
                Some(*order)
            }
            Self::ConfirmMint { order, .. } if &order.get_src_token_id() == token => Some(*order),
            _ => None,
        }
    }
}

impl Operation for IcrcBridgeOp {
    async fn progress(self, id: OperationId, ctx: RuntimeState<Self>) -> BftResult<Self> {
        let result = match self {
            IcrcBridgeOp::BurnIcrc2Tokens(burn_info) => {
                Self::burn_icrc_tokens(ctx, burn_info, id.nonce()).await
            }
            IcrcBridgeOp::SignMintOrder { order, is_refund } => {
                Self::sign_mint_order(ctx, order, is_refund).await
            }
            IcrcBridgeOp::SendMintTransaction { order, is_refund } => {
                Self::send_mint_tx(ctx, order, is_refund).await
            }
            IcrcBridgeOp::ConfirmMint { .. } => Err(Error::FailedToProgress(
                "ConfirmMint task should progress only on the Minted EVM event".into(),
            )),
            IcrcBridgeOp::WrappedTokenMintConfirmed(_) => Err(Error::FailedToProgress(
                "WrappedTokenMintConfirmed task should not progress".into(),
            )),
            IcrcBridgeOp::MintIcrcTokens(event) => {
                Self::mint_icrc_tokens(ctx, event, id.nonce()).await
            }
            IcrcBridgeOp::IcrcMintConfirmed { .. } => Err(Error::FailedToProgress(
                "IcrcMintConfirmed task should not progress".into(),
            )),
        };
        log::debug!("icrc task execution result: {result:?}");
        result
    }

    fn is_complete(&self) -> bool {
        match self {
            IcrcBridgeOp::BurnIcrc2Tokens(_) => false,
            IcrcBridgeOp::SignMintOrder { .. } => false,
            IcrcBridgeOp::SendMintTransaction { .. } => false,
            IcrcBridgeOp::ConfirmMint { .. } => false,
            IcrcBridgeOp::WrappedTokenMintConfirmed(_) => true,
            IcrcBridgeOp::MintIcrcTokens(_) => false,
            IcrcBridgeOp::IcrcMintConfirmed { .. } => true,
        }
    }

    fn evm_wallet_address(&self) -> H160 {
        match self {
            IcrcBridgeOp::BurnIcrc2Tokens(burn) => burn.recipient_address.clone(),
            IcrcBridgeOp::SignMintOrder { order, .. } => order.recipient.clone(),
            IcrcBridgeOp::SendMintTransaction { order, .. } => order.get_recipient(),
            IcrcBridgeOp::ConfirmMint { order, .. } => order.get_recipient(),
            IcrcBridgeOp::WrappedTokenMintConfirmed(event) => event.recipient.clone(),
            IcrcBridgeOp::MintIcrcTokens(event) => event.sender.clone(),
            IcrcBridgeOp::IcrcMintConfirmed { src_address, .. } => src_address.clone(),
        }
    }

    fn scheduling_options(&self) -> Option<TaskOptions> {
        match self {
            IcrcBridgeOp::ConfirmMint { .. } => None,
            IcrcBridgeOp::WrappedTokenMintConfirmed(_) => None,
            IcrcBridgeOp::IcrcMintConfirmed { .. } => None,
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
        _ctx: RuntimeState<Self>,
        event: BurntEventData,
    ) -> Option<OperationAction<Self>> {
        log::trace!("wrapped token burnt");
        let memo = event.memo();
        Some(OperationAction::Create(Self::MintIcrcTokens(event), memo))
    }

    async fn on_wrapped_token_minted(
        _ctx: RuntimeState<Self>,
        event: MintedEventData,
    ) -> Option<OperationAction<Self>> {
        log::trace!("wrapped token minted");
        Some(OperationAction::Update {
            nonce: event.nonce,
            update_to: IcrcBridgeOp::WrappedTokenMintConfirmed(event),
        })
    }

    async fn on_minter_notification(
        _ctx: RuntimeState<Self>,
        event: NotifyMinterEventData,
    ) -> Option<OperationAction<Self>> {
        log::trace!(
            "got minter notification with type: {}",
            event.notification_type
        );
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

        Some(OperationAction::Create(
            IcrcBridgeOp::BurnIcrc2Tokens(icrc_burn),
            memo,
        ))
    }
}

impl IcrcBridgeOp {
    async fn burn_icrc_tokens(
        ctx: impl OperationContext,
        burn_info: Icrc2Burn,
        nonce: u32,
    ) -> BftResult<IcrcBridgeOp> {
        log::trace!("burning icrc tokens due to: {burn_info:?}");

        let evm_params = ctx.get_evm_params()?;

        let caller_account = Account {
            owner: burn_info.sender,
            subaccount: burn_info.from_subaccount,
        };

        let token_info =
            icrc1::query_token_info_or_read_from_cache(burn_info.icrc2_token_principal)
                .await
                .ok_or(Error::Custom {
                    code: ErrorCodes::IcrcMetadataRequestFailed as _,
                    msg: "failed to query Icrc token metadata".into(),
                })?;

        log::trace!("got token info: {token_info:?}");

        let name = order::fit_str_to_array(&token_info.name);
        let symbol = order::fit_str_to_array(&token_info.symbol);

        let spender_subaccount = address_to_icrc_subaccount(&burn_info.recipient_address.0);
        icrc2::burn(
            burn_info.icrc2_token_principal,
            caller_account,
            Some(spender_subaccount),
            (&burn_info.amount).into(),
            true,
        )
        .await
        .map_err(|e| Error::Custom {
            code: ErrorCodes::IcrcBurnFailed as _,
            msg: format!("failed to burn ICRC token: {e}"),
        })?;

        log::trace!("transferred icrc tokens to the bridge account");

        let sender_chain_id = IC_CHAIN_ID;
        let recipient_chain_id = evm_params.chain_id;

        let sender = Id256::from(&burn_info.sender);
        let src_token = Id256::from(&burn_info.icrc2_token_principal);

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
    ) -> BftResult<IcrcBridgeOp> {
        let signer = ctx.get_signer()?;
        let signed_mint_order = order.encode_and_sign(&signer).await?;

        let should_send_by_canister = order.fee_payer != H160::zero();
        let next_op = if should_send_by_canister {
            Self::SendMintTransaction {
                order: signed_mint_order,
                is_refund,
            }
        } else {
            Self::ConfirmMint {
                order: signed_mint_order,
                tx_hash: None,
                is_refund,
            }
        };

        Ok(next_op)
    }

    async fn send_mint_tx(
        ctx: impl OperationContext,
        order: EncodedMintOrder,
        is_refund: bool,
    ) -> BftResult<IcrcBridgeOp> {
        let tx_hash = ctx.send_mint_transaction(&order).await?;

        Ok(Self::ConfirmMint {
            order,
            tx_hash: Some(tx_hash),
            is_refund,
        })
    }

    async fn mint_icrc_tokens(
        ctx: impl OperationContext,
        event: BurntEventData,
        nonce: u32,
    ) -> BftResult<IcrcBridgeOp> {
        log::trace!("Minting Icrc2 tokens");

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

        // Transfer icrc2 tokens to the recipient.
        let amount = Nat::from(&event.amount);

        let mint_result = icrc2::mint(to_token, recipient, amount.clone(), true).await;

        match mint_result {
            Ok(Success { tx_id, .. }) => {
                log::trace!("Finished icrc2 mint to principal: {}", recipient);
                Ok(Self::IcrcMintConfirmed {
                    src_address: event.sender,
                    icrc_tx_id: tx_id,
                })
            }
            Err(
                e @ IcrcCanisterError::TransferFailed(TransferError::TooOld)
                | e @ IcrcCanisterError::TransferFailed(TransferError::CreatedInFuture { .. })
                | e @ IcrcCanisterError::TransferFailed(TransferError::TemporarilyUnavailable)
                | e @ IcrcCanisterError::TransferFailed(TransferError::GenericError { .. })
                | e @ IcrcCanisterError::CanisterError(RejectionCode::SysTransient, _),
            ) => {
                log::warn!("Failed to perform icrc token mint due to: {e}. Retrying...");
                Err(Error::Custom {
                    code: ErrorCodes::IcrcMintFailed as _,
                    msg: format!("ICRC token mint failed: {e}"),
                })
            }
            Err(e) => {
                log::warn!(
                    "Impossible to mint icrc token due to: {e}. Preparing refund MintOrder..."
                );

                let sender_chain_id = IC_CHAIN_ID;
                let recipient_chain_id = evm_params.chain_id;

                // If we pass zero name or symbol, it will not be applied.
                let name = event.name.try_into().unwrap_or_default();
                let symbol = event.symbol.try_into().unwrap_or_default();

                let sender = Id256::from(&recipient);
                let src_token = Id256::from(&to_token);

                let order = MintOrder {
                    amount: event.amount,
                    sender,
                    src_token,
                    recipient: event.sender,
                    dst_token: event.from_erc20,
                    nonce,
                    sender_chain_id,
                    recipient_chain_id,
                    name,
                    symbol,
                    decimals: event.decimals,
                    approve_spender: H160::default(),
                    approve_amount: U256::zero(),
                    fee_payer: H160::default(),
                };

                log::debug!("prepared refund mint order: {:?}", order);

                Ok(Self::SignMintOrder {
                    order,
                    is_refund: true,
                })
            }
        }
    }
}

pub enum ErrorCodes {
    IcrcMetadataRequestFailed = 0,
    IcrcBurnFailed = 1,
    IcrcMintFailed = 2,
}
