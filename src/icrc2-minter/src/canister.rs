use std::cell::RefCell;
use std::rc::Rc;

use bridge_canister::runtime::state::config::ConfigStorage;
use bridge_canister::runtime::state::SharedConfig;
use bridge_canister::runtime::{BridgeRuntime, RuntimeState};
use bridge_canister::BridgeCanister;
use bridge_did::error::{BftResult, Error};
use bridge_did::init::BridgeInitData;
use bridge_did::op_id::OperationId;
use bridge_utils::common::Pagination;
use candid::Principal;
use did::build::BuildData;
use did::H160;
use ic_canister::{
    generate_idl, init, post_upgrade, query, update, Canister, Idl, MethodType, PreUpdate,
};
use ic_exports::ic_kit::ic;
use ic_log::canister::{LogCanister, LogState};
use ic_metrics::{Metrics, MetricsStorage};
use ic_storage::IcStorage;

use crate::ops::IcrcBridgeOp;
use crate::state::IcrcState;

#[cfg(feature = "export-api")]
mod inspect;

pub type SharedRuntime = Rc<RefCell<BridgeRuntime<IcrcBridgeOp>>>;

/// A canister to transfer funds between IC token canisters and EVM canister contracts.
#[derive(Canister, Clone)]
pub struct MinterCanister {
    #[id]
    id: Principal,
}

impl PreUpdate for MinterCanister {
    fn pre_update(&self, _method_name: &str, _method_type: MethodType) {}
}

impl BridgeCanister for MinterCanister {
    fn config(&self) -> SharedConfig {
        ConfigStorage::get()
    }
}

impl MinterCanister {
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

    #[query]
    /// Returns the list of operations for the given wallet address.
    /// Offset, if set, defines the starting index of the page,
    /// Count, if set, defines the number of elements in the page.
    pub fn get_operations_list(
        &self,
        wallet_address: H160,
        pagination: Option<Pagination>,
    ) -> Vec<(OperationId, IcrcBridgeOp)> {
        get_runtime_state()
            .borrow()
            .operations
            .get_for_address(&wallet_address, pagination)
    }

    /// Adds the provided principal to the whitelist.
    #[update]
    pub fn add_to_whitelist(&mut self, icrc2_principal: Principal) -> BftResult<()> {
        let state = get_icrc_state();

        Self::access_control_inspect_message_check(ic::caller(), icrc2_principal)?;

        let mut state = state.borrow_mut();

        state.access_list.add(icrc2_principal)?;

        Ok(())
    }

    /// Remove a icrc2 principal token from the access list
    #[update]
    pub fn remove_from_whitelist(&mut self, icrc2_principal: Principal) -> BftResult<()> {
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
    ) -> BftResult<()> {
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

impl LogCanister for MinterCanister {
    fn log_state(&self) -> Rc<RefCell<LogState>> {
        LogState::get()
    }
}

impl Metrics for MinterCanister {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        MetricsStorage::get()
    }
}

/// inspect function to check whether provided principal is owner
fn inspect_check_is_owner(principal: Principal) -> BftResult<()> {
    let owner = ConfigStorage::get().borrow().get_owner();

    if owner != principal {
        return Err(Error::AccessDenied);
    }

    Ok(())
}

/// inspect function to check whether the provided principal is anonymous
fn check_anonymous_principal(principal: Principal) -> BftResult<()> {
    if principal == Principal::anonymous() {
        return Err(Error::AnonymousPrincipal);
    }

    Ok(())
}

thread_local! {
    pub static RUNTIME: SharedRuntime =
        Rc::new(RefCell::new(BridgeRuntime::default(ConfigStorage::get())));

    pub static ICRC_STATE: Rc<RefCell<IcrcState>> = Rc::default();
}

pub fn get_runtime() -> SharedRuntime {
    RUNTIME.with(|r| r.clone())
}

pub fn get_runtime_state() -> RuntimeState<IcrcBridgeOp> {
    get_runtime().borrow().state().clone()
}

pub fn get_icrc_state() -> Rc<RefCell<IcrcState>> {
    ICRC_STATE.with(|s| s.clone())
}

#[cfg(test)]
mod test {
    use candid::Principal;
    use eth_signer::sign_strategy::SigningStrategy;
    use ic_canister::{canister_call, Canister};
    use ic_exports::ic_kit::{inject, MockContext};

    use super::*;
    use crate::MinterCanister;

    fn owner() -> Principal {
        Principal::from_slice(&[1; 20])
    }

    async fn init_canister() -> MinterCanister {
        MockContext::new().inject();

        const MOCK_PRINCIPAL: &str = "mfufu-x6j4c-gomzb-geilq";
        let mock_canister_id = Principal::from_text(MOCK_PRINCIPAL).expect("valid principal");
        let mut canister = MinterCanister::from_principal(mock_canister_id);

        let init_data = BridgeInitData {
            owner: owner(),
            evm_principal: Principal::from_slice(&[2; 20]),
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
