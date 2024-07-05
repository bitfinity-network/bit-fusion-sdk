use did::H160;
use ic_task_scheduler::task::TaskOptions;

use crate::bft_bridge_api::{BurntEventData, MintedEventData, NotifyMinterEventData};

type StageId = u64;

trait Operation: Sized {
    async fn progress(self) -> Result<Self, Error>;

    async fn should_be_scheduled(&self) -> bool {
        true
    }

    async fn scheduling_options(&self) -> TaskOptions {
        TaskOptions::default()
    }
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

#[derive(Debug)]
pub enum Error {
    Serialization(String),
    Signing(String),
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
