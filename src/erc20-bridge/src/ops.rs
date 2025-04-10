use bridge_canister::bridge::{Operation, OperationProgress};
use bridge_canister::memory::StableMemory;
use bridge_canister::runtime::scheduler::{BridgeTask, SharedScheduler};
use bridge_canister::runtime::service::mint_tx::{MintTxHandler, MintTxResult};
use bridge_canister::runtime::service::sign_orders::MintOrderHandler;
use bridge_canister::runtime::service::{BridgeService, ServiceId};
use bridge_canister::runtime::state::SharedConfig;
use bridge_canister::runtime::RuntimeState;
use bridge_did::bridge_side::BridgeSide;
use bridge_did::error::{BTFResult, Error};
use bridge_did::id256::Id256;
use bridge_did::op_id::OperationId;
use bridge_did::operations::{Erc20BridgeOp, Erc20OpStage};
use bridge_did::order::{MintOrder, SignedOrders};
use candid::CandidType;
use did::H160;
use eth_signer::sign_strategy::TxSigner;
use ic_task_scheduler::scheduler::TaskScheduler;
use ic_task_scheduler::task::{ScheduledTask, TaskOptions};
use serde::{Deserialize, Serialize};

use crate::canister::get_runtime_state;

pub mod events_handler;

pub const REFRESH_BASE_PARAMS_SERVICE_ID: ServiceId = 0;
pub const REFRESH_WRAPPED_PARAMS_SERVICE_ID: ServiceId = 1;
pub const FETCH_BASE_LOGS_SERVICE_ID: ServiceId = 2;
pub const FETCH_WRAPPED_LOGS_SERVICE_ID: ServiceId = 3;
pub const SIGN_MINT_ORDER_SERVICE_ID: ServiceId = 4;
pub const SEND_MINT_TX_SERVICE_ID: ServiceId = 5;

#[derive(Debug, Serialize, Deserialize, CandidType, Clone)]
pub struct Erc20BridgeOpImpl(pub Erc20BridgeOp);

impl Operation for Erc20BridgeOpImpl {
    async fn progress(
        self,
        _id: OperationId,
        _ctx: RuntimeState<Self>,
    ) -> BTFResult<OperationProgress<Self>> {
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
            Erc20OpStage::WaitForMintConfirm { .. } => false,
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
            (BridgeSide::Base, Erc20OpStage::WaitForMintConfirm { order, .. }) => {
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
            (BridgeSide::Wrapped, Erc20OpStage::WaitForMintConfirm { order, .. }) => {
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
            Erc20OpStage::WaitForMintConfirm { .. } => None,
            Erc20OpStage::TokenMintConfirmed(_) => None,
        }
    }
}

pub struct Erc20OpStageImpl(pub Erc20OpStage);

impl Erc20OpStageImpl {
    /// Returns signed mint order if the stage contains it.
    pub fn get_signed_mint_order(&self) -> Option<&SignedOrders> {
        match &self.0 {
            Erc20OpStage::SignMintOrder(_) => None,
            Erc20OpStage::SendMintTransaction(order) => Some(order),
            Erc20OpStage::WaitForMintConfirm { order, .. } => Some(order),
            Erc20OpStage::TokenMintConfirmed(_) => None,
        }
    }

    async fn progress(self) -> BTFResult<OperationProgress<Self>> {
        match self.0 {
            Erc20OpStage::SignMintOrder(data) => {
                log::debug!("ERC20OpStage::SignMintOrder {data:?}");
                Ok(OperationProgress::AddToService(SIGN_MINT_ORDER_SERVICE_ID))
            }
            Erc20OpStage::SendMintTransaction(data) => {
                log::debug!("ERC20OpStage::SendMintTransaction {data:?}",);
                Ok(OperationProgress::AddToService(SEND_MINT_TX_SERVICE_ID))
            }
            Erc20OpStage::WaitForMintConfirm { mint_results, .. } => {
                log::debug!("ERC20OpStage::WaitForMintConfirm {mint_results:?}");
                Err(bridge_did::error::Error::FailedToProgress(
                    "Erc20OpStage::ConfirmMint should progress by the event".into(),
                ))
            }
            Erc20OpStage::TokenMintConfirmed(data) => {
                log::debug!("ERC20OpStage::TokenMintConfirmed {data:?}");
                Err(bridge_did::error::Error::FailedToProgress(
                    "Erc20OpStage::TokenMintConfirmed should not progress".into(),
                ))
            }
        }
    }
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
    async fn run(&self) -> BTFResult<()> {
        let (base_result, wrapped_result) = futures::join!(self.base.run(), self.wrapped.run());
        base_result?;
        wrapped_result?;
        Ok(())
    }

    fn push_operation(&self, id: OperationId) -> BTFResult<()> {
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
    fn get_signer(&self) -> BTFResult<TxSigner> {
        self.config.borrow().get_signer()
    }

    fn get_order(&self, id: OperationId) -> Option<MintOrder> {
        let op = self.state.borrow().operations.get(id)?;
        let Erc20OpStage::SignMintOrder(order) = op.0.stage else {
            log::error!("Mint order handler failed to get MintOrder: unexpected state.");
            return None;
        };

        Some(order)
    }

    fn set_signed_order(&self, id: OperationId, signed: SignedOrders) {
        let Some(op) = self.state.borrow().operations.get(id) else {
            log::error!("Mint order handler failed to set MintOrder: operation not found.");
            return;
        };

        let Erc20OpStage::SignMintOrder(order) = op.0.stage else {
            log::error!("Mint order handler failed to set MintOrder: unexpected state.");
            return;
        };

        let should_send_mint_tx = order.fee_payer != H160::zero();
        log::trace!("Should send mint tx: {should_send_mint_tx}");
        let new_stage = match should_send_mint_tx {
            true => Erc20OpStage::SendMintTransaction(signed),
            false => Erc20OpStage::WaitForMintConfirm {
                order: signed,
                tx_hash: None,
                mint_results: vec![],
            },
        };

        log::trace!("New stage for operation {id}: {new_stage:?}");

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
            let scheduled_task = ScheduledTask::with_options(BridgeTask::new(id, new_op), options);
            self.scheduler.append_task(scheduled_task);
        }
    }
}

impl MintTxHandler for Erc20OrderHandler {
    fn get_signer(&self) -> BTFResult<TxSigner> {
        self.config.borrow().get_signer()
    }

    fn get_evm_config(&self) -> SharedConfig {
        self.config.clone()
    }

    fn get_signed_orders(&self, id: OperationId) -> Option<SignedOrders> {
        let Some(op) = self.state.borrow().operations.get(id) else {
            log::error!("Mint order handler failed to get MintOrder: operation not found.");
            return None;
        };
        let Erc20OpStage::SendMintTransaction(order) = op.0.stage else {
            log::error!(
                "MintTxHandler failed to get mint order batch: unexpected operation state."
            );
            return None;
        };

        Some(order)
    }

    fn mint_tx_sent(&self, id: OperationId, result: MintTxResult) {
        let Some(op) = self.state.borrow().operations.get(id) else {
            log::error!("MintTxHandler failed to update operation: not found.");
            return;
        };
        let Erc20OpStage::SendMintTransaction(order) = op.0.stage else {
            log::error!("MintTxHandler failed to update operation: unexpected state.");
            return;
        };

        log::debug!(
            "Mint transaction successful: {:?}; op_id: {id}; results: {:?}",
            result.tx_hash,
            result.results
        );
        self.state.borrow_mut().operations.update(
            id,
            Erc20BridgeOpImpl(Erc20BridgeOp {
                side: op.0.side,
                stage: Erc20OpStage::WaitForMintConfirm {
                    order,
                    tx_hash: result.tx_hash,
                    mint_results: result.results,
                },
            }),
        );
    }
}
