use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use bridge_did::error::{BftResult, Error};
use bridge_did::op_id::OperationId;
use bridge_did::order::{SignedOrder, SignedOrders};
use bridge_utils::bft_events::{self};
use bridge_utils::evm_link::EvmLinkClient;
use did::H256;
use eth_signer::sign_strategy::TransactionSigner;

use super::BridgeService;
use crate::runtime::state::SharedConfig;

/// Contains signed batch of mint orders and set of operations related to the batch.
#[derive(Debug, Clone)]
pub struct MintOrderBatchInfo {
    orders_batch: SignedOrders,
    related_operations: HashSet<OperationId>,
}

pub trait MintTxHandler {
    fn get_signer(&self) -> BftResult<impl TransactionSigner>;
    fn get_evm_config(&self) -> SharedConfig;
    fn get_signed_orders(&self, id: OperationId) -> Option<SignedOrder>;
    fn mint_tx_sent(&self, id: OperationId, tx_hash: H256);
}

/// Service to send mint transaction with signed mint orders batch.
pub struct SendMintTxService<H> {
    handler: H,
    orders_to_send: RefCell<HashMap<H256, MintOrderBatchInfo>>,
}

impl<H> SendMintTxService<H> {
    /// Creates a new service with the given handler.
    pub fn new(handler: H) -> Self {
        Self {
            handler,
            orders_to_send: Default::default(),
        }
    }
}

#[async_trait::async_trait(?Send)]
impl<H: MintTxHandler> BridgeService for SendMintTxService<H> {
    async fn run(&self) -> BftResult<()> {
        log::trace!("Running SendMintTxService");

        let Some((digest, batch_info)) = self
            .orders_to_send
            .borrow()
            .iter()
            .map(|(digest, batch_info)| (digest.clone(), batch_info.clone()))
            .next()
        else {
            log::trace!("No mint orders batch ready to be sent.");
            return Ok(());
        };

        let config = self.handler.get_evm_config();

        let signer = config.borrow().get_signer()?;
        let sender = signer.get_address().await?;

        let bridge_contract =
            config
                .borrow()
                .get_bft_bridge_contract()
                .ok_or(Error::Initialization(
                    "Singing service failed to get BftBridge address".into(),
                ))?;

        let evm_params = config.borrow().get_evm_params()?;
        let tx_params = evm_params.create_tx_params(sender, bridge_contract);

        log::trace!(
            "Sending batchMint transaction with {} mint orders.",
            batch_info.orders_batch.orders_number()
        );
        let mut tx = bft_events::batch_mint_transaction(
            tx_params,
            &batch_info.orders_batch.orders_data,
            &batch_info.orders_batch.signature,
            &[],
        );

        let signature = signer.sign_transaction(&(&tx).into()).await?;
        tx.r = signature.r.0;
        tx.s = signature.s.0;
        tx.v = signature.v.0;
        tx.hash = tx.hash();

        let link = config.borrow().get_evm_link();
        let client = link.get_json_rpc_client();
        let tx_hash = client.send_raw_transaction(tx).await.map_err(|e| {
            Error::EvmRequestFailed(format!("failed to send batch mint tx to EVM: {e}"))
        })?;

        log::trace!(
            "The batchMint transaction with {} mint orders sent.",
            batch_info.orders_batch.orders_number()
        );

        // Remove sent orders batch from service.
        let sent_batch_info = match self.orders_to_send.borrow_mut().remove(&digest) {
            Some(batch_info) => batch_info,
            None => {
                log::warn!("Failed to remove signed mint orders which was just sent.");
                batch_info
            }
        };

        // Update state for all operations related with the orders batch.
        for op_id in sent_batch_info.related_operations {
            self.handler.mint_tx_sent(op_id, tx_hash.into())
        }

        log::trace!("SendMintTxService run finished.");

        Ok(())
    }

    fn push_operation(&self, op_id: OperationId) -> BftResult<()> {
        let Some(order) = self.handler.get_signed_orders(op_id) else {
            log::warn!("Signed order not found for operation {op_id}.");
            return Err(bridge_did::error::Error::FailedToProgress(format!(
                "Signed order not found for operation {op_id}."
            )));
        };

        let orders_batch = order.into_inner();
        let digest = orders_batch.digest();
        self.orders_to_send
            .borrow_mut()
            .entry(digest)
            .or_insert_with(|| MintOrderBatchInfo {
                orders_batch,
                related_operations: HashSet::new(),
            })
            .related_operations
            .insert(op_id);

        Ok(())
    }
}
