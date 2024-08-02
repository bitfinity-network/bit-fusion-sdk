use std::cell::RefCell;
use std::rc::Rc;

use bridge_canister::runtime::state::config::ConfigStorage;
use bridge_canister::runtime::state::SharedConfig;
use bridge_canister::runtime::{BridgeRuntime, RuntimeState};
use bridge_canister::BridgeCanister;
use bridge_did::init::BridgeInitData;
use bridge_did::order::SignedMintOrder;
use bridge_utils::common::Pagination;
use candid::Principal;
use did::build::BuildData;
use did::H160;
use ic_canister::{
    generate_idl, init, post_upgrade, query, update, virtual_canister_call, Canister, Idl,
    PreUpdate,
};
use ic_ckbtc_minter::updates::get_btc_address::GetBtcAddressArgs;
use ic_exports::ic_cdk;
use ic_exports::ic_kit::ic;
use ic_exports::ledger::Subaccount;
use ic_log::canister::{LogCanister, LogState};
use ic_metrics::{Metrics, MetricsStorage};
use ic_storage::IcStorage;

use crate::ops::BtcBridgeOp;
use crate::state::{BtcConfig, State, WrappedTokenConfig};

type SharedRuntime = Rc<RefCell<BridgeRuntime<BtcBridgeOp>>>;

#[derive(Canister, Clone, Debug)]
pub struct BtcBridge {
    #[id]
    id: Principal,
}

impl PreUpdate for BtcBridge {}

impl BridgeCanister for BtcBridge {
    fn config(&self) -> SharedConfig {
        ConfigStorage::get()
    }
}

impl BtcBridge {
    #[init]
    pub fn init(&mut self, init_data: BridgeInitData) {
        self.init_bridge(init_data, Self::run_scheduler);
    }

    #[post_upgrade]
    pub fn post_upgrade(&mut self) {
        self.bridge_post_upgrade(Self::run_scheduler);
    }

    fn run_scheduler() {
        let runtime = get_runtime();
        runtime.borrow_mut().run();
    }

    /// Returns `(nonce, mint_order)` pairs for the given sender id.
    /// Offset, if set, defines the starting index of the page,
    /// Count, if set, defines the number of elements in the page.
    #[query]
    pub fn list_mint_orders(
        &self,
        wallet_address: H160,
        pagination: Option<Pagination>,
    ) -> Vec<(u32, SignedMintOrder)> {
        Self::token_mint_orders(wallet_address, pagination)
    }

    /// Returns `(nonce, mint_order)` pairs for the given sender id and operation_id.
    #[query]
    pub fn get_mint_order(
        &self,
        wallet_address: H160,
        operation_id: u32,
        pagination: Option<Pagination>,
    ) -> Option<SignedMintOrder> {
        Self::token_mint_orders(wallet_address, pagination)
            .into_iter()
            .find(|(nonce, _)| *nonce == operation_id)
            .map(|(_, mint_order)| mint_order)
    }

    #[update]
    pub async fn get_btc_address(&self, args: GetBtcAddressArgs) -> String {
        let ck_btc_minter = get_state().borrow().ck_btc_minter();
        virtual_canister_call!(ck_btc_minter, "get_btc_address", (args,), String)
            .await
            .unwrap()
    }

    #[update]
    pub fn admin_configure_btc(&self, config: BtcConfig) {
        if !Self::is_admin() {
            ic_cdk::trap("only owner can configure BFT bridge");
        }
        get_state().borrow_mut().configure_btc(config);
    }

    #[update]
    pub fn admin_configure_bft_bridge(&self, config: WrappedTokenConfig) {
        if !Self::is_admin() {
            ic_cdk::trap("only owner can configure BFT bridge");
        }
        get_state().borrow_mut().configure_wrapped_token(config);
    }

    /// Returns the build data of the canister
    #[query]
    fn get_canister_build_data(&self) -> BuildData {
        bridge_canister::build_data!()
    }

    /// Get mint orders for the given wallet address and token;
    /// if `offset` and `count` are provided, returns a page of mint orders.
    fn token_mint_orders(
        wallet_address: H160,
        pagination: Option<Pagination>,
    ) -> Vec<(u32, SignedMintOrder)> {
        let offset = pagination.as_ref().map(|p| p.offset).unwrap_or(0);
        let count = pagination.as_ref().map(|p| p.count).unwrap_or(usize::MAX);
        get_runtime_state()
            .borrow()
            .operations
            .get_for_address(&wallet_address, None)
            .into_iter()
            .filter_map(|(operation_id, operation)| {
                operation
                    .get_signed_mint_order()
                    .map(|mint_order| (operation_id.nonce(), mint_order))
            })
            .skip(offset)
            .take(count)
            .collect()
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }

    pub fn is_admin() -> bool {
        ic::caller() == get_runtime_state().borrow().config.borrow().get_owner()
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

impl LogCanister for BtcBridge {
    fn log_state(&self) -> Rc<RefCell<LogState>> {
        LogState::get()
    }
}

thread_local! {
    pub static STATE: Rc<RefCell<State>> = Rc::default();

    pub static RUNTIME: SharedRuntime =
        Rc::new(RefCell::new(BridgeRuntime::default(ConfigStorage::get())));
}

pub fn get_state() -> Rc<RefCell<State>> {
    STATE.with(|state| state.clone())
}

pub fn get_runtime() -> SharedRuntime {
    RUNTIME.with(|r| r.clone())
}

pub fn get_runtime_state() -> RuntimeState<BtcBridgeOp> {
    get_runtime().borrow().state().clone()
}

#[cfg(test)]
mod test {
    use candid::Principal;
    use eth_signer::sign_strategy::SigningStrategy;
    use ic_canister::{canister_call, Canister};
    use ic_exports::ic_kit::MockContext;

    use super::*;
    use crate::BtcBridge;

    fn owner() -> Principal {
        Principal::from_slice(&[1; 20])
    }

    #[tokio::test]
    #[should_panic = "admin principal is anonymous"]
    async fn disallow_anonymous_owner_in_init() {
        MockContext::new().inject();
        const MOCK_PRINCIPAL: &str = "mfufu-x6j4c-gomzb-geilq";
        let mock_canister_id = Principal::from_text(MOCK_PRINCIPAL).expect("valid principal");
        let mut canister = BtcBridge::from_principal(mock_canister_id);

        let init_data = BridgeInitData {
            owner: owner(),
            evm_principal: Principal::from_slice(&[2; 20]),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            log_settings: None,
        };
        canister_call!(canister.init(init_data), ()).await.unwrap();
    }
}
