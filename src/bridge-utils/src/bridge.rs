#![allow(async_fn_in_trait)]

use candid::CandidType;
use did::H160;
use eth_signer::sign_strategy::TransactionSigner;
use ic_task_scheduler::task::TaskOptions;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::bft_events::{BurntEventData, MintedEventData, NotifyMinterEventData};
use crate::evm_bridge::EvmParams;
use crate::evm_link::EvmLink;
use crate::operation_store::OperationId;

pub type BftResult<T> = Result<T, Error>;

pub trait Operation:
    Sized + CandidType + Serialize + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static
{
    async fn progress(self, ctx: impl OperationContext) -> Result<Self, Error>;

    fn scheduling_options(&self) -> Option<TaskOptions> {
        Some(TaskOptions::default())
    }

    fn is_complete(&self) -> bool;
}

pub trait OperationContext {
    fn get_evm_link(&self) -> EvmLink;
    fn get_bridge_contract_address(&self) -> BftResult<H160>;
    fn get_evm_params(&self) -> BftResult<EvmParams>;
    fn get_signer(&self) -> impl TransactionSigner;
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

#[derive(Debug, Error)]
pub enum Error {
    #[error("initialization failure: {0}")]
    Initialization(String),

    #[error("serializer failure: {0}")]
    Serialization(String),

    #[error("signer failure: {0}")]
    Signing(String),

    #[error("generic error: code=={code}, message=`{msg}`")]
    Other { code: u32, msg: String },

    #[error("operation#{0} not found")]
    OperationNotFound(OperationId),
}

pub enum OperationAction<Stage> {
    Create(Stage),
    Update {
        address: H160,
        nonce: u32,
        update_to: Stage,
    },
}
