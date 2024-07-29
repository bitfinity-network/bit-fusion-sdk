use std::cell::RefCell;
use std::rc::Rc;

use bridge_canister::runtime::state::config::ConfigStorage;
use bridge_canister::runtime::state::SharedConfig;
use bridge_canister::runtime::{BridgeRuntime, RuntimeState};
use bridge_canister::BridgeCanister;
use bridge_did::init::BridgeInitData;
use bridge_did::op_id::OperationId;
use bridge_utils::common::Pagination;
use candid::Principal;
use did::{build::BuildData, H160};
use ic_canister::{generate_idl, init, post_upgrade, query, Canister, Idl, PreUpdate};
use ic_metrics::{Metrics, MetricsStorage};
use ic_storage::IcStorage;

use crate::ops::Erc20BridgeOp;
use crate::state::{BaseEvmSettings, BaseEvmState};

pub type SharedRuntime = Rc<RefCell<BridgeRuntime<Erc20BridgeOp>>>;

#[derive(Canister, Clone, Debug)]
pub struct EvmMinter {
    #[id]
    id: Principal,
}

impl PreUpdate for EvmMinter {}

impl BridgeCanister for EvmMinter {
    fn config(&self) -> SharedConfig {
        ConfigStorage::get()
    }
}

impl EvmMinter {
    #[init]
    pub fn init(&mut self, bridge_settings: BridgeInitData, base_evm_settings: BaseEvmSettings) {
        get_base_evm_state().borrow_mut().reset(base_evm_settings);
        self.init_bridge(bridge_settings, Self::run_scheduler);
    }

    #[post_upgrade]
    pub fn post_upgrade(&mut self) {
        self.bridge_post_upgrade(Self::run_scheduler);
    }

    fn run_scheduler() {
        todo!("run base evm tasks");
        let runtime = get_runtime();
        runtime.borrow_mut().run();
    }

    #[query]
    /// Returns the list of operations for the given wallet address.
    /// Offset, if set, defines the starting index of the page,
    /// Count, if set, defines the number of elements in the page.
    pub fn get_operations_list(
        &self,
        wallet_address: H160,
        pagination: Option<Pagination>,
    ) -> Vec<(OperationId, Erc20BridgeOp)> {
        get_runtime_state()
            .borrow()
            .operations
            .get_for_address(&wallet_address, pagination)
    }

    /// Returns the build data of the canister
    #[query]
    fn get_canister_build_data(&self) -> BuildData {
        bridge_canister::build_data!()
    }

    /// Returns candid IDL.
    /// This should be the last fn to see previous endpoints in macro.
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
    pub static RUNTIME: SharedRuntime =
        Rc::new(RefCell::new(BridgeRuntime::default(ConfigStorage::get())));

    pub static BASE_EVM_STATE: Rc<RefCell<BaseEvmState>> = Rc::default();
}

pub fn get_runtime() -> SharedRuntime {
    RUNTIME.with(|r| r.clone())
}

pub fn get_runtime_state() -> RuntimeState<Erc20BridgeOp> {
    get_runtime().borrow().state().clone()
}

pub fn get_base_evm_state() -> Rc<RefCell<BaseEvmState>> {
    BASE_EVM_STATE.with(|s| s.clone())
}
