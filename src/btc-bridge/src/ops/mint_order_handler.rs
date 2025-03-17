use bridge_canister::bridge::{Operation as _, OperationContext};
use bridge_canister::memory::StableMemory;
use bridge_canister::runtime::scheduler::{BridgeTask, SharedScheduler};
use bridge_canister::runtime::service::sign_orders::MintOrderHandler;
use bridge_canister::runtime::RuntimeState;
use bridge_did::error::BTFResult;
use bridge_did::op_id::OperationId;
use bridge_did::operations::BtcBridgeOp;
use bridge_did::order::{MintOrder, SignedOrders};
use eth_signer::sign_strategy::TxSigner;
use ic_task_scheduler::scheduler::TaskScheduler as _;
use ic_task_scheduler::task::ScheduledTask;

use super::BtcBridgeOpImpl;

/// Allows Signing service to handle MintOrders of Btc bridge.
pub struct BtcMintOrderHandler {
    state: RuntimeState<BtcBridgeOpImpl>,
    scheduler: SharedScheduler<StableMemory, BtcBridgeOpImpl>,
}

impl BtcMintOrderHandler {
    /// Creates a new instance of BtcMintOrderHandler.
    pub fn new(
        state: RuntimeState<BtcBridgeOpImpl>,
        scheduler: SharedScheduler<StableMemory, BtcBridgeOpImpl>,
    ) -> Self {
        Self { state, scheduler }
    }
}

impl MintOrderHandler for BtcMintOrderHandler {
    fn get_signer(&self) -> BTFResult<TxSigner> {
        self.state.get_signer()
    }

    fn get_order(&self, id: OperationId) -> Option<MintOrder> {
        let op = self.state.borrow().operations.get(id)?;
        let BtcBridgeOp::SignMintOrder { order, .. } = op.0 else {
            log::info!(
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

        if !matches!(op.0, BtcBridgeOp::SignMintOrder { .. }) {
            log::info!("Mint order handler failed to set MintOrder: unexpected state.");
            return;
        }

        let new_op = BtcBridgeOpImpl(BtcBridgeOp::MintErc20 { order: signed });
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
