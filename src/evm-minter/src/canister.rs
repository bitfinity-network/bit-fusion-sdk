use std::cell::RefCell;
use std::rc::Rc;

use candid::Principal;
use ic_canister::{generate_idl, init, post_upgrade, Canister, Idl, PreUpdate};
use ic_metrics::{Metrics, MetricsStorage};
use ic_task_scheduler::retry::BackoffPolicy;
use ic_task_scheduler::scheduler::TaskScheduler;
use ic_task_scheduler::task::{ScheduledTask, TaskOptions};

use crate::state::{BridgeSide, Settings, State};
use crate::tasks::PersistentTask;

const EVM_INFO_INITIALIZATION_RETRIES: u32 = 5;
const EVM_INFO_INITIALIZATION_RETRY_DELAY: u32 = 2;
const EVM_INFO_INITIALIZATION_RETRY_MULTIPLIER: u32 = 2;

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
            self.update_metrics_timer(std::time::Duration::from_secs(60 * 60));

            const GLOBAL_TIMER_INTERVAL: Duration = Duration::from_secs(1);
            ic_exports::ic_cdk_timers::set_timer_interval(GLOBAL_TIMER_INTERVAL, move || {
                let state = get_state();
                state
                    .borrow_mut()
                    .scheduler
                    .run()
                    .expect("scheduler failed to execute tasks");
            });
        }
    }

    #[init]
    pub fn init(&mut self, settings: Settings) {
        let state = get_state();
        state.borrow_mut().init(settings);

        let tasks = vec![
            Self::init_evm_info_task(BridgeSide::Base),
            Self::init_evm_info_task(BridgeSide::Wrapped),
        ];

        state.borrow_mut().scheduler.append_tasks(tasks);

        self.set_timers();
    }

    fn init_evm_info_task(bridge_side: BridgeSide) -> ScheduledTask<PersistentTask> {
        let init_options = TaskOptions::default()
            .with_max_retries_policy(EVM_INFO_INITIALIZATION_RETRIES)
            .with_backoff_policy(BackoffPolicy::Exponential {
                secs: EVM_INFO_INITIALIZATION_RETRY_DELAY,
                multiplier: EVM_INFO_INITIALIZATION_RETRY_MULTIPLIER,
            });
        PersistentTask::InitEvmState(bridge_side).into_scheduled(init_options)
    }

    #[post_upgrade]
    pub fn post_upgrade(&mut self) {
        self.set_timers();
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
