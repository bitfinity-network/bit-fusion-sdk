use std::cell::RefCell;
use std::rc::Rc;

use candid::{CandidType, Principal};
use did::H160;
use eth_signer::sign_strategy::TransactionSigner;
use ic_canister::virtual_canister_call;
use ic_canister::{generate_idl, init, post_upgrade, query, update, Canister, Idl, PreUpdate};
use ic_ckbtc_minter::updates::get_btc_address::GetBtcAddressArgs;
use ic_exports::ic_kit::ic;
use ic_exports::icrc_types::icrc1::account::Account;
use ic_exports::ledger::Subaccount;
use ic_metrics::{Metrics, MetricsStorage};
use ic_stable_structures::CellStructure;
use ic_task_scheduler::retry::BackoffPolicy;
use ic_task_scheduler::scheduler::TaskScheduler;
use ic_task_scheduler::task::{InnerScheduledTask, ScheduledTask, TaskOptions, TaskStatus};
use serde::Deserialize;

use crate::interface::{Erc20MintError, Erc20MintStatus};
use crate::memory::{MEMORY_MANAGER, PENDING_TASKS_MEMORY_ID};
use crate::scheduler::{BtcTask, PersistentScheduler, TasksStorage};
use crate::state::{BftBridgeConfig, BtcBridgeConfig, State};
use crate::{
    EVM_INFO_INITIALIZATION_RETRIES, EVM_INFO_INITIALIZATION_RETRY_DELAY_SEC,
    EVM_INFO_INITIALIZATION_RETRY_MULTIPLIER,
};

#[derive(Canister, Clone, Debug)]
pub struct BtcBridge {
    #[id]
    id: Principal,
}

impl PreUpdate for BtcBridge {}

#[derive(Debug, CandidType, Deserialize)]
pub struct InitArgs {
    ck_btc_minter: Principal,
    ck_btc_ledger: Principal,
}

impl BtcBridge {
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
    pub fn init(&mut self, config: BtcBridgeConfig) {
        get_state().borrow_mut().configure(config);

        {
            let scheduler = get_scheduler();
            let mut borrowed_scheduler = scheduler.borrow_mut();
            borrowed_scheduler.on_completion_callback(log_task_execution_error);
            borrowed_scheduler.append_task(Self::init_evm_info_task());
        }

        self.set_timers();
    }

    #[post_upgrade]
    pub fn post_upgrade(&mut self) {
        self.set_timers();
    }

    /// Converts Bitcoins into ERC20 wrapper token in EVM.
    ///
    /// # Arguments
    ///
    /// * `eth_address` - EVM Etherium address of the receiver of the wrapper tokens
    ///
    /// # Details
    ///
    /// Before this method is called, the Bitcoins to be bridged are to be transferred to a
    /// certain address. This address is received from `ckBTC` canister by calling `get_btc_address`
    /// query method. Account given as an argument to this method can be calculated as:
    /// * `owner` is BtcBridge canister principal
    /// * `subaccount` is right-zero-padded Etherium address of the caller
    ///
    /// Here is a sample Rust code:
    ///
    /// ```ignore
    /// let mut caller_subaccount = [0; 32];
    /// caller_subaccount[0..caller_eth_address.0.0.len()].copy_from_slice(caller_eth_address.0.as_bytes());
    ///
    /// let argument = Account {
    ///   owner: btc_bridge_canister_principal,
    ///   subaccount: Some(caller_subaccount),
    /// }
    /// ```
    ///
    /// After Bitcoins are transferred to the correct address, `btc_to_erc20` method can be called
    /// right away. (there is no need to wait for the Bitcoin confirmation process to complete) The
    /// method will return status of all pending transactions.
    ///
    /// After the number of Bitcoin confirmations surpass the number required by the ckBTC minter
    /// canister, the BtcBridge canister will automatically create a mint order for wrapped tokens
    /// and send it to the EVM. After the EVM transaction is confirmed, the minted wrapped tokens
    /// will appear at the given `eth_address`.
    #[update]
    pub async fn btc_to_erc20(
        &self,
        eth_address: H160,
    ) -> Vec<Result<Erc20MintStatus, Erc20MintError>> {
        crate::ops::btc_to_erc20(get_state(), eth_address).await
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

    /// Returns bridge contract address for EVM.
    /// If contract isn't initialized yet - returns None.
    #[query]
    pub fn get_bft_bridge_contract(&mut self) -> Option<H160> {
        Some(get_state().borrow().bft_config.bridge_address.clone())
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
    pub async fn get_btc_address(&self, args: GetBtcAddressArgs) -> String {
        let ck_btc_minter = get_state().borrow().ck_btc_minter();
        return virtual_canister_call!(ck_btc_minter, "get_btc_address", (args,), String)
            .await
            .unwrap();
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

impl Metrics for BtcBridge {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
    }
}

fn log_task_execution_error(task: InnerScheduledTask<BtcTask>) {
    match task.status() {
        TaskStatus::Failed {
            timestamp_secs,
            error,
        } => {
            log::error!(
                "task #{} execution failed: {error} at {timestamp_secs}",
                task.id()
            )
        }
        TaskStatus::TimeoutOrPanic { timestamp_secs } => {
            log::error!("task #{} panicked at {timestamp_secs}", task.id())
        }
        _ => (),
    };
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
