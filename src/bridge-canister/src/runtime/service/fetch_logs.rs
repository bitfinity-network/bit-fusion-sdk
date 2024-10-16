use bridge_did::{
    error::{BftResult, Error},
    event_data::{BurntEventData, MintedEventData, NotifyMinterEventData},
    op_id::OperationId,
};
use bridge_utils::bft_events::BridgeEvent;

use crate::{bridge::OperationContext, runtime::state::SharedConfig};

use super::BridgeService;

pub trait BftBridgeEventHandler {
    /// Action to perform when a WrappedToken is minted.
    fn on_wrapped_token_minted(&self, event: MintedEventData) -> BftResult<()>;

    /// Action to perform when a WrappedToken is burnt.
    fn on_wrapped_token_burnt(&self, event: BurntEventData) -> BftResult<()>;

    /// Action to perform on notification from BftBridge contract.
    fn on_minter_notification(&self, event: NotifyMinterEventData) -> BftResult<()>;
}

pub struct FetchBftBridgeEventsService<H> {
    handler: H,
    evm_config: SharedConfig,
}

impl<H: BftBridgeEventHandler> FetchBftBridgeEventsService<H> {
    const MAX_LOG_REQUEST_COUNT: u64 = 1000;

    /// Creates new instance of the service, which will fetch events using the `evm_config`
    /// and process it using the `handler`.
    pub fn new(handler: H, evm_config: SharedConfig) -> Self {
        Self {
            handler,
            evm_config,
        }
    }

    async fn collect_evm_logs(&self) -> BftResult<()> {
        let collected = self
            .evm_config
            .collect_evm_events(Self::MAX_LOG_REQUEST_COUNT)
            .await?;
        let events = collected.events;

        self.evm_config
            .borrow_mut()
            .update_evm_params(|params| params.next_block = collected.last_block_number + 1);

        for event in events {
            let result = match event {
                BridgeEvent::Burnt(event) => self.handler.on_wrapped_token_burnt(event),
                BridgeEvent::Minted(event) => self.handler.on_wrapped_token_minted(event),
                BridgeEvent::Notify(event) => self.handler.on_minter_notification(event),
            };

            if let Err(e) = result {
                log::warn!("Failed to process bft event: {e}");
            }
        }

        log::debug!("EVM logs collected");
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl<H: BftBridgeEventHandler> BridgeService for FetchBftBridgeEventsService<H> {
    async fn run(&self) -> BftResult<()> {
        self.collect_evm_logs().await
    }

    fn push_operation(&self, _: OperationId) -> BftResult<()> {
        Err(Error::FailedToProgress(
            "Log fetch service doesn't requre operations".into(),
        ))
    }
}
