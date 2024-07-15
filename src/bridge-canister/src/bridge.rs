#![allow(async_fn_in_trait)]

use bridge_did::error::BftResult;
use bridge_utils::bft_events::{BurntEventData, MintedEventData, NotifyMinterEventData};
use bridge_utils::evm_bridge::EvmParams;
use bridge_utils::evm_link::EvmLink;
use candid::CandidType;
use did::H160;
use eth_signer::sign_strategy::TransactionSigner;
use ic_task_scheduler::task::TaskOptions;
use serde::{Deserialize, Serialize};

pub trait Operation:
    Sized + CandidType + Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static
{
    async fn progress(self, ctx: impl OperationContext) -> BftResult<Self>;

    fn scheduling_options(&self) -> Option<TaskOptions> {
        Some(TaskOptions::default())
    }

    fn is_complete(&self) -> bool;
}

pub trait OperationContext {
    fn get_evm_link(&self) -> EvmLink;
    fn get_bridge_contract_address(&self) -> BftResult<H160>;
    fn get_evm_params(&self) -> BftResult<EvmParams>;
    fn get_signer(&self) -> BftResult<impl TransactionSigner>;
}

pub trait EventHandler {
    type Stage;

    async fn on_wrapped_token_minted(
        &self,
        event: MintedEventData,
    ) -> Option<OperationAction<Self::Stage>>;

    async fn on_wrapped_token_burnt(
        &self,
        event: BurntEventData,
    ) -> Option<OperationAction<Self::Stage>>;

    async fn on_minter_notification(
        &self,
        event: NotifyMinterEventData,
    ) -> Option<OperationAction<Self::Stage>>;
}

pub enum OperationAction<Stage> {
    Create(Stage),
    Update {
        address: H160,
        nonce: u32,
        update_to: Stage,
    },
}
