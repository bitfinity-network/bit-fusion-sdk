use std::cell::RefCell;
use std::rc::Rc;

use bridge_canister::bridge::OperationContext;
use bridge_canister::runtime::service::mint_tx::SendMintTxService;
use bridge_canister::runtime::service::sign_orders::SignMintOrdersService;
use bridge_canister::runtime::service::ServiceOrder;
use bridge_canister::runtime::state::config::ConfigStorage;
use bridge_canister::runtime::state::SharedConfig;
use bridge_canister::runtime::{BridgeRuntime, RuntimeState};
use bridge_canister::BridgeCanister;
use bridge_did::bridge_side::BridgeSide;
use bridge_did::error::{BftResult, Error};
use bridge_did::id256::Id256;
use bridge_did::init::BridgeInitData;
use bridge_did::op_id::OperationId;
use bridge_did::operation_log::{Memo, OperationLog};
use bridge_did::operations::{Erc20BridgeOp, Erc20OpStage};
use bridge_utils::bft_events::BridgeEvent;
use bridge_utils::common::Pagination;
use candid::Principal;
use did::build::BuildData;
use did::H160;
use drop_guard::guard;
use eth_signer::sign_strategy::TransactionSigner;
use ic_canister::{generate_idl, init, post_upgrade, query, update, Canister, Idl, PreUpdate};
use ic_exports::ic_kit::ic;
use ic_log::canister::{LogCanister, LogState};
use ic_metrics::{Metrics, MetricsStorage};
use ic_storage::IcStorage;

use crate::ops::{
    self, Erc20BridgeOpImpl, Erc20OrderHandler, Erc20ServiceSelector, SEND_MINT_TX_SERVICE_ID,
    SIGN_MINT_ORDER_SERVICE_ID,
};
use crate::state::{BaseEvmSettings, SharedBaseEvmState};

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
        if get_base_evm_state().0.borrow().should_collect_evm_logs() {
            get_base_evm_state().0.borrow_mut().collecting_logs_ts = Some(ic::time());
            ic::spawn(process_base_evm_logs());
        }

        if get_base_evm_state().0.borrow().should_refresh_evm_params() {
            get_base_evm_state().0.borrow_mut().refreshing_evm_params_ts = Some(ic::time());
            ic::spawn(get_base_evm_state().refresh_base_evm_params());
        }

        let runtime = get_runtime();
        runtime.borrow_mut().run();
    }

    #[update]
    fn set_base_bft_bridge_contract(&mut self, address: H160) {
        let config = get_runtime_state().borrow().config.clone();
        bridge_canister::inspect::inspect_set_bft_bridge_contract(config);
        get_base_evm_config()
            .borrow_mut()
            .set_bft_bridge_contract(address.clone());

        log::info!("Bridge canister base EVM BFT bridge contract address changed to {address}");
    }

    #[query]
    /// Returns the list of operations for the given wallet address.
    pub fn get_operations_list(
        &self,
        wallet_address: H160,
        pagination: Option<Pagination>,
    ) -> Vec<(OperationId, Erc20BridgeOpImpl)> {
        get_runtime_state()
            .borrow()
            .operations
            .get_for_address(&wallet_address, pagination)
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

    #[query]
    pub async fn get_bridge_canister_base_evm_address(&self) -> BftResult<H160> {
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

async fn process_base_evm_logs() {
    log::trace!("processing base evm logs");

    let _lock = guard(get_base_evm_state(), |s| {
        s.0.borrow_mut().collecting_logs_ts = None
    });

    let base_evm_state = get_base_evm_state();
    const MAX_LOGS_PER_REQUEST: u64 = 1000;
    let collect_result = base_evm_state
        .collect_evm_events(MAX_LOGS_PER_REQUEST)
        .await;
    let collected = match collect_result {
        Ok(c) => c,
        Err(err) => {
            log::warn!("failed to collect base EVM events: {err}");
            return;
        }
    };

    log::debug!("collected base evm events: {collected:?}");

    get_base_evm_config()
        .borrow_mut()
        .update_evm_params(|params| params.next_block = collected.last_block_number + 1);

    for event in collected.events {
        if let Err(e) = process_base_evm_event(event) {
            log::warn!("failed to process base EVM event: {e}")
        };
    }

    log::debug!("base EVM logs processed");
}

fn process_base_evm_event(event: BridgeEvent) -> BftResult<()> {
    match event {
        BridgeEvent::Burnt(event) => {
            log::trace!("base token burnt");

            let wrapped_evm_params = get_runtime_state().get_evm_params().expect(
                "process_base_evm_event must not be called if wrapped evm params are not initialized",
            );
            let base_evm_params = get_base_evm_config().borrow().get_evm_params().expect(
                "process_base_evm_logs must not be called if base evm params are not initialized",
            );

            let nonce = get_base_evm_state().0.borrow_mut().next_nonce();
            let order = ops::mint_order_from_burnt_event(
                event.clone(),
                base_evm_params,
                wrapped_evm_params,
                nonce,
            )
            .ok_or_else(|| {
                Error::Serialization(format!(
                    "failed to create mint order from base evm burnt event: {event:?}"
                ))
            })?;

            let op_id = OperationId::new(nonce as _);
            let operation = Erc20BridgeOpImpl(Erc20BridgeOp {
                side: BridgeSide::Wrapped,
                stage: Erc20OpStage::SignMintOrder(order),
            });

            let memo = event.memo();

            get_runtime_state()
                .borrow_mut()
                .operations
                .new_operation_with_id(op_id, operation.clone(), memo);

            get_runtime()
                .borrow_mut()
                .schedule_operation(op_id, operation);
        }
        BridgeEvent::Minted(event) => {
            log::trace!("base token minted");

            let Some((_, wrapped_token_sender)) =
                Id256::from_slice(&event.sender_id).and_then(|id| id.to_evm_address().ok())
            else {
                return Err(Error::Serialization(
                    "failed to decode wrapped address from minted event".into(),
                ));
            };

            let Some((op_id, _)) = get_runtime_state()
                .borrow()
                .operations
                .get_for_address_nonce(&wrapped_token_sender, event.nonce)
            else {
                return Err(Error::OperationNotFound(OperationId::new(event.nonce as _)));
            };

            let confirmed = Erc20BridgeOpImpl(Erc20BridgeOp {
                side: BridgeSide::Base,
                stage: Erc20OpStage::TokenMintConfirmed(event),
            });
            get_runtime_state()
                .borrow_mut()
                .operations
                .update(op_id, confirmed);
        }
        BridgeEvent::Notify(_) => {}
    };

    Ok(())
}

fn init_runtime() -> SharedRuntime {
    let runtime = BridgeRuntime::default(ConfigStorage::get());
    let state = runtime.state();
    let base_evm_config = get_base_evm_config();
    let scheduler = runtime.scheduler().clone();

    // Init operation handlers
    let base_handler = Erc20OrderHandler::new(state.clone(), base_evm_config, scheduler.clone());
    let wrapped_config = state.borrow().config.clone();
    let wrapped_handler = Erc20OrderHandler::new(state.clone(), wrapped_config, scheduler.clone());

    // Init mint order signing service
    let base_sign_service = SignMintOrdersService::new(base_handler.clone());
    let wrapped_sign_service = SignMintOrdersService::new(wrapped_handler.clone());
    let sign_service = Erc20ServiceSelector::new(base_sign_service, wrapped_sign_service);

    // Init mint tx service
    let base_mint_tx_service = SendMintTxService::new(base_handler);
    let wrapped_mint_tx_service = SendMintTxService::new(wrapped_handler);
    let send_mint_tx_service =
        Erc20ServiceSelector::new(base_mint_tx_service, wrapped_mint_tx_service);

    let services = state.borrow().services.clone();
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

    Rc::new(RefCell::new(runtime))
}

thread_local! {
    pub static RUNTIME: SharedRuntime = init_runtime();

    pub static BASE_EVM_STATE: SharedBaseEvmState = SharedBaseEvmState::default();
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
