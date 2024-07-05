use candid::CandidType;
use did::H160;
use eth_signer::sign_strategy::TransactionSigner;
use ic_task_scheduler::task::TaskOptions;
use serde::Deserialize;
use thiserror::Error;

use crate::{
    bft_bridge_api::{BurntEventData, MintedEventData, NotifyMinterEventData},
    evm_bridge::EvmParams,
    evm_link::EvmLink,
};

type StageId = u64;

pub type BftResult<T> = Result<T, Error>;

pub trait Operation:
    Sized + CandidType + for<'de> Deserialize<'de> + Clone + Send + Sync + 'static
{
    async fn progress(self, ctx: impl OperationContext) -> Result<Self, Error>;

    fn scheduling_options(&self) -> Option<TaskOptions> {
        Some(TaskOptions::default())
    }

    fn is_complete(&self) -> bool;
}

pub trait OperationContext {
    fn get_evm_link(&self) -> EvmLink;
    fn get_bridge_contract_address(&self) -> H160;
    fn get_evm_params(&self) -> BftResult<EvmParams>;
    fn get_signer(&self) -> impl TransactionSigner;
}

pub trait EventHandler {
    type Stage;

    async fn on_wrapped_token_minted(
        &self,
        event: MintedEventData,
    ) -> Option<StageAction<Self::Stage>>;

    async fn on_wrapped_token_burnt(
        &self,
        event: BurntEventData,
    ) -> Option<StageAction<Self::Stage>>;

    async fn on_minter_notification(
        &self,
        event: NotifyMinterEventData,
    ) -> Option<StageAction<Self::Stage>>;
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("serializer failure: {0}")]
    Serialization(String),

    #[error("signer failure: {0}")]
    Signing(String),

    #[error("generic error: code=={code}, message=`{msg}`")]
    Other { code: u32, msg: String },
}

pub enum StageAction<Stage> {
    Create(Stage),
    Update {
        address: H160,
        nonce: u32,
        update_to: Stage,
    },
}

enum UserOperations {
    PredefinedSteps,

    // Custom
    IcrcBurn,
    IcrcMint,
}

// in the lib
enum PredefinedSteps {
    OrderSign,
    OrderSinged,
    OrderSent,
    WrappedBurnt,
}
