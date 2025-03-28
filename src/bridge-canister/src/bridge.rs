#![allow(async_fn_in_trait)]

use bridge_did::error::{BTFResult, Error};
use bridge_did::evm_link::EvmLink;
use bridge_did::op_id::OperationId;
use bridge_did::operation_log::Memo;
use bridge_utils::btf_events::BridgeEvent;
use bridge_utils::evm_bridge::EvmParams;
use bridge_utils::evm_link::EvmLinkClient;
use candid::CandidType;
use did::H160;
use eth_signer::sign_strategy::TxSigner;
use ic_task_scheduler::task::TaskOptions;
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::runtime::service::ServiceId;
use crate::runtime::RuntimeState;

/// Defines an operation that can be executed by the bridge.
pub trait Operation:
    Sized + CandidType + Serialize + DeserializeOwned + Clone + Send + Sync + 'static
{
    /// Execute the operation, and move it to next stage.
    async fn progress(
        self,
        id: OperationId,
        ctx: RuntimeState<Self>,
    ) -> BTFResult<OperationProgress<Self>>;

    /// Check if the operation is complete.
    fn is_complete(&self) -> bool;

    /// Address of EVM wallet to/from which operation will move tokens.
    fn evm_wallet_address(&self) -> H160;

    /// Describes how the operation execution should be scheduled.
    fn scheduling_options(&self) -> Option<TaskOptions> {
        Some(TaskOptions::default())
    }
}

/// Context for an operation execution.
pub trait OperationContext {
    /// Get link to the EVM with wrapped tokens.
    fn get_evm_link(&self) -> EvmLink;

    /// Get address of the Btfbridge contract.
    fn get_bridge_contract_address(&self) -> BTFResult<H160>;

    /// Get EVM parameters.
    fn get_evm_params(&self) -> BTFResult<EvmParams>;

    /// Get signer for transactions, orders, etc...
    fn get_signer(&self) -> BTFResult<TxSigner>;

    async fn collect_evm_events(&self, max_logs_number: u64) -> BTFResult<CollectedEvents> {
        let link = self.get_evm_link();

        log::trace!("collecting evm events from {link:?}");

        let client = link.get_json_rpc_client();
        let evm_params = self.get_evm_params()?;
        let bridge_contract = self.get_bridge_contract_address()?;

        let last_chain_block = match client.get_block_number().await {
            Ok(block) => block,
            Err(e) => {
                log::warn!("failed to get evm block number: {e}");
                return Err(Error::EvmRequestFailed(e.to_string()));
            }
        };
        let last_request_block = last_chain_block.min(evm_params.next_block + max_logs_number);

        let events = BridgeEvent::collect(
            &client,
            evm_params.next_block,
            last_request_block,
            bridge_contract.0,
        )
        .await?;

        if !events.is_empty() {
            log::debug!("collected EVM events: {events:?}");
        }

        Ok(CollectedEvents {
            events,
            last_block_number: last_request_block,
        })
    }
}

/// Variants of operation progress.
#[derive(Debug, PartialEq, Eq)]
pub enum OperationProgress<Op> {
    Progress(Op),
    AddToService(ServiceId),
}

/// Action to create or update an operation.
#[derive(Debug, PartialEq, Eq)]
pub enum OperationAction<Op> {
    Create(Op, Option<Memo>),
    CreateWithId(OperationId, Op, Option<Memo>),
    Update { nonce: u32, update_to: Op },
}

#[derive(Debug)]
pub struct CollectedEvents {
    pub events: Vec<BridgeEvent>,
    pub last_block_number: u64,
}
