use std::cell::RefCell;
use std::rc::Rc;

use candid::Principal;
use did::error::EvmError;
use did::H160;
use eth_signer::sign_strategy::TransactionSigner;
use ic_canister::{generate_idl, init, post_upgrade, query, update, Canister, Idl, PreUpdate};
use ic_metrics::{Metrics, MetricsStorage};
use ic_stable_structures::CellStructure;
use ic_task_scheduler::retry::{BackoffPolicy, RetryPolicy};
use ic_task_scheduler::scheduler::TaskScheduler;
use ic_task_scheduler::task::{ScheduledTask, TaskOptions};
use minter_did::id256::Id256;
use minter_did::order::SignedMintOrder;

use crate::state::{BridgeSide, Settings, State};
use crate::tasks::BridgeTask;

const EVM_INFO_INITIALIZATION_RETRIES: u32 = 5;
const EVM_INFO_INITIALIZATION_RETRY_DELAY: u32 = 2;
const EVM_INFO_INITIALIZATION_RETRY_MULTIPLIER: u32 = 2;
const EVM_EVENTS_COLLECTING_DELAY: u32 = 1;

#[derive(Canister, Clone, Debug)]
pub struct EvmMinter {
    #[id]
    id: Principal,
}

impl PreUpdate for EvmMinter {}

impl EvmMinter {
    fn set_timers(&mut self) {
        // Set the metrics updating interval
        #[cfg(target_family = "wasm")]
        {
            use std::time::Duration;

            self.update_metrics_timer(std::time::Duration::from_secs(60 * 60));

            const GLOBAL_TIMER_INTERVAL: Duration = Duration::from_secs(1);
            ic_exports::ic_cdk_timers::set_timer_interval(GLOBAL_TIMER_INTERVAL, move || {
                let state = get_state();

                let task_execution_result = state.borrow_mut().scheduler.run();

                if let Err(err) = task_execution_result {
                    log::error!("task execution failed: {err}",);
                }
            });
        }
    }

    #[init]
    pub fn init(&mut self, settings: Settings) {
        let state = get_state();
        state.borrow_mut().init(settings);

        let tasks = vec![
            // Tasks to init EVMs state
            Self::init_evm_info_task(BridgeSide::Base),
            Self::init_evm_info_task(BridgeSide::Wrapped),
            // Tasks to collect EVMs events
            Self::collect_evm_info_task(BridgeSide::Base),
            Self::collect_evm_info_task(BridgeSide::Wrapped),
        ];

        state.borrow_mut().scheduler.append_tasks(tasks);

        self.set_timers();
    }

    fn init_evm_info_task(bridge_side: BridgeSide) -> ScheduledTask<BridgeTask> {
        let init_options = TaskOptions::default()
            .with_max_retries_policy(EVM_INFO_INITIALIZATION_RETRIES)
            .with_backoff_policy(BackoffPolicy::Exponential {
                secs: EVM_INFO_INITIALIZATION_RETRY_DELAY,
                multiplier: EVM_INFO_INITIALIZATION_RETRY_MULTIPLIER,
            });
        BridgeTask::InitEvmState(bridge_side).into_scheduled(init_options)
    }

    fn collect_evm_info_task(bridge_side: BridgeSide) -> ScheduledTask<BridgeTask> {
        let options = TaskOptions::default()
            .with_retry_policy(RetryPolicy::Infinite)
            .with_backoff_policy(BackoffPolicy::Fixed {
                secs: EVM_EVENTS_COLLECTING_DELAY,
            });

        BridgeTask::CollectEvmInfo(bridge_side).into_scheduled(options)
    }

    #[post_upgrade]
    pub fn post_upgrade(&mut self) {
        self.set_timers();
    }

    /// Returns `(operaion_id, signed_mint_order)` pairs for the given sender id.
    #[query]
    pub async fn list_mint_orders(
        &self,
        sender: Id256,
        src_token: Id256,
    ) -> Vec<(u32, SignedMintOrder)> {
        get_state().borrow().mint_orders.get_all(sender, src_token)
    }

    /// Returns the `signed_mint_order` if present.
    #[query]
    pub async fn get_mint_orders(
        &self,
        sender: Id256,
        src_token: Id256,
        operation_id: u32,
    ) -> Option<SignedMintOrder> {
        get_state()
            .borrow()
            .mint_orders
            .get(sender, src_token, operation_id)
    }

    /// Returns EVM address of the canister.
    #[update]
    pub async fn get_evm_address(&self) -> Result<H160, EvmError> {
        get_state().borrow().signer.get().get_address().await
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}

impl Metrics for EvmMinter {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
    }
}

thread_local! {
    pub static STATE: Rc<RefCell<State>> = Rc::default();
}

pub fn get_state() -> Rc<RefCell<State>> {
    STATE.with(|state| state.clone())
}

#[cfg(test)]
mod test {}
