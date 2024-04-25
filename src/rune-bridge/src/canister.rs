use bitcoin::consensus::Encodable;
use bitcoin::hashes::sha256d::Hash;
use bitcoin::{Address, Amount, OutPoint, TxOut, Txid};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::str::FromStr;

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
use ord_rs::wallet::{ScriptType, TxInputInfo};
use ord_rs::OrdTransactionBuilder;
use serde::Deserialize;

use crate::interface::{
    CreateEdictTxArgs, DepositError, Erc20MintStatus, GetAddressError, WithdrawError,
};
use crate::memory::{MEMORY_MANAGER, PENDING_TASKS_MEMORY_ID};
use crate::ops::{get_fee_rate, get_rune_list, get_tx_outputs, get_utxos};
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
    pub fn init(&mut self, config: RuneBridgeConfig) {
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

    #[update]
    pub async fn get_rune_balances(&self, btc_address: String) -> Vec<(String, u128)> {
        let address = Address::from_str(&btc_address)
            .expect("invalid address")
            .assume_checked();
        let state = get_state();
        let utxos = get_utxos(&state, &address)
            .await
            .expect("failed to get utxos");

        let mut amounts: HashMap<String, u128> = HashMap::new();
        for utxo in &utxos.utxos {
            let outputs = get_tx_outputs(&state, utxo)
                .await
                .expect("failed to get utxo outputs");
            for rune in outputs.runes {
                let entry = amounts.entry(rune.0.rune.to_string()).or_default();
                *entry += rune.1.amount;
            }
        }

        amounts.into_iter().collect()
    }

    #[update]
    pub async fn create_edict_tx(&self, args: CreateEdictTxArgs) -> Vec<u8> {
        let from_addr = Address::from_str(&args.from_address)
            .expect("failed to parse from address")
            .assume_checked();
        let to_addr = Address::from_str(&args.destination)
            .expect("failed to parse destination address")
            .assume_checked();
        let change_addr = args
            .change_address
            .map(|addr| {
                Address::from_str(&addr)
                    .expect("failed to parse change address")
                    .assume_checked()
            })
            .unwrap_or_else(|| from_addr.clone());

        let state = get_state();
        let runes_list = get_rune_list(&state)
            .await
            .expect("failed to get rune list");
        let rune_id = runes_list
            .into_iter()
            .find(|(_, spaced_rune)| args.rune_name == spaced_rune.to_string())
            .unwrap_or_else(|| panic!("rune {} is not in the list of runes", args.rune_name))
            .0;

        let input_utxos = get_utxos(&state, &from_addr)
            .await
            .expect("failed to get input utxos");
        let inputs = input_utxos
            .utxos
            .iter()
            .map(|utxo| TxInputInfo {
                outpoint: OutPoint {
                    txid: Txid::from_raw_hash(*Hash::from_bytes_ref(
                        &utxo.outpoint.txid.clone().try_into().expect("invalid txid"),
                    )),
                    vout: utxo.outpoint.vout,
                },
                tx_out: TxOut {
                    value: Amount::from_sat(utxo.value),
                    script_pubkey: from_addr.script_pubkey(),
                },
            })
            .collect();

        let fee_rate = get_fee_rate(&state).await.expect("failed to get fee rate");

        let args = ord_rs::wallet::CreateEdictTxArgs {
            rune: rune_id,
            inputs,
            destination: to_addr,
            change_address: change_addr.clone(),
            rune_change_address: change_addr,
            amount: args.amount,
            fee_rate,
        };

        let builder = OrdTransactionBuilder::new(
            state.borrow().public_key(),
            ScriptType::P2WSH,
            state.borrow().wallet(vec![]),
        );
        let unsigned_tx = builder
            .create_edict_transaction(&args)
            .map_err(|err| {
                log::warn!("Failed to create withdraw transaction: {err:?}");
                WithdrawError::TransactionCreation
            })
            .expect("failed to create transaction");

        let mut bytes = vec![];
        unsigned_tx
            .consensus_encode(&mut bytes)
            .expect("failed to encode transaction");

        bytes
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
