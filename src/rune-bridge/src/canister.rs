use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;
use std::str::FromStr;

use bitcoin::Address;
use bridge_canister::runtime::state::config::ConfigStorage;
use bridge_canister::runtime::{BridgeRuntime, RuntimeState};
use bridge_canister::BridgeCanister;
use bridge_did::op_id::OperationId;
use bridge_did::operation_log::{Memo, OperationLog};
use bridge_utils::common::Pagination;
use candid::Principal;
use did::H160;
use ic_canister::{generate_idl, init, post_upgrade, query, update, Canister, Idl, PreUpdate};
use ic_exports::ic_cdk::api::management_canister::ecdsa::{
    ecdsa_public_key, EcdsaPublicKeyArgument,
};
use ic_exports::ledger::Subaccount;
use ic_log::canister::{LogCanister, LogState};
use ic_metrics::{Metrics, MetricsStorage};
use ic_storage::IcStorage;

use crate::canister::inspect::{inspect_configure_ecdsa, inspect_configure_indexers};
use crate::core::deposit::RuneDeposit;
use crate::interface::GetAddressError;
use crate::ops::RuneBridgeOp;
use crate::rune_info::RuneInfo;
use crate::state::{RuneBridgeConfig, RuneState};

mod inspect;

#[derive(Canister, Clone, Debug)]
pub struct RuneBridge {
    #[id]
    id: Principal,
}

impl PreUpdate for RuneBridge {}

impl BridgeCanister for RuneBridge {
    fn config(&self) -> Rc<RefCell<ConfigStorage>> {
        ConfigStorage::get()
    }
}

impl RuneBridge {
    #[init]
    pub fn init(&mut self, config: RuneBridgeConfig) {
        let bridge_init_data = config.bridge_init_data();
        get_rune_state().borrow_mut().configure(config);
        self.init_bridge(bridge_init_data, Self::run_scheduler);
    }

    #[post_upgrade]
    pub fn post_upgrade(&mut self) {
        self.bridge_post_upgrade(Self::run_scheduler)
    }

    fn run_scheduler() {
        let runtime = get_runtime();
        runtime.borrow_mut().run();
    }

    /// Returns the bitcoin address that a user has to use to deposit runes to be received on the given Ethereum address.
    #[query]
    pub fn get_deposit_address(&self, eth_address: H160) -> Result<String, GetAddressError> {
        crate::key::get_transit_address(&get_rune_state(), &eth_address).map(|v| v.to_string())
    }

    #[query]
    pub fn get_operations_list(
        &self,
        wallet_address: H160,
        pagination: Option<Pagination>,
    ) -> Vec<(OperationId, RuneBridgeOp)> {
        get_runtime_state()
            .borrow()
            .operations
            .get_for_address(&wallet_address, pagination)
    }

    /// Returns operation by memo
    #[query]
    pub fn get_operation_by_memo_and_user(
        &self,
        memo: Memo,
        user_id: H160,
    ) -> Option<(OperationId, RuneBridgeOp)> {
        get_runtime_state()
            .borrow()
            .operations
            .get_operation_by_memo_and_user(&memo, &user_id)
    }

    /// Returns operation by memo
    #[query]
    pub fn get_operations_by_memo(&self, memo: Memo) -> Vec<(OperationId, RuneBridgeOp)> {
        get_runtime_state()
            .borrow()
            .operations
            .get_operations_by_memo(&memo)
    }

    /// Returns log of an operation by its ID.
    #[query]
    pub fn get_operation_log(
        &self,
        operation_id: OperationId,
    ) -> Option<OperationLog<RuneBridgeOp>> {
        get_runtime_state()
            .borrow()
            .operations
            .get_log(operation_id)
    }

    #[update]
    pub async fn admin_configure_ecdsa(&self) {
        inspect_configure_ecdsa(self.config());

        let key_id = get_rune_state().borrow().ecdsa_key_id();

        let master_key = ecdsa_public_key(EcdsaPublicKeyArgument {
            canister_id: None,
            derivation_path: vec![],
            key_id,
        })
        .await
        .expect("failed to get master key");

        get_rune_state().borrow_mut().configure_ecdsa(master_key.0);
    }

    #[update]
    pub async fn get_rune_balances(&self, btc_address: String) -> Vec<(RuneInfo, u128)> {
        let address = Address::from_str(&btc_address)
            .expect("invalid address")
            .assume_checked();

        let deposit = RuneDeposit::get(get_runtime_state());
        let utxos = deposit
            .get_deposit_utxos(&address)
            .await
            .expect("failed to get utxos");
        let (rune_info_amounts, _) = deposit
            .get_mint_amounts(&utxos.utxos, &None)
            .await
            .expect("failed to get rune amounts");

        rune_info_amounts
    }

    #[update]
    pub fn admin_configure_indexers(&self, no_of_indexer_urls: u8, indexer_urls: HashSet<String>) {
        inspect_configure_indexers(self.config());

        get_rune_state()
            .borrow_mut()
            .configure_indexers(no_of_indexer_urls, indexer_urls);
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

impl LogCanister for RuneBridge {
    fn log_state(&self) -> Rc<RefCell<LogState>> {
        LogState::get()
    }
}

type SharedRuntime = Rc<RefCell<BridgeRuntime<RuneBridgeOp>>>;

thread_local! {
    pub static RUNTIME: SharedRuntime =
        Rc::new(RefCell::new(BridgeRuntime::default(ConfigStorage::get())));

    pub static RUNE_STATE: Rc<RefCell<RuneState>> = Rc::default();
}

pub fn get_runtime() -> SharedRuntime {
    RUNTIME.with(|r| r.clone())
}

pub fn get_runtime_state() -> RuntimeState<RuneBridgeOp> {
    get_runtime().borrow().state().clone()
}

pub fn get_rune_state() -> Rc<RefCell<RuneState>> {
    RUNE_STATE.with(|s| s.clone())
}
