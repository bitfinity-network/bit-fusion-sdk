use std::cell::RefCell;
use std::rc::Rc;

use bridge_canister::BridgeCanister;
use bridge_canister::runtime::service::ServiceOrder;
use bridge_canister::runtime::service::fetch_logs::FetchBtfBridgeEventsService;
use bridge_canister::runtime::service::mint_tx::SendMintTxService;
use bridge_canister::runtime::service::sign_orders::SignMintOrdersService;
use bridge_canister::runtime::service::update_evm_params::RefreshEvmParamsService;
use bridge_canister::runtime::state::SharedConfig;
use bridge_canister::runtime::state::config::ConfigStorage;
use bridge_canister::runtime::{BridgeRuntime, RuntimeState};
use bridge_did::error::{BTFResult, Error};
use bridge_did::init::BridgeInitData;
use bridge_did::op_id::OperationId;
use bridge_did::operation_log::{Memo, OperationLog};
use bridge_did::operations::IcrcBridgeOp;
use bridge_utils::common::Pagination;
use candid::Principal;
use did::H160;
use did::build::BuildData;
use ic_canister::{
    Canister, Idl, MethodType, PreUpdate, generate_idl, init, post_upgrade, query, update,
};
use ic_exports::ic_kit::ic;
use ic_log::canister::{LogCanister, LogState};
use ic_metrics::{Metrics, MetricsStorage};
use ic_storage::IcStorage;

use crate::ops::events_handler::IcrcEventsHandler;
use crate::ops::{
    FETCH_BTF_EVENTS_SERVICE_ID, IcrcBridgeOpImpl, IcrcMintOrderHandler, IcrcMintTxHandler,
    REFRESH_PARAMS_SERVICE_ID, SEND_MINT_TX_SERVICE_ID, SIGN_MINT_ORDER_SERVICE_ID,
};
use crate::state::IcrcState;

#[cfg(feature = "export-api")]
mod inspect;

pub type SharedRuntime = Rc<RefCell<BridgeRuntime<IcrcBridgeOpImpl>>>;

/// A canister to transfer funds between IC token canisters and EVM canister contracts.
#[derive(Canister, Clone)]
pub struct Icrc2BridgeCanister {
    #[id]
    id: Principal,
}

impl PreUpdate for Icrc2BridgeCanister {
    fn pre_update(&self, _method_name: &str, _method_type: MethodType) {}
}

impl BridgeCanister for Icrc2BridgeCanister {
    fn config(&self) -> SharedConfig {
        ConfigStorage::get()
    }
}

impl Icrc2BridgeCanister {
    /// Initialize the canister with given data.
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
    ) -> Vec<(OperationId, IcrcBridgeOpImpl)> {
        get_runtime_state().borrow().operations.get_for_address(
            &wallet_address,
            min_included_id,
            pagination,
        )
    }

    #[query]
    /// Returns operation by memo
    pub fn get_operation_by_memo_and_user(
        &self,
        memo: Memo,
        user_id: H160,
    ) -> Option<(OperationId, IcrcBridgeOp)> {
        get_runtime_state()
            .borrow()
            .operations
            .get_operation_by_memo_and_user(&memo, &user_id)
            .map(|op| (op.0, op.1.0))
    }

    /// Returns log of an operation by its ID.
    #[query]
    pub fn get_operation_log(
        &self,
        operation_id: OperationId,
    ) -> Option<OperationLog<IcrcBridgeOpImpl>> {
        get_runtime_state()
            .borrow()
            .operations
            .get_log(operation_id)
    }

    /// Returns all memos for a given user_id.
    #[query]
    pub fn get_memos_by_user_address(&self, user_id: H160) -> Vec<Memo> {
        get_runtime_state()
            .borrow()
            .operations
            .get_memos_by_user_address(&user_id)
    }

    /// Adds the provided principal to the whitelist.
    #[update]
    pub fn add_to_whitelist(&mut self, icrc2_principal: Principal) -> BTFResult<()> {
        let state = get_icrc_state();

        Self::access_control_inspect_message_check(ic::caller(), icrc2_principal)?;

        let mut state = state.borrow_mut();

        state.access_list.add(icrc2_principal)?;

        Ok(())
    }

    /// Remove a icrc2 principal token from the access list
    #[update]
    pub fn remove_from_whitelist(&mut self, icrc2_principal: Principal) -> BTFResult<()> {
        let state = get_icrc_state();

        Self::access_control_inspect_message_check(ic::caller(), icrc2_principal)?;

        let mut state = state.borrow_mut();

        state.access_list.remove(&icrc2_principal);

        Ok(())
    }

    /// Returns the list of all principals in the whitelist.
    #[query]
    fn get_whitelist(&self) -> Vec<Principal> {
        get_icrc_state().borrow().access_list.get_all_principals()
    }

    fn access_control_inspect_message_check(
        owner: Principal,
        icrc2_principal: Principal,
    ) -> BTFResult<()> {
        inspect_check_is_owner(owner)?;
        check_anonymous_principal(icrc2_principal)?;

        Ok(())
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

impl LogCanister for Icrc2BridgeCanister {
    fn log_state(&self) -> Rc<RefCell<LogState>> {
        LogState::get()
    }
}

impl Metrics for Icrc2BridgeCanister {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        MetricsStorage::get()
    }
}

/// inspect function to check whether provided principal is owner
fn inspect_check_is_owner(principal: Principal) -> BTFResult<()> {
    let owner = ConfigStorage::get().borrow().get_owner();

    if owner != principal {
        return Err(Error::AccessDenied);
    }

    Ok(())
}

/// inspect function to check whether the provided principal is anonymous
fn check_anonymous_principal(principal: Principal) -> BTFResult<()> {
    if principal == Principal::anonymous() {
        return Err(Error::AnonymousPrincipal);
    }

    Ok(())
}

fn init_runtime() -> SharedRuntime {
    let runtime = BridgeRuntime::default(ConfigStorage::get());
    let state = runtime.state().clone();
    let scheduler = runtime.scheduler().clone();
    let runtime = Rc::new(RefCell::new(runtime));
    let config = state.borrow().config.clone();

    let refresh_params_service = RefreshEvmParamsService::new(config.clone());

    let fetch_btf_events_service =
        FetchBtfBridgeEventsService::new(IcrcEventsHandler, runtime.clone(), config);

    let sign_orders_handler = IcrcMintOrderHandler::new(state.clone(), scheduler);
    let sign_mint_orders_service = SignMintOrdersService::new(sign_orders_handler);

    let mint_tx_handler = IcrcMintTxHandler::new(state.clone());
    let mint_tx_service = SendMintTxService::new(mint_tx_handler);

    let services = state.borrow().services.clone();
    services.borrow_mut().add_service(
        ServiceOrder::BeforeOperations,
        REFRESH_PARAMS_SERVICE_ID,
        Rc::new(refresh_params_service),
    );
    services.borrow_mut().add_service(
        ServiceOrder::BeforeOperations,
        FETCH_BTF_EVENTS_SERVICE_ID,
        Rc::new(fetch_btf_events_service),
    );
    services.borrow_mut().add_service(
        ServiceOrder::ConcurrentWithOperations,
        SIGN_MINT_ORDER_SERVICE_ID,
        Rc::new(sign_mint_orders_service),
    );
    services.borrow_mut().add_service(
        ServiceOrder::ConcurrentWithOperations,
        SEND_MINT_TX_SERVICE_ID,
        Rc::new(mint_tx_service),
    );

    runtime
}

thread_local! {
    pub static RUNTIME: SharedRuntime = init_runtime();

    pub static ICRC_STATE: Rc<RefCell<IcrcState>> = Rc::default();
}

pub fn get_runtime() -> SharedRuntime {
    RUNTIME.with(|r| r.clone())
}

pub fn get_runtime_state() -> RuntimeState<IcrcBridgeOpImpl> {
    get_runtime().borrow().state().clone()
}

pub fn get_icrc_state() -> Rc<RefCell<IcrcState>> {
    ICRC_STATE.with(|s| s.clone())
}

#[cfg(test)]
mod test {
    use bridge_did::evm_link::EvmLink;
    use candid::Principal;
    use eth_signer::sign_strategy::SigningStrategy;
    use ic_canister::{Canister, canister_call};
    use ic_exports::ic_kit::{MockContext, inject};

    use super::*;
    use crate::Icrc2BridgeCanister;

    fn owner() -> Principal {
        Principal::from_slice(&[1; 20])
    }

    async fn init_canister() -> Icrc2BridgeCanister {
        MockContext::new().inject();

        const MOCK_PRINCIPAL: &str = "mfufu-x6j4c-gomzb-geilq";
        let mock_canister_id = Principal::from_text(MOCK_PRINCIPAL).expect("valid principal");
        let mut canister = Icrc2BridgeCanister::from_principal(mock_canister_id);

        let init_data = BridgeInitData {
            owner: owner(),
            evm_link: EvmLink::Ic(Principal::from_slice(&[2; 20])),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            log_settings: None,
        };
        canister_call!(canister.init(init_data), ()).await.unwrap();
        canister
    }

    #[tokio::test]
    async fn correct_initialization() {
        let canister = init_canister().await;

        let stored_owner = canister_call!(canister.get_owner(), Principal)
            .await
            .unwrap();
        assert_eq!(stored_owner, owner());

        let stored_evm = canister_call!(canister.get_evm_principal(), Principal)
            .await
            .unwrap();
        assert_eq!(stored_evm, Principal::from_slice(&[2; 20]));
    }

    #[tokio::test]
    async fn test_access_list() {
        let mut canister = init_canister().await;

        let icrc2_principal = Principal::from_text("2chl6-4hpzw-vqaaa-aaaaa-c").unwrap();

        // Add to whitelist
        inject::get_context().update_id(owner());
        canister_call!(canister.add_to_whitelist(icrc2_principal), Result<()>)
            .await
            .unwrap()
            .unwrap();

        // Check if the principal is in the whitelist
        let whitelist = canister_call!(canister.get_whitelist(), Vec<Principal>)
            .await
            .unwrap();
        assert_eq!(whitelist, vec![icrc2_principal]);

        // Remove from whitelist
        canister_call!(canister.remove_from_whitelist(icrc2_principal), Result<()>)
            .await
            .unwrap()
            .unwrap();

        // Check if the principal is removed from the whitelist
        let whitelist = canister_call!(canister.get_whitelist(), Vec<Principal>)
            .await
            .unwrap();

        assert!(whitelist.is_empty());
    }
}
