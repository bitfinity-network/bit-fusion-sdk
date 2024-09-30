use bridge_canister::bridge::{Operation as _, OperationContext};
use bridge_canister::memory::StableMemory;
use bridge_canister::runtime::scheduler::{BridgeTask, SharedScheduler};
use bridge_canister::runtime::service::sign_orders::MintOrderHandler;
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::BftResult;
use bridge_did::op_id::OperationId;
use bridge_did::operations::{RuneBridgeDepositOp, RuneBridgeOp};
use bridge_did::order::{MintOrder, SignedOrders};
use eth_signer::sign_strategy::TransactionSigner;
use ic_task_scheduler::scheduler::TaskScheduler as _;
use ic_task_scheduler::task::ScheduledTask;

use super::RuneBridgeOpImpl;

/// Allows Signing service to handle MintOrders of Rune bridge.
pub struct RuneMintOrderHandler {
    state: RuntimeState<RuneBridgeOpImpl>,
    scheduler: SharedScheduler<StableMemory, RuneBridgeOpImpl>,
}

impl RuneMintOrderHandler {
    /// Creates a new instance of RuneMintOrderHandler.
    pub fn new(
        state: RuntimeState<RuneBridgeOpImpl>,
        scheduler: SharedScheduler<StableMemory, RuneBridgeOpImpl>,
    ) -> Self {
        Self { state, scheduler }
    }
}

impl MintOrderHandler for RuneMintOrderHandler {
    fn get_signer(&self) -> BftResult<impl TransactionSigner> {
        self.state.get_signer()
    }

    fn get_order(&self, id: OperationId) -> Option<MintOrder> {
        let op = self.state.borrow().operations.get(id)?;
        let RuneBridgeOp::Deposit(RuneBridgeDepositOp::SignMintOrder(order)) = op.0 else {
            log::error!(
                "Mint order handler failed to get MintOrder: unexpected state for operation {id}"
            );
            return None;
        };

        Some(order)
    }

    fn set_signed_order(&self, id: OperationId, signed: SignedOrders) {
        let Some(op) = self.state.borrow().operations.get(id) else {
            log::info!("Mint order handler failed to set MintOrder: operation {id} not found.");
            return;
        };

        if !matches!(
            &op.0,
            RuneBridgeOp::Deposit(RuneBridgeDepositOp::SignMintOrder(_)),
        ) {
            log::error!(
                "Mint order handler failed to set MintOrder: unexpected state for operation {id}"
            );
            return;
        }

        let new_op = RuneBridgeOpImpl(RuneBridgeOp::Deposit(RuneBridgeDepositOp::SendMintOrder(
            signed,
        )));
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
