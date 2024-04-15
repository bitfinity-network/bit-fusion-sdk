use std::cell::RefCell;
use std::rc::Rc;

use candid::Principal;
use did::H160;
use ic_canister::{generate_idl, init, post_upgrade, query, Canister, Idl, PreUpdate};
use ic_metrics::{Metrics, MetricsStorage};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{StableUnboundedMap, VirtualMemory};
use ic_task_scheduler::retry::BackoffPolicy;
use ic_task_scheduler::scheduler::{Scheduler, TaskScheduler};
use ic_task_scheduler::task::{ScheduledTask, TaskOptions};

use crate::api::BridgeError;
use crate::constant::{
    EVM_INFO_INITIALIZATION_RETRIES, EVM_INFO_INITIALIZATION_RETRY_DELAY_SEC,
    EVM_INFO_INITIALIZATION_RETRY_MULTIPLIER,
};
use crate::memory::{MEMORY_MANAGER, PENDING_TASKS_MEMORY_ID};
use crate::scheduler::Brc20Task;
use crate::state::{Brc20BridgeConfig, State};

#[derive(Canister, Clone, Debug)]
pub struct Brc20Bridge {
    #[id]
    id: Principal,
}

impl PreUpdate for Brc20Bridge {}

impl Brc20Bridge {
    #[init]
    pub fn init(&mut self, config: Brc20BridgeConfig) {
        get_state().borrow_mut().configure(config);

        {
            let scheduler = get_scheduler();
            let mut borrowed_scheduler = scheduler.borrow_mut();
            borrowed_scheduler.set_failed_task_callback(|task, error| {
                log::error!("task failed: {task:?}, error: {error:?}")
            });
            borrowed_scheduler.append_task(Self::init_evm_info_task());
        }

        self.set_timers();
    }

    #[query]
    pub async fn get_deposit_address(&self, eth_address: H160) -> Result<String, BridgeError> {
        Ok(crate::ops::get_deposit_address(&get_state(), eth_address)
            .await?
            .to_string())
    }

    #[post_upgrade]
    pub fn post_upgrade(&mut self) {
        self.set_timers();
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }

    fn set_timers(&mut self) {
        #[cfg(target_family = "wasm")]
        {
            use std::time::Duration;
            const METRICS_UPDATE_INTERVAL_SEC: u64 = 60 * 60;

            self.update_metrics_timer(Duration::from_secs(METRICS_UPDATE_INTERVAL_SEC));

            const GLOBAL_TIMER_INTERVAL: Duration = Duration::from_secs(1);
            ic_exports::ic_cdk_timers::set_timer_interval(GLOBAL_TIMER_INTERVAL, move || {
                get_scheduler()
                    .borrow_mut()
                    .append_task(Self::collect_evm_events_task());

                let task_execution_result = get_scheduler().borrow_mut().run();

                if let Err(err) = task_execution_result {
                    log::error!("task execution failed: {err}",);
                }
            });
        }
    }

    fn init_evm_info_task() -> ScheduledTask<Brc20Task> {
        let init_options = TaskOptions::default()
            .with_max_retries_policy(EVM_INFO_INITIALIZATION_RETRIES)
            .with_backoff_policy(BackoffPolicy::Exponential {
                secs: EVM_INFO_INITIALIZATION_RETRY_DELAY_SEC,
                multiplier: EVM_INFO_INITIALIZATION_RETRY_MULTIPLIER,
            });
        Brc20Task::InitEvmState.into_scheduled(init_options)
    }

    #[cfg(target_family = "wasm")]
    fn collect_evm_events_task() -> ScheduledTask<Brc20Task> {
        const EVM_EVENTS_COLLECTING_DELAY: u32 = 1;

        let options = TaskOptions::default()
            .with_retry_policy(ic_task_scheduler::retry::RetryPolicy::Infinite)
            .with_backoff_policy(BackoffPolicy::Fixed {
                secs: EVM_EVENTS_COLLECTING_DELAY,
            });

        Brc20Task::CollectEvmEvents.into_scheduled(options)
    }
}

impl Metrics for Brc20Bridge {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
    }
}

pub type TasksStorage =
    StableUnboundedMap<u32, ScheduledTask<Brc20Task>, VirtualMemory<DefaultMemoryImpl>>;

pub type PersistentScheduler = Scheduler<Brc20Task, TasksStorage>;

thread_local! {
    pub static STATE: Rc<RefCell<State>> = Rc::default();

    pub static SCHEDULER: Rc<RefCell<PersistentScheduler>> = Rc::new(RefCell::new({
        let pending_tasks =
            TasksStorage::new(MEMORY_MANAGER.with(|mm| mm.get(PENDING_TASKS_MEMORY_ID)));
            PersistentScheduler::new(pending_tasks)
    }));
}

pub fn get_state() -> Rc<RefCell<State>> {
    STATE.with(|state| state.clone())
}

pub fn get_scheduler() -> Rc<RefCell<PersistentScheduler>> {
    SCHEDULER.with(|scheduler| scheduler.clone())
}
