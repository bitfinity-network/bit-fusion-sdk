use bridge_did::{
    error::{BftResult, Error},
    op_id::OperationId,
};

use super::BridgeService;

struct FetchEvmLogsService {}

impl FetchEvmLogsService {
    const MAX_LOG_REQUEST_COUNT: u64 = 1000;
    async fn collect_evm_logs<Op: Operation>(
        ctx: RuntimeState<Op>,
        task_scheduler: DynScheduler<Op>,
    ) -> BftResult<()> {
        let collected = ctx.collect_evm_events(Self::MAX_LOG_REQUEST_COUNT).await?;
        let events = collected.events;

        ctx.borrow()
            .config
            .borrow_mut()
            .update_evm_params(|params| params.next_block = collected.last_block_number + 1);

        for event in events {
            let operation_action = match event {
                BridgeEvent::Burnt(event) => Op::on_wrapped_token_burnt(ctx.clone(), event).await,
                BridgeEvent::Minted(event) => Op::on_wrapped_token_minted(ctx.clone(), event).await,
                BridgeEvent::Notify(event) => {
                    Self::on_minter_notification(ctx.clone(), event, &task_scheduler).await
                }
            };

            let to_schedule = match operation_action {
                Some(OperationAction::Create(op, memo)) => {
                    let new_op_id = ctx.borrow_mut().operations.new_operation(op.clone(), memo);
                    op.scheduling_options().zip(Some((new_op_id, op)))
                }
                Some(OperationAction::CreateWithId(id, op, memo)) => {
                    ctx.borrow_mut()
                        .operations
                        .new_operation_with_id(id, op.clone(), memo);
                    op.scheduling_options().zip(Some((id, op)))
                }
                Some(OperationAction::Update { nonce, update_to }) => {
                    let Some((operation_id, _)) = ctx
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
                        return Err(Error::OperationNotFound(OperationId::new(nonce as _)));
                    };

                    ctx.borrow_mut()
                        .operations
                        .update(operation_id, update_to.clone());
                    update_to
                        .scheduling_options()
                        .zip(Some((operation_id, update_to)))
                }
                None => None,
            };

            if let Some((options, (op_id, op))) = to_schedule {
                let task = ScheduledTask::with_options(BridgeTask::Operation(op_id, op), options);
                task_scheduler.append_task(task);
            }
        }

        log::debug!("EVM logs collected");
        Ok(())
    }

    async fn on_minter_notification<Op: Operation>(
        ctx: RuntimeState<Op>,
        data: NotifyMinterEventData,
        scheduler: &DynScheduler<Op>,
    ) -> Option<OperationAction<Op>> {
        match data.notification_type {
            MinterNotificationType::RescheduleOperation => {
                let operation_id = match Decode!(&data.user_data, OperationId) {
                    Ok(v) => v,
                    Err(err) => {
                        log::warn!(
                        "Failed to decode operation id from reschedule operation request: {err:?}"
                    );
                        return None;
                    }
                };

                Self::reschedule_operation(ctx, operation_id, scheduler);
                None
            }
            _ => Op::on_minter_notification(ctx, data).await,
        }
    }

    fn reschedule_operation<Op: Operation>(
        ctx: RuntimeState<Op>,
        operation_id: OperationId,
        scheduler: &DynScheduler<Op>,
    ) {
        let Some(operation) = ctx.borrow().operations.get(operation_id) else {
            log::warn!(
                "Reschedule of operation #{operation_id} is requested but it does not exist"
            );
            return;
        };

        let Some(task_options) = operation.scheduling_options() else {
            log::info!("Reschedule of operation #{operation_id} is requested but no scheduling is required for this operation");
            return;
        };

        let current_task_id = scheduler.find_id(&|op| match op {
            BridgeTask::Operation(id, _) => id == operation_id,
            BridgeTask::Service(_) => false,
        });
        match current_task_id {
            Some(task_id) => {
                scheduler.reschedule(task_id, task_options.clone());
                log::trace!("Updated schedule for operation #{operation_id} task #{task_id} to {task_options:?}");
            }
            None => {
                let task_id = scheduler.append_task(
                    (BridgeTask::Operation(operation_id, operation), task_options).into(),
                );
                log::trace!("Restarted operation #{operation_id} with task id #{task_id}");
            }
        }
    }
}

impl BridgeService for FetchEvmLogsService {
    async fn run(&self) -> BftResult<()> {}

    fn push_operation(&self, _: OperationId) -> BftResult<()> {
        Err(Error::FailedToProgress(
            "Log fetch service doesn't requre operations".into(),
        ))
    }
}
