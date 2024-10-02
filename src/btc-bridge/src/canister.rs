mod inspect;

use std::cell::RefCell;
use std::rc::Rc;

use bridge_canister::runtime::service::mint_tx::SendMintTxService;
use bridge_canister::runtime::service::sign_orders::SignMintOrdersService;
use bridge_canister::runtime::service::ServiceOrder;
use bridge_canister::runtime::state::config::ConfigStorage;
use bridge_canister::runtime::state::SharedConfig;
use bridge_canister::runtime::{BridgeRuntime, RuntimeState};
use bridge_canister::BridgeCanister;
use bridge_did::error::BftResult;
use bridge_did::init::BtcBridgeConfig;
use bridge_did::op_id::OperationId;
use bridge_did::operation_log::Memo;
use candid::Principal;
use did::build::BuildData;
use did::H160;
use ic_canister::{
    generate_idl, init, post_upgrade, query, update, virtual_canister_call, Canister, Idl,
    PreUpdate,
};
use ic_ckbtc_minter::updates::get_btc_address::GetBtcAddressArgs;
use ic_exports::ic_cdk;
use ic_exports::ledger::Subaccount;
use ic_log::canister::{LogCanister, LogState};
use ic_metrics::{Metrics, MetricsStorage};
use ic_storage::IcStorage;

use crate::ops::{
    BtcBridgeOpImpl, BtcMintOrderHandler, BtcMintTxHandler, SEND_MINT_TX_SERVICE_ID,
    SIGN_MINT_ORDER_SERVICE_ID,
};
use crate::state::{State, WrappedTokenConfig};

type SharedRuntime = Rc<RefCell<BridgeRuntime<BtcBridgeOpImpl>>>;

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
    pub fn init(&mut self, config: BtcBridgeConfig) {
        let BtcBridgeConfig { network, init_data } = config;
        get_state().borrow_mut().configure_btc(network);
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

    /// Returns operation by memo
    #[query]
    pub fn get_operation_by_memo_and_user(
        &self,
        memo: Memo,
        user_id: H160,
    ) -> Option<(OperationId, BtcBridgeOpImpl)> {
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

    #[update]
    pub async fn get_btc_address(&self, args: GetBtcAddressArgs) -> String {
        let ck_btc_minter = get_state().borrow().ck_btc_minter();
        virtual_canister_call!(ck_btc_minter, "get_btc_address", (args,), String)
            .await
            .unwrap()
    }

    #[update]
    pub fn admin_configure_wrapped_token(&self, config: WrappedTokenConfig) -> BftResult<()> {
        Self::inspect_caller_is_owner()?;

        get_state().borrow_mut().configure_wrapped_token(config);

        Ok(())
    }

    /// Returns the build data of the canister
    #[query]
    fn get_canister_build_data(&self) -> BuildData {
        bridge_canister::build_data!()
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }

    pub fn inspect_caller_is_owner() -> BftResult<()> {
        let owner = ConfigStorage::get().borrow().get_owner();

        if ic_cdk::caller() == owner {
            Ok(())
        } else {
            Err(bridge_did::error::Error::AccessDenied)
        }
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

fn init_runtime() -> SharedRuntime {
    let runtime = BridgeRuntime::default(ConfigStorage::get());
    let state = runtime.state();

    let sign_orders_handler = BtcMintOrderHandler::new(state.clone(), runtime.scheduler().clone());
    let sign_mint_orders_service = Rc::new(SignMintOrdersService::new(sign_orders_handler));

    let mint_tx_handler = BtcMintTxHandler::new(state.clone());
    let mint_tx_service = Rc::new(SendMintTxService::new(mint_tx_handler));

    let services = state.borrow().services.clone();
    services.borrow_mut().add_service(
        ServiceOrder::ConcurrentWithOperations,
        SIGN_MINT_ORDER_SERVICE_ID,
        sign_mint_orders_service,
    );
    services.borrow_mut().add_service(
        ServiceOrder::ConcurrentWithOperations,
        SEND_MINT_TX_SERVICE_ID,
        mint_tx_service,
    );

    Rc::new(RefCell::new(runtime))
}

thread_local! {
    pub static STATE: Rc<RefCell<State>> = Rc::default();

    pub static RUNTIME: SharedRuntime = init_runtime();
}

pub fn get_state() -> Rc<RefCell<State>> {
    STATE.with(|state| state.clone())
}

pub fn get_runtime() -> SharedRuntime {
    RUNTIME.with(|r| r.clone())
}

pub fn get_runtime_state() -> RuntimeState<BtcBridgeOpImpl> {
    get_runtime().borrow().state().clone()
}

#[cfg(test)]
mod test {
    use bridge_did::evm_link::EvmLink;
    use bridge_did::init::{BitcoinConnection, BridgeInitData};
    use candid::Principal;
    use eth_signer::sign_strategy::SigningStrategy;
    use ic_canister::{canister_call, Canister};
    use ic_exports::ic_kit::MockContext;

    use super::*;
    use crate::BtcBridge;

    fn owner() -> Principal {
        Principal::from_slice(&[1; 20])
    }

    async fn init_canister() -> BtcBridge {
        MockContext::new().inject();

        const MOCK_PRINCIPAL: &str = "mfufu-x6j4c-gomzb-geilq";
        let mock_canister_id = Principal::from_text(MOCK_PRINCIPAL).expect("valid principal");
        let mut canister = BtcBridge::from_principal(mock_canister_id);

        let init_data = BridgeInitData {
            owner: owner(),
            evm_link: EvmLink::Ic(Principal::from_slice(&[2; 20])),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            log_settings: None,
        };
        let config = BtcBridgeConfig {
            network: BitcoinConnection::Mainnet,
            init_data,
        };
        canister_call!(canister.init(config), ()).await.unwrap();
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
}
