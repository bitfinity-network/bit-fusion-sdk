use bridge_canister::bridge::OperationContext as _;
use bridge_canister::runtime::RuntimeState;
use bridge_canister::runtime::service::mint_tx::{MintTxHandler, MintTxResult};
use bridge_canister::runtime::state::SharedConfig;
use bridge_did::error::BTFResult;
use bridge_did::op_id::OperationId;
use bridge_did::operations::{RuneBridgeDepositOp, RuneBridgeOp};
use bridge_did::order::SignedOrders;
use eth_signer::sign_strategy::TxSigner;

use super::RuneBridgeOpImpl;

/// Allows MintTxService to handle MintTx of Rune bridge.
pub struct RuneMintTxHandler {
    state: RuntimeState<RuneBridgeOpImpl>,
}

impl RuneMintTxHandler {
    /// Creates a new instance of RuneMintTxHandler.
    pub fn new(state: RuntimeState<RuneBridgeOpImpl>) -> Self {
        Self { state }
    }
}

impl MintTxHandler for RuneMintTxHandler {
    fn get_signer(&self) -> BTFResult<TxSigner> {
        self.state.get_signer()
    }

    fn get_evm_config(&self) -> SharedConfig {
        self.state.borrow().config.clone()
    }

    fn get_signed_orders(&self, id: OperationId) -> Option<SignedOrders> {
        let op = self.state.borrow().operations.get(id)?;

        let RuneBridgeOp::Deposit(RuneBridgeDepositOp::SendMintOrder(order)) = op.0 else {
            log::info!(
                "Mint order handler failed to get SignedOrders: unexpected state for operation {id}"
            );
            return None;
        };

        Some(order)
    }

    fn mint_tx_sent(&self, id: OperationId, result: MintTxResult) {
        let op = self.state.borrow().operations.get(id);
        let Some(RuneBridgeOp::Deposit(RuneBridgeDepositOp::SendMintOrder(order))) =
            op.map(|op| op.0)
        else {
            log::info!(
                "Mint order handler failed to update operation state: unexpected state for operation {id}"
            );
            return;
        };

        log::debug!(
            "Mint transaction successful: {:?}; op_id: {id}; results: {:?}",
            result.tx_hash,
            result.results
        );
        self.state.borrow_mut().operations.update(
            id,
            RuneBridgeOpImpl(RuneBridgeOp::Deposit(
                RuneBridgeDepositOp::WaitForMintConfirm {
                    order,
                    tx_id: result.tx_hash,
                    mint_results: result.results,
                },
            )),
        )
    }
}
