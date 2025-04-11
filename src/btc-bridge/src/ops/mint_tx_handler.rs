use bridge_canister::bridge::OperationContext as _;
use bridge_canister::runtime::RuntimeState;
use bridge_canister::runtime::service::mint_tx::MintTxHandler;
use bridge_canister::runtime::state::SharedConfig;
use bridge_did::error::BTFResult;
use bridge_did::op_id::OperationId;
use bridge_did::operations::BtcBridgeOp;
use bridge_did::order::SignedOrders;
use did::H256;
use eth_signer::sign_strategy::TxSigner;

use super::BtcBridgeOpImpl;

/// Allows MintTxService to handle MintTx of Btc bridge.
pub struct BtcMintTxHandler {
    state: RuntimeState<BtcBridgeOpImpl>,
}

impl BtcMintTxHandler {
    /// Creates a new instance of BtcMintTxHandler.
    pub fn new(state: RuntimeState<BtcBridgeOpImpl>) -> Self {
        Self { state }
    }
}

impl MintTxHandler for BtcMintTxHandler {
    fn get_signer(&self) -> BTFResult<TxSigner> {
        self.state.get_signer()
    }

    fn get_evm_config(&self) -> SharedConfig {
        self.state.borrow().config.clone()
    }

    fn get_signed_orders(&self, id: OperationId) -> Option<SignedOrders> {
        let op = self.state.borrow().operations.get(id)?;

        let BtcBridgeOp::MintErc20 { order, .. } = op.0 else {
            log::info!(
                "Mint order handler failed to get SignedOrders: unexpected state for operation {id}"
            );
            return None;
        };

        Some(order)
    }

    fn mint_tx_sent(&self, id: OperationId, tx_hash: H256) {
        let op = self.state.borrow().operations.get(id);
        let Some(BtcBridgeOp::MintErc20 { order, .. }) = op.map(|op| op.0) else {
            log::info!(
                "Mint order handler failed to update operation state: unexpected state for operation {id}"
            );
            return;
        };

        self.state.borrow_mut().operations.update(
            id,
            BtcBridgeOpImpl(BtcBridgeOp::ConfirmErc20Mint {
                order,
                tx_id: tx_hash,
            }),
        )
    }
}
