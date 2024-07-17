use bridge_canister::bridge::{Operation, OperationAction, OperationContext};
use bridge_did::{
    error::{BftResult, Error},
    id256::Id256,
    op_id::OperationId,
    order::{self, MintOrder, SignedMintOrder},
    reason::Icrc2Burn,
};
use bridge_utils::{
    bft_events::{self, BurntEventData, MintedEventData, NotifyMinterEventData},
    evm_link::address_to_icrc_subaccount,
};
use candid::{CandidType, Decode, Nat};
use did::{H160, H256, U256};
use eth_signer::sign_strategy::TransactionSigner;
use ic_exports::ic_kit::RejectionCode;
use ic_task_scheduler::{retry::BackoffPolicy, task::TaskOptions};
use icrc_client::{account::Account, transfer::TransferError};
use serde::{Deserialize, Serialize};

use crate::{
    constant::IC_CHAIN_ID,
    tokens::{
        icrc1::{self, IcrcCanisterError},
        icrc2::{self, Success},
    },
};

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub enum IcrcBridgeOp {
    // Deposit operations:
    BurnIcrc2Tokens(Icrc2Burn),
    SignMintOrder {
        order: MintOrder,
        is_refund: bool,
    },
    SendMintTransaction {
        dst_address: H160,
        order: SignedMintOrder,
        is_refund: bool,
    },
    ConfirmMint {
        dst_address: H160,
        order: SignedMintOrder,
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

impl Operation for IcrcBridgeOp {
    async fn progress(self, id: OperationId, ctx: impl OperationContext) -> BftResult<Self> {
        match self {
            IcrcBridgeOp::BurnIcrc2Tokens(burn_info) => {
                Self::burn_icrc_tokens(ctx, burn_info, id.nonce()).await
            }
            IcrcBridgeOp::SignMintOrder { order, is_refund } => {
                Self::sign_mint_order(ctx, order, is_refund).await
            }
            IcrcBridgeOp::SendMintTransaction {
                order,
                dst_address,
                is_refund,
            } => Self::send_mint_tx(ctx, order, dst_address, is_refund).await,
            IcrcBridgeOp::ConfirmMint { .. } => {
                log::warn!("ConfirmMint task should progress only on the Minted EVM event");
                Err(Error::FailToProgress(
                    "ConfirmMint task should progress only on the Minted EVM event".into(),
                ))
            }
            IcrcBridgeOp::WrappedTokenMintConfirmed(_) => {
                log::warn!("WrappedTokenMintConfirmed task should not progress");
                Err(Error::FailToProgress(
                    "WrappedTokenMintConfirmed task should not progress".into(),
                ))
            }
            IcrcBridgeOp::MintIcrcTokens(event) => {
                Self::mint_icrc_tokens(ctx, event, id.nonce()).await
            }
            IcrcBridgeOp::IcrcMintConfirmed { .. } => {
                log::warn!("IcrcMintConfirmed task should not progress");
                Err(Error::FailToProgress(
                    "IcrcMintConfirmed task should not progress".into(),
                ))
            }
        }
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

    fn evm_address(&self) -> H160 {
        match self {
            IcrcBridgeOp::BurnIcrc2Tokens(burn) => &burn.recipient_address,
            IcrcBridgeOp::SignMintOrder { order, .. } => &order.recipient,
            IcrcBridgeOp::SendMintTransaction { dst_address, .. } => dst_address,
            IcrcBridgeOp::ConfirmMint { dst_address, .. } => dst_address,
            IcrcBridgeOp::WrappedTokenMintConfirmed(event) => &event.recipient,
            IcrcBridgeOp::MintIcrcTokens(event) => &event.sender,
            IcrcBridgeOp::IcrcMintConfirmed { src_address, .. } => src_address,
        }
        .clone()
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
        _ctx: impl OperationContext,
        event: BurntEventData,
    ) -> Option<OperationAction<Self>> {
        Some(OperationAction::Create(Self::MintIcrcTokens(event)))
    }

    async fn on_wrapped_token_minted(
        _ctx: impl OperationContext,
        event: MintedEventData,
    ) -> Option<OperationAction<Self>> {
        Some(OperationAction::Update {
            nonce: event.nonce,
            update_to: IcrcBridgeOp::WrappedTokenMintConfirmed(event),
        })
    }

    async fn on_minter_notification(
        _ctx: impl OperationContext,
        event: NotifyMinterEventData,
    ) -> Option<OperationAction<Self>> {
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

        Some(OperationAction::Create(IcrcBridgeOp::BurnIcrc2Tokens(
            icrc_burn,
        )))
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
                dst_address: order.recipient,
                order: signed_mint_order,
                is_refund,
            }
        } else {
            Self::ConfirmMint {
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
        dst_address: H160,
        is_refund: bool,
    ) -> BftResult<IcrcBridgeOp> {
        let signer = ctx.get_signer()?;
        let sender = signer.get_address().await?;
        let bridge_contract = ctx.get_bridge_contract_address()?;
        let evm_params = ctx.get_evm_params()?;

        let mut tx = bft_events::mint_transaction(
            sender.0,
            bridge_contract.0,
            evm_params.nonce.into(),
            evm_params.gas_price.clone().into(),
            &order.0,
            evm_params.chain_id as _,
        );

        let signature = signer.sign_transaction(&(&tx).into()).await?;
        tx.r = signature.r.0;
        tx.s = signature.s.0;
        tx.v = signature.v.0;
        tx.hash = tx.hash();

        let client = ctx.get_evm_link().get_json_rpc_client();
        let tx_hash = client
            .send_raw_transaction(tx)
            .await
            .map_err(|e| Error::EvmRequestFailed(format!("failed to send mint tx to EVM: {e}")))?;

        Ok(Self::ConfirmMint {
            dst_address,
            order,
            tx_hash: Some(tx_hash.into()),
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
