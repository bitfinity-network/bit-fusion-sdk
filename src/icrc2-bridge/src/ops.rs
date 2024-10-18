use bridge_canister::bridge::{Operation, OperationContext, OperationProgress};
use bridge_canister::memory::StableMemory;
use bridge_canister::runtime::scheduler::{BridgeTask, SharedScheduler};
use bridge_canister::runtime::service::mint_tx::MintTxHandler;
use bridge_canister::runtime::service::sign_orders::MintOrderHandler;
use bridge_canister::runtime::service::ServiceId;
use bridge_canister::runtime::state::SharedConfig;
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::{BftResult, Error};
use bridge_did::event_data::BurntEventData;
use bridge_did::id256::Id256;
use bridge_did::op_id::OperationId;
use bridge_did::operations::IcrcBridgeOp;
use bridge_did::order::{self, MintOrder, SignedOrders};
use bridge_did::reason::Icrc2Burn;
use bridge_utils::evm_link::address_to_icrc_subaccount;
use candid::{CandidType, Nat};
use did::{H160, H256, U256};
use eth_signer::sign_strategy::TransactionSigner;
use ic_exports::ic_kit::RejectionCode;
use ic_task_scheduler::retry::BackoffPolicy;
use ic_task_scheduler::scheduler::TaskScheduler;
use ic_task_scheduler::task::{ScheduledTask, TaskOptions};
use icrc_client::account::Account;
use icrc_client::transfer::TransferError;
use serde::{Deserialize, Serialize};

use crate::constant::IC_CHAIN_ID;
use crate::tokens::icrc1::{self, IcrcCanisterError};
use crate::tokens::icrc2::{self, Success};

mod events_handler;

pub const SIGN_MINT_ORDER_SERVICE_ID: ServiceId = 0;
pub const SEND_MINT_TX_SERVICE_ID: ServiceId = 1;

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub struct IcrcBridgeOpImpl(pub IcrcBridgeOp);

impl Operation for IcrcBridgeOpImpl {
    async fn progress(
        self,
        id: OperationId,
        ctx: RuntimeState<Self>,
    ) -> BftResult<OperationProgress<Self>> {
        let next_step = match self.0 {
            IcrcBridgeOp::BurnIcrc2Tokens(burn_info) => {
                Self::burn_icrc_tokens(ctx, burn_info, id.nonce()).await
            }
            IcrcBridgeOp::SignMintOrder { .. } => {
                return Ok(OperationProgress::AddToService(SIGN_MINT_ORDER_SERVICE_ID));
            }
            IcrcBridgeOp::SendMintTransaction { .. } => {
                return Ok(OperationProgress::AddToService(SEND_MINT_TX_SERVICE_ID));
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

        Ok(OperationProgress::Progress(Self(next_step?)))
    }

    fn is_complete(&self) -> bool {
        match self.0 {
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
        match &self.0 {
            IcrcBridgeOp::BurnIcrc2Tokens(burn) => burn.recipient_address.clone(),
            IcrcBridgeOp::SignMintOrder { order, .. } => order.recipient.clone(),
            IcrcBridgeOp::SendMintTransaction { order, .. } => order.reader().get_recipient(),
            IcrcBridgeOp::ConfirmMint { order, .. } => order.reader().get_recipient(),
            IcrcBridgeOp::WrappedTokenMintConfirmed(event) => event.recipient.clone(),
            IcrcBridgeOp::MintIcrcTokens(event) => event.sender.clone(),
            IcrcBridgeOp::IcrcMintConfirmed { src_address, .. } => src_address.clone(),
        }
    }

    fn scheduling_options(&self) -> Option<TaskOptions> {
        match self.0 {
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
}

impl IcrcBridgeOpImpl {
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

        Ok(IcrcBridgeOp::SignMintOrder {
            order,
            is_refund: false,
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
                Ok(IcrcBridgeOp::IcrcMintConfirmed {
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

                Ok(IcrcBridgeOp::SignMintOrder {
                    order,
                    is_refund: true,
                })
            }
        }
    }
}

/// ICRC token related errors.
pub enum ErrorCodes {
    IcrcMetadataRequestFailed = 0,
    IcrcBurnFailed = 1,
    IcrcMintFailed = 2,
}

/// Allows Signing service to handle MintOrders of ICRC bridge.
pub struct IcrcMintOrderHandler {
    state: RuntimeState<IcrcBridgeOpImpl>,
    scheduler: SharedScheduler<StableMemory, IcrcBridgeOpImpl>,
}

impl IcrcMintOrderHandler {
    /// Creates new handler instance.
    pub fn new(
        state: RuntimeState<IcrcBridgeOpImpl>,
        scheduler: SharedScheduler<StableMemory, IcrcBridgeOpImpl>,
    ) -> Self {
        Self { state, scheduler }
    }
}

impl MintOrderHandler for IcrcMintOrderHandler {
    fn get_signer(&self) -> BftResult<impl TransactionSigner> {
        self.state.get_signer()
    }

    fn get_order(&self, id: OperationId) -> Option<MintOrder> {
        let op = self.state.borrow().operations.get(id)?;
        let IcrcBridgeOp::SignMintOrder { order, .. } = op.0 else {
            log::info!("Mint order handler failed to get MintOrder: unexpected state.");
            return None;
        };

        Some(order)
    }

    fn set_signed_order(&self, id: OperationId, signed: SignedOrders) {
        let Some(op) = self.state.borrow().operations.get(id) else {
            log::info!("Mint order handler failed to set MintOrder: operation not found.");
            return;
        };

        let IcrcBridgeOp::SignMintOrder { is_refund, order } = op.0 else {
            log::info!("Mint order handler failed to set MintOrder: unexpected state.");
            return;
        };

        let will_pay_fee = order.fee_payer != H160::zero();
        let should_send_mint_tx = !is_refund && will_pay_fee;
        let new_op = match should_send_mint_tx {
            true => IcrcBridgeOp::SendMintTransaction {
                order: signed,
                is_refund,
            },
            false => IcrcBridgeOp::ConfirmMint {
                order: signed,
                is_refund,
                tx_hash: None,
            },
        };

        let new_op = IcrcBridgeOpImpl(new_op);
        let scheduling_options = new_op.scheduling_options();
        self.state
            .borrow_mut()
            .operations
            .update(id, new_op.clone());

        if let Some(options) = scheduling_options {
            let scheduled_task = ScheduledTask::with_options(BridgeTask::new(id, new_op), options);
            self.scheduler.append_task(scheduled_task);
        }
    }
}

/// Allows MintTxService to handle IcrcOperations.
pub struct IcrcMintTxHandler {
    state: RuntimeState<IcrcBridgeOpImpl>,
}

impl IcrcMintTxHandler {
    /// Creates new handler instance.
    pub fn new(state: RuntimeState<IcrcBridgeOpImpl>) -> Self {
        Self { state }
    }
}

impl MintTxHandler for IcrcMintTxHandler {
    fn get_signer(&self) -> BftResult<impl TransactionSigner> {
        self.state.get_signer()
    }

    fn get_evm_config(&self) -> SharedConfig {
        self.state.borrow().config.clone()
    }

    fn get_signed_orders(&self, id: OperationId) -> Option<SignedOrders> {
        let op = self.state.borrow().operations.get(id);
        let Some(IcrcBridgeOp::SendMintTransaction { order, .. }) = op.map(|op| op.0) else {
            log::info!("MintTxHandler failed to get mint order batch: unexpected operation state.");
            return None;
        };

        Some(order)
    }

    fn mint_tx_sent(&self, id: OperationId, tx_hash: H256) {
        let op = self.state.borrow().operations.get(id);
        let Some(IcrcBridgeOp::SendMintTransaction { order, is_refund }) = op.map(|op| op.0) else {
            log::info!("MintTxHandler failed to update operation: unexpected operation state.");
            return;
        };

        self.state.borrow_mut().operations.update(
            id,
            IcrcBridgeOpImpl(IcrcBridgeOp::ConfirmMint {
                order,
                tx_hash: Some(tx_hash),
                is_refund,
            }),
        );
    }
}
