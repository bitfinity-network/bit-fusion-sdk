use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use bridge_did::error::{BTFResult, Error};
use bridge_did::evm_link::EvmLink;
use bridge_did::init::BridgeInitData;
use candid::Principal;
use did::H160;
use ic_canister::{
    Canister, Idl, PreUpdate, generate_exports, generate_idl, query, state_getter, update,
};
use ic_exports::ic_kit::ic;
use ic_log::canister::{LogCanister, LogState};
use ic_storage::IcStorage;
use log::{debug, info};

use crate::inspect;
use crate::memory::{LOG_SETTINGS_MEMORY_ID, memory_by_id};
use crate::runtime::state::config::ConfigStorage;

/// Common API of all bridge canisters.
pub trait BridgeCanister: Canister + LogCanister {
    /// Gets the bridge core state.
    #[state_getter]
    fn config(&self) -> Rc<RefCell<ConfigStorage>>;

    /// Returns principal of canister owner.
    #[query(trait = true)]
    fn get_owner(&self) -> Principal {
        self.config().borrow().get_owner()
    }

    /// Sets a new principal for canister owner.
    ///
    /// This method should be called only by current owner,
    /// else `Error::NotAuthorised` will be returned.
    #[update(trait = true)]
    fn set_owner(&mut self, owner: Principal) {
        inspect::inspect_new_owner_is_valid(owner);
        let core = self.config();
        inspect::inspect_caller_is_owner(core.borrow().get_owner(), ic::caller());
        core.borrow_mut().set_owner(owner);

        info!("Bridge canister owner changed to {owner}");
    }

    /// Returns principal of EVM canister with which the bridge canister works.
    #[query(trait = true)]
    fn get_evm_principal(&self) -> Principal {
        let link = self.config().borrow().get_evm_link();
        match link {
            EvmLink::Ic(principal) => principal,
            _ => ic::trap("expected evm canister link in config"),
        }
    }

    /// Returns bridge contract address for EVM.
    /// If contract isn't initialized yet - returns None.
    #[query(trait = true)]
    fn get_btf_bridge_contract(&mut self) -> Option<H160> {
        self.config().borrow().get_btf_bridge_contract()
    }

    /// Set BTF bridge contract address.
    #[update(trait = true)]
    fn set_btf_bridge_contract(&mut self, address: H160) {
        let config = self.config();
        inspect::inspect_set_btf_bridge_contract(self.config());
        config.borrow_mut().set_btf_bridge_contract(address.clone());

        info!("Bridge canister BTF bridge contract address changed to {address}");
    }

    /// Returns evm_address of the bridge canister.
    #[allow(async_fn_in_trait)]
    #[update(trait = true)]
    async fn get_bridge_canister_evm_address(&mut self) -> BTFResult<H160> {
        let signer = self.config().borrow().get_signer()?;
        signer.get_address().await.map_err(|e| {
            Error::Initialization(format!("failed to get bridge canister address: {e}"))
        })
    }

    /// Initialize the bridge with the given parameters.
    ///
    /// This method should be called only once from the `#[init]` method of the canister.
    ///
    /// `_run_scheduler` callback is called in a timer and should start scheduler task execution
    /// round.
    fn init_bridge(&mut self, init_data: BridgeInitData, _run_scheduler: impl Fn() + 'static) {
        inspect::inspect_new_owner_is_valid(init_data.owner);

        self.config().borrow_mut().init(&init_data);

        if let Some(log_settings) = &init_data.log_settings {
            // Since this code is only run on initialization, we want to fail canister setup if
            // the specified parameters are invalid, so we panic in that case.
            self.log_state()
                .borrow_mut()
                .init(
                    init_data.owner,
                    memory_by_id(LOG_SETTINGS_MEMORY_ID),
                    log_settings.clone(),
                )
                .expect("failed to configure logger");
        }

        #[cfg(target_arch = "wasm32")]
        self.start_timers(_run_scheduler);

        log::trace!("Bridge canister initialized: {init_data:?}");
    }

    /// Re-initializes the bridge after upgrade. This method should be called from the `#[post-upgrade]`
    /// method.
    fn bridge_post_upgrade(&mut self, _run_scheduler: impl Fn() + 'static) {
        if let Err(err) = self
            .log_state()
            .borrow_mut()
            .reload(memory_by_id(LOG_SETTINGS_MEMORY_ID))
        {
            ic_exports::ic_cdk::println!("Error configuring the logger. Err: {err:?}")
        }

        #[cfg(target_arch = "wasm32")]
        self.start_timers(_run_scheduler);

        debug!("Upgrade completed");
    }

    /// Starts scheduler timer.
    fn start_timers(&mut self, run_scheduler: impl Fn() + 'static) {
        const GLOBAL_TIMER_INTERVAL: Duration = Duration::from_secs(2);
        ic_exports::ic_cdk_timers::set_timer_interval(GLOBAL_TIMER_INTERVAL, move || {
            run_scheduler();
        });
    }

    /// Returns IDL of the bridge API.
    fn get_idl() -> Idl {
        let mut idl = generate_idl!();
        let logger_idl = <Self as LogCanister>::get_idl();
        idl.merge(&logger_idl);

        idl
    }
}

generate_exports!(BridgeCanister, BridgeCanisterExport);

impl LogCanister for BridgeCanisterExport {
    fn log_state(&self) -> Rc<RefCell<LogState>> {
        LogState::get()
    }
}

#[cfg(test)]
mod tests {
    use bridge_did::evm_link::EvmLink;
    use eth_signer::sign_strategy::SigningStrategy;
    use ic_canister::{canister_call, init};
    use ic_exports::ic_kit::{MockContext, inject};
    use ic_storage::IcStorage;

    use super::*;

    #[derive(Debug, Canister)]
    struct TestBridge {
        #[id]
        id: Principal,
    }

    impl TestBridge {
        #[init]
        fn init(&mut self, init_data: BridgeInitData) {
            self.init_bridge(init_data, || {});
        }
    }

    impl BridgeCanister for TestBridge {
        fn config(&self) -> Rc<RefCell<ConfigStorage>> {
            ConfigStorage::get()
        }
    }

    impl PreUpdate for TestBridge {}

    impl LogCanister for TestBridge {
        fn log_state(&self) -> Rc<RefCell<LogState>> {
            LogState::get()
        }
    }

    fn owner() -> Principal {
        Principal::from_slice(&[1; 20])
    }

    fn bob() -> Principal {
        Principal::from_slice(&[2; 20])
    }

    async fn init_canister() -> TestBridge {
        let init_data = BridgeInitData {
            owner: owner(),
            evm_link: EvmLink::Ic(bob()),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            log_settings: None,
        };
        init_with_data(init_data).await
    }

    async fn init_with_data(init_data: BridgeInitData) -> TestBridge {
        MockContext::new().inject();

        const MOCK_PRINCIPAL: &str = "mfufu-x6j4c-gomzb-geilq";
        let mock_canister_id = Principal::from_text(MOCK_PRINCIPAL).expect("valid principal");
        let mut canister = TestBridge::from_principal(mock_canister_id);

        canister_call!(canister.init(init_data), ()).await.unwrap();
        canister
    }

    #[tokio::test]
    #[should_panic(expected = "unexpected anonymous evm principal")]
    async fn init_rejects_anonymous_evm() {
        let init_data = BridgeInitData {
            owner: owner(),
            evm_link: EvmLink::Ic(Principal::anonymous()),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            log_settings: None,
        };
        let _ = init_with_data(init_data).await;
    }

    #[tokio::test]
    #[should_panic(expected = "unexpected management canister as evm principal")]
    async fn init_rejects_management_evm() {
        let init_data = BridgeInitData {
            owner: owner(),
            evm_link: EvmLink::Ic(Principal::management_canister()),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            log_settings: None,
        };
        let _ = init_with_data(init_data).await;
    }

    #[tokio::test]
    #[should_panic(expected = "Owner cannot be an anonymous")]
    async fn init_rejects_anonymous_owner() {
        let init_data = BridgeInitData {
            owner: Principal::anonymous(),
            evm_link: EvmLink::Ic(bob()),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            log_settings: None,
        };
        let _ = init_with_data(init_data).await;
    }

    #[tokio::test]
    async fn set_owner_changes_owner() {
        let mut canister = init_canister().await;

        inject::get_context().update_id(owner());
        canister_call!(canister.set_owner(bob()), ()).await.unwrap();

        // check if state updated
        let stored_owner = canister_call!(canister.get_owner(), Principal)
            .await
            .unwrap();
        assert_eq!(stored_owner, bob());
    }

    #[tokio::test]
    #[should_panic(expected = "Running this method is only allowed for the owner of the canister")]
    async fn set_owner_rejected_for_non_owner() {
        let mut canister = init_canister().await;
        let _ = canister_call!(canister.set_owner(bob()), ()).await;
    }

    #[tokio::test]
    #[should_panic(expected = "Owner cannot be an anonymous")]
    async fn set_owner_rejects_anonymous() {
        let mut canister = init_canister().await;

        inject::get_context().update_id(owner());

        let _ = canister_call!(canister.set_owner(Principal::anonymous()), ()).await;
    }

    #[tokio::test]
    async fn set_btf_bridge_works() {
        let mut canister = init_canister().await;

        inject::get_context().update_id(owner());
        let address = H160::from_slice(&[42; 20]);
        let _ = canister_call!(canister.set_btf_bridge_contract(address.clone()), ()).await;

        // check if state updated
        let stored_btf = canister_call!(canister.get_btf_bridge_contract(), Option<H160>)
            .await
            .unwrap();
        assert_eq!(stored_btf, Some(address));
    }

    #[tokio::test]
    #[should_panic(expected = "Running this method is only allowed for the owner of the canister")]
    async fn set_btf_bridge_rejected_for_non_owner() {
        let mut canister = init_canister().await;

        let address = H160::from_slice(&[42; 20]);
        let _ = canister_call!(canister.set_btf_bridge_contract(address), ()).await;
    }
}
