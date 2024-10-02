use bridge_canister::bridge::{Operation, OperationAction, OperationContext, OperationProgress};
use bridge_canister::memory::StableMemory;
use bridge_canister::runtime::scheduler::{BridgeTask, SharedScheduler};
use bridge_canister::runtime::service::mint_tx::MintTxHandler;
use bridge_canister::runtime::service::sing_orders::MintOrderHandler;
use bridge_canister::runtime::service::{BridgeService, ServiceId};
use bridge_canister::runtime::state::SharedConfig;
use bridge_canister::runtime::RuntimeState;
use bridge_did::bridge_side::BridgeSide;
use bridge_did::error::{BftResult, Error};
use bridge_did::event_data::*;
use bridge_did::id256::Id256;
use bridge_did::op_id::OperationId;
use bridge_did::operations::{Erc20BridgeOp, Erc20OpStage};
use bridge_did::order::{MintOrder, SignedOrder};
use bridge_utils::evm_bridge::EvmParams;
use candid::CandidType;
use did::{H160, U256};
use eth_signer::sign_strategy::TransactionSigner;
use ic_task_scheduler::scheduler::TaskScheduler;
use ic_task_scheduler::task::{ScheduledTask, TaskOptions};
use serde::{Deserialize, Serialize};

use crate::canister::{get_base_evm_config, get_base_evm_state, get_runtime_state};

pub const SIGN_MINT_ORDER_SERVICE_ID: ServiceId = 0;
pub const SEND_MINT_TX_SERVICE_ID: ServiceId = 1;

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub struct Erc20BridgeOpImpl(pub Erc20BridgeOp);

impl Operation for Erc20BridgeOpImpl {
    async fn progress(
        self,
        _id: OperationId,
        _ctx: RuntimeState<Self>,
    ) -> BftResult<OperationProgress<Self>> {
        let stage = Erc20OpStageImpl(self.0.stage);
        let next_stage = match self.0.side {
            BridgeSide::Base => stage.progress().await?,
            BridgeSide::Wrapped => stage.progress().await?,
        };

        let progress = match next_stage {
            OperationProgress::Progress(stage) => {
                OperationProgress::Progress(Self(Erc20BridgeOp {
                    side: self.0.side,
                    stage: stage.0,
                }))
            }
            OperationProgress::AddToService(op_id) => OperationProgress::AddToService(op_id),
        };
        Ok(progress)
    }

    fn is_complete(&self) -> bool {
        match self.0.stage {
            Erc20OpStage::SignMintOrder(_) => false,
            Erc20OpStage::SendMintTransaction(_) => false,
            Erc20OpStage::ConfirmMint { .. } => false,
            Erc20OpStage::TokenMintConfirmed(_) => true,
        }
    }

    fn evm_wallet_address(&self) -> H160 {
        match (self.0.side, &self.0.stage) {
            // If withdrawal, then use sender address.
            (BridgeSide::Base, Erc20OpStage::SignMintOrder(order)) => {
                order.sender.to_evm_address().expect("evm address").1
            }
            (BridgeSide::Base, Erc20OpStage::SendMintTransaction(order)) => {
                order
                    .reader()
                    .get_sender_id()
                    .to_evm_address()
                    .expect("evm address")
                    .1
            }
            (BridgeSide::Base, Erc20OpStage::ConfirmMint { order, .. }) => {
                order
                    .reader()
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
                order.reader().get_recipient()
            }
            (BridgeSide::Wrapped, Erc20OpStage::ConfirmMint { order, .. }) => {
                order.reader().get_recipient()
            }
            (BridgeSide::Wrapped, Erc20OpStage::TokenMintConfirmed(event)) => {
                event.recipient.clone()
            }
        }
    }

    fn scheduling_options(&self) -> Option<TaskOptions> {
        match self.0.stage {
            Erc20OpStage::SignMintOrder(_) => Some(TaskOptions::default()),
            Erc20OpStage::SendMintTransaction(_) => Some(TaskOptions::default()),
            Erc20OpStage::ConfirmMint { .. } => None,
            Erc20OpStage::TokenMintConfirmed(_) => None,
        }
    }

    async fn on_wrapped_token_burnt(
        ctx: RuntimeState<Self>,
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

        let operation = Self(Erc20BridgeOp {
            side: BridgeSide::Base,
            stage: Erc20OpStage::SignMintOrder(order),
        });
        let memo = event.memo();

        let action = OperationAction::CreateWithId(OperationId::new(nonce as _), operation, memo);
        Some(action)
    }

    async fn on_wrapped_token_minted(
        _ctx: RuntimeState<Self>,
        event: MintedEventData,
    ) -> Option<OperationAction<Self>> {
        log::trace!("wrapped token minted. Updating operation to the complete state...");

        let nonce = event.nonce;
        let operation = Self(Erc20BridgeOp {
            side: BridgeSide::Wrapped,
            stage: Erc20OpStage::TokenMintConfirmed(event),
        });
        let action = OperationAction::Update {
            nonce,
            update_to: operation,
        };
        Some(action)
    }

    async fn on_minter_notification(
        _ctx: RuntimeState<Self>,
        _event: NotifyMinterEventData,
    ) -> Option<OperationAction<Self>> {
        log::info!("got unexpected mint notification event");
        None
    }
}

pub struct Erc20OpStageImpl(pub Erc20OpStage);

impl Erc20OpStageImpl {
    /// Returns signed mint order if the stage contains it.
    pub fn get_signed_mint_order(&self) -> Option<&SignedOrder> {
        match &self.0 {
            Erc20OpStage::SignMintOrder(_) => None,
            Erc20OpStage::SendMintTransaction(order) => Some(order),
            Erc20OpStage::ConfirmMint { order, .. } => Some(order),
            Erc20OpStage::TokenMintConfirmed(_) => None,
        }
    }

    async fn progress(self) -> BftResult<OperationProgress<Self>> {
        match self.0 {
            Erc20OpStage::SignMintOrder(_) => {
                Ok(OperationProgress::AddToService(SIGN_MINT_ORDER_SERVICE_ID))
            }
            Erc20OpStage::SendMintTransaction(_) => {
                Ok(OperationProgress::AddToService(SEND_MINT_TX_SERVICE_ID))
            }
            Erc20OpStage::ConfirmMint { .. } => Err(bridge_did::error::Error::FailedToProgress(
                "Erc20OpStage::ConfirmMint should progress by the event".into(),
            )),
            Erc20OpStage::TokenMintConfirmed(_) => Err(bridge_did::error::Error::FailedToProgress(
                "Erc20OpStage::TokenMintConfirmed should not progress".into(),
            )),
        }
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

/// Select base or wrapped service based on operation side.
pub struct Erc20ServiceSelector<S> {
    base: S,
    wrapped: S,
}

impl<S> Erc20ServiceSelector<S> {
    pub fn new(base: S, wrapped: S) -> Self {
        Self { base, wrapped }
    }
}

#[async_trait::async_trait(?Send)]
impl<S: BridgeService> BridgeService for Erc20ServiceSelector<S> {
    async fn run(&self) -> BftResult<()> {
        let (base_result, wrapped_result) = futures::join!(self.base.run(), self.wrapped.run());
        base_result?;
        wrapped_result?;
        Ok(())
    }

    fn push_operation(&self, id: OperationId) -> BftResult<()> {
        let Some(op) = get_runtime_state().borrow().operations.get(id) else {
            log::warn!("Attempt to add unexisting operataion to mint order sign service");
            return Err(Error::OperationNotFound(id));
        };

        match op.0.side {
            BridgeSide::Base => self.base.push_operation(id),
            BridgeSide::Wrapped => self.wrapped.push_operation(id),
        }
    }
}

#[derive(Clone)]
pub struct Erc20OrderHandler {
    state: RuntimeState<Erc20BridgeOpImpl>,
    config: SharedConfig,
    scheduler: SharedScheduler<StableMemory, Erc20BridgeOpImpl>,
}

impl Erc20OrderHandler {
    pub fn new(
        state: RuntimeState<Erc20BridgeOpImpl>,
        config: SharedConfig,
        scheduler: SharedScheduler<StableMemory, Erc20BridgeOpImpl>,
    ) -> Self {
        Self {
            state,
            config,
            scheduler,
        }
    }
}

impl MintOrderHandler for Erc20OrderHandler {
    fn get_signer(&self) -> BftResult<impl TransactionSigner> {
        self.config.borrow().get_signer()
    }

    fn get_order(&self, id: OperationId) -> Option<MintOrder> {
        let op = self.state.borrow().operations.get(id)?;
        let Erc20OpStage::SignMintOrder(order) = op.0.stage else {
            log::info!("Mint order handler failed to get MintOrder: unexpected state.");
            return None;
        };

        Some(order)
    }

    fn set_signed_order(&self, id: OperationId, signed: SignedOrder) {
        let Some(op) = self.state.borrow().operations.get(id) else {
            log::info!("Mint order handler failed to set MintOrder: operation not found.");
            return;
        };

        let Erc20OpStage::SignMintOrder(order) = op.0.stage else {
            log::info!("Mint order handler failed to set MintOrder: unexpected state.");
            return;
        };

        let should_send_mint_tx = order.fee_payer != H160::zero();
        let new_stage = match should_send_mint_tx {
            true => Erc20OpStage::SendMintTransaction(signed),
            false => Erc20OpStage::ConfirmMint {
                order: signed,
                tx_hash: None,
            },
        };

        let new_op = Erc20BridgeOpImpl(Erc20BridgeOp {
            side: op.0.side,
            stage: new_stage,
        });
        let scheduling_options = new_op.scheduling_options();
        self.state
            .borrow_mut()
            .operations
            .update(id, new_op.clone());

        if let Some(options) = scheduling_options {
            let scheduled_task =
                ScheduledTask::with_options(BridgeTask::Operation(id, new_op), options);
            self.scheduler.append_task(scheduled_task);
        }
    }
}

impl MintTxHandler for Erc20OrderHandler {
    fn get_signer(&self) -> BftResult<impl TransactionSigner> {
        self.config.borrow().get_signer()
    }

    fn get_evm_config(&self) -> SharedConfig {
        self.config.clone()
    }

    fn get_signed_orders(&self, id: OperationId) -> Option<SignedOrder> {
        let Some(op) = self.state.borrow().operations.get(id) else {
            log::info!("Mint order handler failed to get MintOrder: operation not found.");
            return None;
        };
        let Erc20OpStage::SendMintTransaction(order) = op.0.stage else {
            log::info!("MintTxHandler failed to get mint order batch: unexpected operation state.");
            return None;
        };

        Some(order)
    }

    fn mint_tx_sent(&self, id: OperationId, tx_hash: did::H256) {
        let Some(op) = self.state.borrow().operations.get(id) else {
            log::info!("MintTxHandler failed to update operation: not found.");
            return;
        };
        let Erc20OpStage::SendMintTransaction(order) = op.0.stage else {
            log::info!("MintTxHandler failed to update operation: unexpected state.");
            return;
        };

        self.state.borrow_mut().operations.update(
            id,
            Erc20BridgeOpImpl(Erc20BridgeOp {
                side: op.0.side,
                stage: Erc20OpStage::ConfirmMint {
                    order,
                    tx_hash: Some(tx_hash),
                },
            }),
        );
    }
}
