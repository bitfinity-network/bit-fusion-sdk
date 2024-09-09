use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

use bridge_canister::runtime::state::config::ConfigStorage;
use bridge_canister::runtime::{BridgeRuntime, RuntimeState};
use bridge_canister::BridgeCanister;
use bridge_did::init::BridgeInitData;
use bridge_did::op_id::OperationId;
use bridge_did::operation_log::OperationLog;
use bridge_utils::common::Pagination;
use candid::Principal;
use did::H160;
use ic_canister::{generate_idl, init, post_upgrade, query, update, Canister, Idl, PreUpdate};
use ic_exports::ic_cdk::api::management_canister::ecdsa::{
    ecdsa_public_key, EcdsaPublicKeyArgument,
};
use ic_exports::ic_kit::ic;
use ic_exports::ledger::Subaccount;
use ic_log::canister::{LogCanister, LogState};
use ic_metrics::{Metrics, MetricsStorage};
use ic_storage::IcStorage;

use crate::canister::inspect::inspect_is_owner;
use crate::interface::GetAddressError;
use crate::ops::Brc20BridgeOp;
use crate::state::{Brc20BridgeConfig, Brc20State};

mod inspect;

#[derive(Canister, Clone, Debug)]
pub struct Brc20Bridge {
    #[id]
    id: Principal,
}

impl PreUpdate for Brc20Bridge {}

impl BridgeCanister for Brc20Bridge {
    fn config(&self) -> Rc<RefCell<ConfigStorage>> {
        ConfigStorage::get()
    }
}

impl Brc20Bridge {
    #[init]
    pub fn init(&mut self, bridge_init_data: BridgeInitData, brc20_config: Brc20BridgeConfig) {
        self.init_bridge(bridge_init_data, Self::run_scheduler);
        get_brc20_state().borrow_mut().configure(brc20_config);
    }

    #[post_upgrade]
    pub fn post_upgrade(&mut self) {
        self.bridge_post_upgrade(Self::run_scheduler)
    }

    fn run_scheduler() {
        ic::spawn(async {
            let runtime = get_runtime();
            runtime.borrow_mut().run().await;
        })
    }

    /// Returns the bitcoin address that a user has to use to deposit runes to be received on the given Ethereum address.
    #[query]
    pub fn get_deposit_address(&self, eth_address: H160) -> Result<String, GetAddressError> {
        crate::key::get_transit_address(&get_brc20_state(), &eth_address)
            .map(|v| v.to_string())
            .map_err(GetAddressError::from)
    }

    #[query]
    pub fn get_operations_list(
        &self,
        wallet_address: H160,
        pagination: Option<Pagination>,
    ) -> Vec<(OperationId, Brc20BridgeOp)> {
        get_runtime_state()
            .borrow()
            .operations
            .get_for_address(&wallet_address, pagination)
    }

    /// Returns log of an operation by its ID.
    #[query]
    pub fn get_operation_log(
        &self,
        operation_id: OperationId,
    ) -> Option<OperationLog<Brc20BridgeOp>> {
        get_runtime_state()
            .borrow()
            .operations
            .get_log(operation_id)
    }

    #[update]
    pub async fn admin_configure_ecdsa(&self) {
        inspect_is_owner(self.config());

        let signing_strategy = get_runtime_state()
            .borrow()
            .config
            .borrow()
            .get_signing_strategy();

        let key_id = get_brc20_state().borrow().ecdsa_key_id(&signing_strategy);

        let (master_key,) = ecdsa_public_key(EcdsaPublicKeyArgument {
            canister_id: None,
            derivation_path: vec![],
            key_id: key_id.clone(),
        })
        .await
        .expect("failed to get master key");

        get_brc20_state()
            .borrow_mut()
            .configure_ecdsa(master_key, key_id)
            .expect("failed to configure ecdsa");
    }

    #[update]
    pub fn admin_configure_indexers(&self, indexer_urls: HashSet<String>) {
        inspect_is_owner(self.config());

        get_brc20_state()
            .borrow_mut()
            .configure_indexers(indexer_urls);
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

impl Metrics for Brc20Bridge {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
    }
}

impl LogCanister for Brc20Bridge {
    fn log_state(&self) -> Rc<RefCell<LogState>> {
        LogState::get()
    }
}

type SharedRuntime = Rc<RefCell<BridgeRuntime<Brc20BridgeOp>>>;

thread_local! {
    pub static RUNTIME: SharedRuntime =
        Rc::new(RefCell::new(BridgeRuntime::default(ConfigStorage::get())));

    pub static BRC20_STATE: Rc<RefCell<Brc20State>> = Rc::default();
}

pub fn get_runtime() -> SharedRuntime {
    RUNTIME.with(|r| r.clone())
}

pub fn get_runtime_state() -> RuntimeState<Brc20BridgeOp> {
    get_runtime().borrow().state().clone()
}

pub fn get_brc20_state() -> Rc<RefCell<Brc20State>> {
    BRC20_STATE.with(|s| s.clone())
}
