use std::cell::RefCell;
use std::rc::Rc;

use candid::{CandidType, Principal};
use did::H160;
use eth_signer::sign_strategy::TransactionSigner;
use ic_canister::{generate_idl, init, post_upgrade, query, update, Canister, Idl, PreUpdate};
use ic_exports::ic_cdk::api::management_canister::ecdsa::{
    ecdsa_public_key, EcdsaPublicKeyArgument,
};
use ic_exports::ic_kit::ic;
use ic_exports::ledger::Subaccount;
use ic_metrics::{Metrics, MetricsStorage};
use ic_stable_structures::CellStructure;
use ic_task_scheduler::retry::BackoffPolicy;
use ic_task_scheduler::scheduler::TaskScheduler;
use ic_task_scheduler::task::{ScheduledTask, TaskOptions};
use serde::Deserialize;

use crate::interface::{DepositError, Erc20MintStatus, GetAddressError};
use crate::memory::{MEMORY_MANAGER, PENDING_TASKS_MEMORY_ID};
use crate::scheduler::{BtcTask, PersistentScheduler, TasksStorage};
use crate::state::{BftBridgeConfig, RuneBridgeConfig, State};
use crate::{
    EVM_INFO_INITIALIZATION_RETRIES, EVM_INFO_INITIALIZATION_RETRY_DELAY_SEC,
    EVM_INFO_INITIALIZATION_RETRY_MULTIPLIER,
};

#[derive(Canister, Clone, Debug)]
pub struct RuneBridge {
    #[id]
    id: Principal,
}

impl PreUpdate for RuneBridge {}

#[derive(Debug, CandidType, Deserialize)]
pub struct InitArgs {
    ck_btc_minter: Principal,
    ck_btc_ledger: Principal,
}

impl RuneBridge {
    fn set_timers(&mut self) {
        #[cfg(target_family = "wasm")]
        {
            use std::time::Duration;
            const METRICS_UPDATE_INTERVAL_SEC: u64 = 60 * 60;

            self.update_metrics_timer(std::time::Duration::from_secs(METRICS_UPDATE_INTERVAL_SEC));

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

    #[init]
    pub async fn init(&mut self, config: RuneBridgeConfig) {
        get_state().borrow_mut().configure(config).await;

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

    #[post_upgrade]
    pub fn post_upgrade(&mut self) {
        self.set_timers();
    }

    #[query]
    pub fn get_deposit_address(&self, eth_address: H160) -> Result<String, GetAddressError> {
        crate::key::get_deposit_address(&get_state(), &eth_address).map(|v| v.to_string())
    }

    #[update]
    pub async fn deposit(&self, eth_address: H160) -> Result<Erc20MintStatus, DepositError> {
        crate::ops::deposit(get_state(), &eth_address).await
    }

    fn init_evm_info_task() -> ScheduledTask<BtcTask> {
        let init_options = TaskOptions::default()
            .with_max_retries_policy(EVM_INFO_INITIALIZATION_RETRIES)
            .with_backoff_policy(BackoffPolicy::Exponential {
                secs: EVM_INFO_INITIALIZATION_RETRY_DELAY_SEC,
                multiplier: EVM_INFO_INITIALIZATION_RETRY_MULTIPLIER,
            });
        BtcTask::InitEvmState.into_scheduled(init_options)
    }

    /// Returns EVM address of the canister.
    #[update]
    pub async fn get_evm_address(&self) -> Option<H160> {
        let signer = get_state().borrow().signer().get().clone();
        match signer.get_address().await {
            Ok(address) => Some(address),
            Err(e) => {
                log::error!("failed to get EVM address: {e}");
                None
            }
        }
    }

    #[update]
    pub async fn admin_configure_ecdsa(&self) {
        let key_id = get_state().borrow().ecdsa_key_id();

        let master_key = ecdsa_public_key(EcdsaPublicKeyArgument {
            canister_id: None,
            derivation_path: vec![],
            key_id,
        })
        .await
        .expect("failed to get master key");
        get_state().borrow_mut().configure_ecdsa(master_key.0);
    }

    #[update]
    pub fn admin_configure_bft_bridge(&self, config: BftBridgeConfig) {
        get_state().borrow().check_admin(ic::caller());
        get_state().borrow_mut().configure_bft(config);
    }

    #[cfg(target_family = "wasm")]
    fn collect_evm_events_task() -> ScheduledTask<BtcTask> {
        const EVM_EVENTS_COLLECTING_DELAY: u32 = 1;

        let options = TaskOptions::default()
            .with_retry_policy(ic_task_scheduler::retry::RetryPolicy::Infinite)
            .with_backoff_policy(BackoffPolicy::Fixed {
                secs: EVM_EVENTS_COLLECTING_DELAY,
            });

        BtcTask::CollectEvmEvents.into_scheduled(options)
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}

pub fn eth_address_to_subaccount(eth_address: &H160) -> Subaccount {
    let mut subaccount = [0; 32];
    subaccount[0..eth_address.0 .0.len()].copy_from_slice(eth_address.0.as_bytes());

    Subaccount(subaccount)
}

impl Metrics for RuneBridge {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
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

pub fn get_state() -> Rc<RefCell<State>> {
    STATE.with(|state| state.clone())
}

pub fn get_scheduler() -> Rc<RefCell<PersistentScheduler>> {
    SCHEDULER.with(|scheduler| scheduler.clone())
}
