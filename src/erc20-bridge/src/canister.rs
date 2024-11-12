use std::cell::RefCell;
use std::rc::Rc;

use bridge_canister::memory::{memory_by_id, StableMemory};
use bridge_canister::runtime::service::fetch_logs::FetchBtfBridgeEventsService;
use bridge_canister::runtime::service::mint_tx::SendMintTxService;
use bridge_canister::runtime::service::sign_orders::SignMintOrdersService;
use bridge_canister::runtime::service::timer::ServiceTimer;
use bridge_canister::runtime::service::update_evm_params::RefreshEvmParamsService;
use bridge_canister::runtime::service::ServiceOrder;
use bridge_canister::runtime::state::config::ConfigStorage;
use bridge_canister::runtime::state::SharedConfig;
use bridge_canister::runtime::{BridgeRuntime, RuntimeState};
use bridge_canister::BridgeCanister;
use bridge_did::bridge_side::BridgeSide;
use bridge_did::error::{BTFResult, Error};
use bridge_did::init::erc20::BaseEvmSettings;
use bridge_did::init::BridgeInitData;
use bridge_did::op_id::OperationId;
use bridge_did::operation_log::{Memo, OperationLog};
use bridge_utils::common::Pagination;
use candid::Principal;
use did::build::BuildData;
use did::H160;
use eth_signer::sign_strategy::TransactionSigner;
use ic_canister::{generate_idl, init, post_upgrade, query, update, Canister, Idl, PreUpdate};
use ic_log::canister::{LogCanister, LogState};
use ic_metrics::{Metrics, MetricsStorage};
use ic_stable_structures::StableCell;
use ic_storage::IcStorage;

use crate::memory::NONCE_COUNTER_MEMORY_ID;
use crate::ops::events_handler::Erc20EventsHandler;
use crate::ops::{
    Erc20BridgeOpImpl, Erc20OrderHandler, Erc20ServiceSelector, FETCH_BASE_LOGS_SERVICE_ID,
    FETCH_WRAPPED_LOGS_SERVICE_ID, REFRESH_BASE_PARAMS_SERVICE_ID,
    REFRESH_WRAPPED_PARAMS_SERVICE_ID, SEND_MINT_TX_SERVICE_ID, SIGN_MINT_ORDER_SERVICE_ID,
};
use crate::state::SharedBaseEvmState;

#[cfg(feature = "export-api")]
pub mod inspect;

pub type SharedRuntime = Rc<RefCell<BridgeRuntime<Erc20BridgeOpImpl>>>;

#[derive(Canister, Clone, Debug)]
pub struct Erc20Bridge {
    #[id]
    id: Principal,
}

impl PreUpdate for Erc20Bridge {}

impl BridgeCanister for Erc20Bridge {
    fn config(&self) -> SharedConfig {
        ConfigStorage::get()
    }
}

impl Erc20Bridge {
    #[init]
    pub fn init(&mut self, bridge_settings: BridgeInitData, base_evm_settings: BaseEvmSettings) {
        get_base_evm_state().0.borrow_mut().reset(base_evm_settings);
        self.init_bridge(bridge_settings, Self::run_scheduler);
    }

    #[post_upgrade]
    pub fn post_upgrade(&mut self) {
        self.bridge_post_upgrade(Self::run_scheduler);
    }

    fn run_scheduler() {
        let runtime = get_runtime();
        runtime.borrow_mut().run();
    }

    #[update]
    fn set_base_btf_bridge_contract(&mut self, address: H160) {
        let config = get_runtime_state().borrow().config.clone();
        bridge_canister::inspect::inspect_set_btf_bridge_contract(config);
        get_base_evm_config()
            .borrow_mut()
            .set_btf_bridge_contract(address.clone());

        log::info!("Bridge canister base EVM BTF bridge contract address changed to {address}");
    }

    /// Retrieves all operations for the given ETH wallet address whose
    /// id is greater than or equal to `min_included_id` if provided.
    /// The operations are then paginated with the given `pagination` parameters,
    /// starting from `offset` returning a max of `count` items
    /// If `offset` is `None`, it starts from the beginning (i.e. the first entry is the min_included_id).
    /// If `count` is `None`, it returns all operations.
    #[query]
    pub fn get_operations_list(
        &self,
        wallet_address: H160,
        min_included_id: Option<OperationId>,
        pagination: Option<Pagination>,
    ) -> Vec<(OperationId, Erc20BridgeOpImpl)> {
        get_runtime_state().borrow().operations.get_for_address(
            &wallet_address,
            min_included_id,
            pagination,
        )
    }

    /// Returns operation by memo and user.
    #[query]
    pub fn get_operation_by_memo_and_user(
        &self,
        memo: Memo,
        user_id: H160,
    ) -> Option<(OperationId, Erc20BridgeOpImpl)> {
        get_runtime_state()
            .borrow()
            .operations
            .get_operation_by_memo_and_user(&memo, &user_id)
    }

    /// Returns all memos for a given user_id.
    #[query]
    pub fn get_memos_by_user_address(&self, user_id: H160) -> Vec<Memo> {
        get_runtime_state()
            .borrow()
            .operations
            .get_memos_by_user_address(&user_id)
    }

    /// Returns log of an operation by its ID.
    #[query]
    pub fn get_operation_log(
        &self,
        operation_id: OperationId,
    ) -> Option<OperationLog<Erc20BridgeOpImpl>> {
        get_runtime_state()
            .borrow()
            .operations
            .get_log(operation_id)
    }

    #[update]
    pub async fn get_bridge_canister_base_evm_address(&self) -> BTFResult<H160> {
        let signer = get_base_evm_config().borrow().get_signer()?;
        signer.get_address().await.map_err(|e| {
            Error::Initialization(format!("failed to get bridge canister address: {e}"))
        })
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

impl Metrics for Erc20Bridge {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
    }
}

impl LogCanister for Erc20Bridge {
    fn log_state(&self) -> Rc<RefCell<LogState>> {
        LogState::get()
    }
}

fn init_runtime() -> SharedRuntime {
    let runtime = BridgeRuntime::default(ConfigStorage::get());
    let base_state = get_base_evm_state();
    let wrapped_state = runtime.state().clone();
    let base_config = base_state.0.borrow().config.clone();
    let wrapped_config = wrapped_state.borrow().config.clone();
    let scheduler = runtime.scheduler().clone();
    let runtime = Rc::new(RefCell::new(runtime));

    // Init refresh_evm_params services
    let refresh_base_params_service = RefreshEvmParamsService::new(base_config.clone());
    let refresh_base_params_service_with_delay = ServiceTimer::new(
        refresh_base_params_service,
        base_state.query_delays().evm_params_query,
    );
    let refresh_wrapped_params_service = RefreshEvmParamsService::new(wrapped_config.clone());

    // Init event listener services
    let base_event_handler = Erc20EventsHandler::new(
        get_mint_order_nonce_counter(),
        BridgeSide::Base,
        base_config.clone(),
        wrapped_config.clone(),
    );
    let base_events_service =
        FetchBtfBridgeEventsService::new(base_event_handler, runtime.clone(), base_config.clone());
    let base_events_service_with_delay =
        ServiceTimer::new(base_events_service, base_state.query_delays().logs_query);
    let wrapped_event_handler = Erc20EventsHandler::new(
        get_mint_order_nonce_counter(),
        BridgeSide::Wrapped,
        wrapped_config.clone(),
        base_config.clone(),
    );
    let wrapped_events_service = FetchBtfBridgeEventsService::new(
        wrapped_event_handler,
        runtime.clone(),
        wrapped_config.clone(),
    );

    // Init operation handlers
    let base_handler =
        Erc20OrderHandler::new(wrapped_state.clone(), base_config, scheduler.clone());
    let wrapped_handler =
        Erc20OrderHandler::new(wrapped_state.clone(), wrapped_config, scheduler.clone());

    // Init mint order signing service
    let base_sign_service = SignMintOrdersService::new(base_handler.clone());
    let wrapped_sign_service = SignMintOrdersService::new(wrapped_handler.clone());
    let sign_service = Erc20ServiceSelector::new(base_sign_service, wrapped_sign_service);

    // Init mint tx service
    let base_mint_tx_service = SendMintTxService::new(base_handler);
    let wrapped_mint_tx_service = SendMintTxService::new(wrapped_handler);
    let send_mint_tx_service =
        Erc20ServiceSelector::new(base_mint_tx_service, wrapped_mint_tx_service);

    let services = wrapped_state.borrow().services.clone();
    services.borrow_mut().add_service(
        ServiceOrder::BeforeOperations,
        REFRESH_BASE_PARAMS_SERVICE_ID,
        Rc::new(refresh_base_params_service_with_delay),
    );
    services.borrow_mut().add_service(
        ServiceOrder::BeforeOperations,
        REFRESH_WRAPPED_PARAMS_SERVICE_ID,
        Rc::new(refresh_wrapped_params_service),
    );
    services.borrow_mut().add_service(
        ServiceOrder::BeforeOperations,
        FETCH_BASE_LOGS_SERVICE_ID,
        Rc::new(base_events_service_with_delay),
    );
    services.borrow_mut().add_service(
        ServiceOrder::BeforeOperations,
        FETCH_WRAPPED_LOGS_SERVICE_ID,
        Rc::new(wrapped_events_service),
    );
    services.borrow_mut().add_service(
        ServiceOrder::ConcurrentWithOperations,
        SIGN_MINT_ORDER_SERVICE_ID,
        Rc::new(sign_service),
    );
    services.borrow_mut().add_service(
        ServiceOrder::ConcurrentWithOperations,
        SEND_MINT_TX_SERVICE_ID,
        Rc::new(send_mint_tx_service),
    );

    runtime
}

pub type SharedNonceCounter = Rc<RefCell<StableCell<u32, StableMemory>>>;

thread_local! {
    pub static RUNTIME: SharedRuntime = init_runtime();

    pub static BASE_EVM_STATE: SharedBaseEvmState = SharedBaseEvmState::default();

    pub static MINT_ORDER_NONCE_COUNTER: SharedNonceCounter =
        Rc::new(RefCell::new(
            StableCell::new(memory_by_id(NONCE_COUNTER_MEMORY_ID), 0)
                .expect("failed to initialize nonce counter StableCell")
        ));
}

pub fn get_runtime() -> SharedRuntime {
    RUNTIME.with(|r| r.clone())
}

pub fn get_runtime_state() -> RuntimeState<Erc20BridgeOpImpl> {
    get_runtime().borrow().state().clone()
}

pub fn get_base_evm_state() -> SharedBaseEvmState {
    BASE_EVM_STATE.with(|s| s.clone())
}

pub fn get_base_evm_config() -> Rc<RefCell<ConfigStorage>> {
    BASE_EVM_STATE.with(|s| s.0.borrow().config.clone())
}

pub fn get_mint_order_nonce_counter() -> SharedNonceCounter {
    MINT_ORDER_NONCE_COUNTER.with(|c| c.clone())
}
