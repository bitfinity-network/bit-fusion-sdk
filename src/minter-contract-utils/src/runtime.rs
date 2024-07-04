use std::{cell::RefCell, rc::Rc};

use crate::bridge::BftBridge;

pub struct BridgeRuntime<Ctx, Bridge> {
    context: Ctx,
    bridge: Bridge,
}

impl<Ctx, Bridge> BridgeRuntime<Ctx, Bridge>
where
    Bridge: BftBridge,
{
    pub async fn update_evm_params(state: Rc<RefCell<State>>) -> Result<(), SchedulerError> {
        let client = state.borrow().config.get_evm_client();

        let Some(initial_params) = state.borrow().config.get_evm_params() else {
            log::warn!("no evm parameters set, unable to update");
            return Err(SchedulerError::TaskExecutionFailed(
                "no evm parameters set".into(),
            ));
        };

        let address = {
            let signer = state.borrow().signer.get_transaction_signer();
            signer.get_address().await.into_scheduler_result()?
        };

        // Update the EvmParams
        log::trace!("updating evm params");
        let responses = query::batch_query(
            &client,
            &[
                QueryType::Nonce {
                    address: address.into(),
                },
                QueryType::GasPrice,
            ],
        )
        .await
        .into_scheduler_result()?;

        let nonce: U256 = responses
            .get_value_by_id(Id::Str(NONCE_ID.into()))
            .into_scheduler_result()?;
        let gas_price: U256 = responses
            .get_value_by_id(Id::Str(GAS_PRICE_ID.into()))
            .into_scheduler_result()?;

        let params = EvmParams {
            nonce: nonce.0.as_u64(),
            gas_price,
            ..initial_params
        };

        state.borrow_mut().config.update_evm_params(|p| *p = params);
        log::trace!("evm params updated");

        Ok(())
    }
}

thread_local! {
    pub static STATE: Rc<RefCell<State>> = Rc::default();

    pub static SCHEDULER: Rc<RefCell<PersistentScheduler>> = Rc::new(RefCell::new({
        let pending_tasks =
            TasksStorage::new(MEMORY_MANAGER.with(|mm| mm.get(PENDING_TASKS_MEMORY_ID)));
            PersistentScheduler::new(pending_tasks)
    }));
}
