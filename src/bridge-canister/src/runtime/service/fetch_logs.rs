use bridge_did::error::{BftResult, Error};
use bridge_did::event_data::{BurntEventData, MintedEventData, NotifyMinterEventData};
use bridge_did::op_id::OperationId;
use bridge_did::operation_log::Memo;
use bridge_utils::bft_events::BridgeEvent;

use super::BridgeService;
use crate::bridge::{Operation, OperationAction, OperationContext};
use crate::runtime::state::SharedConfig;
use crate::runtime::{RuntimeState, SharedRuntime};

pub trait BftBridgeEventHandler<Op> {
    /// Action to perform when a WrappedToken is minted.
    fn on_wrapped_token_minted(&self, event: MintedEventData) -> Option<OperationAction<Op>>;

    /// Action to perform when a WrappedToken is burnt.
    fn on_wrapped_token_burnt(&self, event: BurntEventData) -> Option<OperationAction<Op>>;

    /// Action to perform on notification from BftBridge contract.
    fn on_minter_notification(&self, event: NotifyMinterEventData) -> Option<OperationAction<Op>>;
}

/// Service to fetch logs from evm and process it using event handler H.
pub struct FetchBftBridgeEventsService<Op: Operation, H> {
    handler: H,
    runtime: SharedRuntime<Op>,
    evm_config: SharedConfig,
}

impl<Op: Operation, H: BftBridgeEventHandler<Op>> FetchBftBridgeEventsService<Op, H> {
    const MAX_LOG_REQUEST_COUNT: u64 = 1000;

    /// Creates new instance of the service, which will fetch events using the `evm_config`
    /// and process it using the `handler`.
    pub fn new(handler: H, runtime: SharedRuntime<Op>, evm_config: SharedConfig) -> Self {
        Self {
            handler,
            runtime,
            evm_config,
        }
    }

    fn state(&self) -> RuntimeState<Op> {
        self.runtime.borrow().state().clone()
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
            let op_action = match event {
                BridgeEvent::Burnt(event) => self.handler.on_wrapped_token_burnt(event),
                BridgeEvent::Minted(event) => self.handler.on_wrapped_token_minted(event),
                BridgeEvent::Notify(event) => {
                    if let Some(operation_id) = event.try_decode_reschedule_operation_id() {
                        self.runtime.borrow().reschedule_operation(operation_id);
                        return Ok(());
                    }

                    self.handler.on_minter_notification(event)
                }
            };

            let Some(to_schedule) = op_action.and_then(|a| self.perform_action(a)) else {
                continue;
            };

            self.runtime
                .borrow()
                .schedule_operation(to_schedule.0, to_schedule.1);
        }

        log::debug!("EVM logs collected");
        Ok(())
    }

    fn perform_action(&self, action: OperationAction<Op>) -> Option<(OperationId, Op)> {
        let to_schedule = match action {
            OperationAction::Create(op, memo) => self.create_operation(op, memo),
            OperationAction::CreateWithId(id, op, memo) => {
                self.create_operation_with_id(id, op, memo)
            }
            OperationAction::Update { nonce, update_to } => {
                self.update_operation(nonce, update_to)?
            }
        };

        Some(to_schedule)
    }

    fn create_operation(&self, op: Op, memo: Option<Memo>) -> (OperationId, Op) {
        let new_op_id = self
            .state()
            .borrow_mut()
            .operations
            .new_operation(op.clone(), memo);
        (new_op_id, op)
    }

    fn create_operation_with_id(
        &self,
        op_id: OperationId,
        op: Op,
        memo: Option<Memo>,
    ) -> (OperationId, Op) {
        self.state()
            .borrow_mut()
            .operations
            .new_operation_with_id(op_id, op.clone(), memo);
        (op_id, op)
    }

    fn update_operation(&self, nonce: u32, update_to: Op) -> Option<(OperationId, Op)> {
        let Some((op_id, _)) = self
            .state()
            .borrow()
            .operations
            .get_for_address(&update_to.evm_wallet_address(), None)
            .into_iter()
            .find(|(operation_id, _)| operation_id.nonce() == nonce)
        else {
            log::warn!(
                "operation with dst_address = {} and nonce {} not found",
                update_to.evm_wallet_address(),
                nonce
            );
            return None;
        };

        self.state()
            .borrow_mut()
            .operations
            .update(op_id, update_to.clone());
        Some((op_id, update_to))
    }
}

#[async_trait::async_trait(?Send)]
impl<Op: Operation, H: BftBridgeEventHandler<Op>> BridgeService
    for FetchBftBridgeEventsService<Op, H>
{
    async fn run(&self) -> BftResult<()> {
        self.collect_evm_logs().await
    }

    fn push_operation(&self, _: OperationId) -> BftResult<()> {
        Err(Error::FailedToProgress(
            "Log fetch service doesn't requre operations".into(),
        ))
    }
}
