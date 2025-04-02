use std::cell::RefCell;
use std::collections::HashMap;

use alloy::consensus::{SignableTransaction as _, TxEnvelope};
use alloy::rpc::types::Transaction as AlloyRpcTransaction;
use bridge_did::error::{BTFResult, Error};
use bridge_did::op_id::OperationId;
use bridge_did::order::{SignedOrders, SignedOrdersData};
use bridge_utils::btf_events::{self, BatchMintErrorCode};
use bridge_utils::evm_link::EvmLinkClient;
use did::{BlockNumber, Transaction as DidTransaction, H256};
use eth_signer::sign_strategy::TxSigner;

use super::BridgeService;
use crate::runtime::state::SharedConfig;

/// Contains signed batch of mint orders and set of operations related to the batch.
#[derive(Debug, Clone)]
pub struct MintOrderBatchInfo {
    orders_batch: SignedOrdersData,
    related_operation: OperationId,
}

///  [`BridgeService::run`] Result of an operation for the mint transaction.
#[derive(Debug, Clone)]
pub struct MintTxResult {
    /// Transaction hash [`H256`] of the mint transaction.
    /// If the transaction was not sent, this field is [`None`].
    pub tx_hash: Option<H256>,
    /// For each order in the batch, the result of the mint transaction.
    pub results: Vec<BatchMintErrorCode>,
}

pub trait MintTxHandler {
    fn get_signer(&self) -> BTFResult<TxSigner>;
    fn get_evm_config(&self) -> SharedConfig;
    fn get_signed_orders(&self, id: OperationId) -> Option<SignedOrders>;
    fn mint_tx_sent(&self, id: OperationId, result: MintTxResult);
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
    async fn run(&self) -> BTFResult<()> {
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
                .get_btf_bridge_contract()
                .ok_or(Error::Initialization(
                    "Singing service failed to get Btfbridge address".into(),
                ))?;

        let evm_params = config.borrow().get_evm_params()?;
        let tx_params = evm_params.create_tx_params(sender, bridge_contract);
        let sender = tx_params.sender;

        log::trace!(
            "Sending batchMint transaction with {} mint orders.",
            batch_info.orders_batch.orders_number()
        );

        let mut tx = btf_events::batch_mint_transaction(
            tx_params,
            &batch_info.orders_batch.orders_data,
            &batch_info.orders_batch.signature,
            &[],
        );

        let signature = signer.sign_transaction(&mut tx).await?;
        let signed = tx.into_signed(signature.into());
        let transaction: DidTransaction = AlloyRpcTransaction {
            inner: signed.into(),
            from: sender,
            block_hash: None,
            block_number: None,
            transaction_index: None,
            effective_gas_price: None,
        }
        .into();

        let link = config.borrow().get_evm_link();
        let client = link.get_json_rpc_client();

        // make tx envelope
        let envelope: TxEnvelope = transaction.try_into().map_err(|e| {
            log::error!("failed to convert transaction to envelope: {e}");
            Error::EvmRequestFailed(format!("failed to convert transaction to envelope: {e}"))
        })?;

        // eth call to get the output
        let eth_call_request = envelope.clone().into();
        let output = client
            .eth_call(&eth_call_request, BlockNumber::Latest)
            .await
            .map_err(|e| {
                log::error!("Failed to call batch mint tx: {e}");
                Error::EvmRequestFailed(format!("failed to call batch mint tx: {e}"))
            })?;
        log::trace!("mint tx output of eth_call: {output}");

        // decode output
        let output = hex::decode(output.trim_start_matches("0x")).map_err(|e| {
            log::error!("Failed to decode batch mint tx output: {e}");
            Error::EvmRequestFailed(format!("failed to decode batch mint tx output: {e}"))
        })?;
        let mint_result = btf_events::batch_mint_result(&output).map_err(|e| {
            log::error!("Failed to decode batch mint tx output: {e}");
            Error::EvmRequestFailed(format!("failed to decode batch mint tx output: {e}"))
        })?;

        // zip operation ids with mint output
        let operation_id = batch_info.related_operation;

        // if at least one order is successful, commit the transaction, otherwise we can skip it
        let mut tx_hash = None;
        if mint_result.is_empty()
            || mint_result
                .iter()
                .any(|result| result == &BatchMintErrorCode::Ok)
        {
            // now commit the transaction
            tx_hash = Some(client.send_raw_transaction(&envelope).await.map_err(|e| {
                log::error!("Failed to send batch mint tx to EVM: {e}");
                Error::EvmRequestFailed(format!("failed to send batch mint tx to EVM: {e}"))
            })?);

            // Increase nonce after tx sending.
            self.handler
                .get_evm_config()
                .borrow_mut()
                .update_evm_params(|p| p.nonce += 1);

            log::trace!(
                "The batchMint transaction with {} mint orders sent.",
                batch_info.orders_batch.orders_number()
            );
        } else {
            log::trace!(
                "The batchMint transaction with {} mint orders not sent, because all of the order would fail.",
                batch_info.orders_batch.orders_number()
            );
        }

        // Remove sent orders batch from service.
        self.orders_to_send.borrow_mut().remove(&digest);

        // Update state for all operations related with the orders batch.
        log::trace!(
            "Updating state `mint_tx_sent` for operation {operation_id} and tx {tx_hash:?} (results: {mint_result:?})."
        );
        self.handler.mint_tx_sent(
            operation_id,
            MintTxResult {
                tx_hash,
                results: mint_result,
            },
        );

        log::trace!("SendMintTxService run finished.");

        Ok(())
    }

    fn push_operation(&self, op_id: OperationId) -> BTFResult<()> {
        let Some(order) = self.handler.get_signed_orders(op_id) else {
            log::warn!("Signed order not found for operation {op_id}.");
            return Err(bridge_did::error::Error::FailedToProgress(format!(
                "Signed order not found for operation {op_id}."
            )));
        };

        let orders_batch = order.into_inner();
        let digest = orders_batch.digest();
        {
            let mut orders_to_send = self.orders_to_send.borrow_mut();
            let entry = orders_to_send
                .entry(digest)
                .or_insert_with(|| MintOrderBatchInfo {
                    orders_batch,
                    related_operation: op_id,
                });
            entry.related_operation = op_id;
        }

        Ok(())
    }
}
