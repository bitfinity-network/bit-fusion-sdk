#![allow(async_fn_in_trait)]

use bridge_did::error::{BftResult, Error};
use bridge_did::op_id::OperationId;
use bridge_did::order::SignedMintOrder;
use bridge_utils::bft_events::{self, BurntEventData, MintedEventData, NotifyMinterEventData};
use bridge_utils::evm_bridge::EvmParams;
use bridge_utils::evm_link::EvmLink;
use candid::CandidType;
use did::{H160, H256};
use eth_signer::sign_strategy::TransactionSigner;
use ethers_core::types::BlockNumber;
use ic_task_scheduler::task::TaskOptions;
use serde::de::DeserializeOwned;
use serde::Serialize;

/// Defines an operation that can be executed by the bridge.
pub trait Operation:
    Sized + CandidType + Serialize + DeserializeOwned + Clone + Send + Sync + 'static
{
    /// Execute the operation, and move it to next stage.
    async fn progress(self, id: OperationId, ctx: impl OperationContext) -> BftResult<Self>;

    /// Check if the operation is complete.
    fn is_complete(&self) -> bool;

    /// Address of EVM wallet to/from which operation will move tokens.
    fn evm_wallet_address(&self) -> H160;

    /// Describes how the operation execution should be scheduled.
    fn scheduling_options(&self) -> Option<TaskOptions> {
        Some(TaskOptions::default())
    }

    /// Action to perform when a WrappedToken is minted.
    async fn on_wrapped_token_minted(
        _ctx: impl OperationContext,
        _event: MintedEventData,
    ) -> Option<OperationAction<Self>>;

    /// Action to perform when a WrappedToken is burnt.
    async fn on_wrapped_token_burnt(
        _ctx: impl OperationContext,
        _event: BurntEventData,
    ) -> Option<OperationAction<Self>>;

    /// Action to perform on notification from BftBridge contract.
    async fn on_minter_notification(
        _ctx: impl OperationContext,
        _event: NotifyMinterEventData,
    ) -> Option<OperationAction<Self>>;
}

/// Context for an operation execution.
pub trait OperationContext {
    /// Get link to the EVM with wrapped tokens.
    fn get_evm_link(&self) -> EvmLink;

    /// Get address of the BftBridge contract.
    fn get_bridge_contract_address(&self) -> BftResult<H160>;

    /// Get EVM parameters.
    fn get_evm_params(&self) -> BftResult<EvmParams>;

    /// Get signer for transactions, orders, etc...
    fn get_signer(&self) -> BftResult<impl TransactionSigner>;

    /// Send mint transaction with the given `order` to EVM.
    async fn send_mint_transaction(&self, order: &SignedMintOrder) -> BftResult<H256> {
        let signer = self.get_signer()?;
        let sender = signer.get_address().await?;
        let bridge_contract = self.get_bridge_contract_address()?;
        let evm_params = self.get_evm_params()?;

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

        let client = self.get_evm_link().get_json_rpc_client();

        let bridge_canister_address = tx.from;
        let balance_before_mint = client
            .get_balance(bridge_canister_address, BlockNumber::Latest)
            .await
            .unwrap();

        let tx_hash = client
            .send_raw_transaction(tx.clone())
            .await
            .map_err(|e| Error::EvmRequestFailed(format!("failed to send mint tx to EVM: {e}")))?;

        loop {
            let tx_receipt_result = client.get_receipt_by_hash(tx_hash).await;
            if let Ok(receipt) = tx_receipt_result {
                let gas_used = receipt.gas_used;
                log::info!("MINT_TX_GAS_USED: {gas_used:?}");

                let balance_after_mint = client
                    .get_balance(bridge_canister_address, BlockNumber::Latest)
                    .await
                    .unwrap();

                let balance_after_mint = client
                    .get_balance(bridge_canister_address, BlockNumber::Latest)
                    .await
                    .unwrap();

                let balance_after_mint = client
                    .get_balance(bridge_canister_address, BlockNumber::Latest)
                    .await
                    .unwrap();

                let balance_after_mint = client
                    .get_balance(bridge_canister_address, BlockNumber::Latest)
                    .await
                    .unwrap();

                let balance_after_mint = client
                    .get_balance(bridge_canister_address, BlockNumber::Latest)
                    .await
                    .unwrap();

                let balance_after_mint = client
                    .get_balance(bridge_canister_address, BlockNumber::Latest)
                    .await
                    .unwrap();

                let balance_after_mint = client
                    .get_balance(bridge_canister_address, BlockNumber::Latest)
                    .await
                    .unwrap();

                let balance_after_mint = client
                    .get_balance(bridge_canister_address, BlockNumber::Latest)
                    .await
                    .unwrap();

                let change = if balance_after_mint >= balance_before_mint {
                    log::info!("MINT_TX_BALANCE_INCREASED");
                    balance_after_mint - balance_before_mint
                } else {
                    log::info!("MINT_TX_BALANCE_DECREASED");
                    balance_before_mint - balance_after_mint
                };

                log::info!("MINT_TX_GAS_PRICE: {:?}", tx.gas_price);

                let gas_change = change / tx.gas_price.unwrap();
                log::info!("MINT_TX_GAS_CHANGE: {gas_change:?}");

                break;
            }
        }

        Ok(tx_hash.into())
    }
}

/// Action to create or update an operation.
pub enum OperationAction<Op> {
    Create(Op),
    Update { nonce: u32, update_to: Op },
}
