use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use bridge_did::error::{Error, Result};
use bridge_did::init::BridgeInitData;
use candid::Principal;
use did::H160;
use eth_signer::sign_strategy::TransactionSigner;
use ic_canister::{
    generate_exports, generate_idl, query, state_getter, update, Canister, Idl, PreUpdate,
};
use ic_log::writer::Logs;
use ic_log::LogSettings;
use ic_task_scheduler::task::TaskOptions;
use log::{debug, info};

use crate::log_config::LoggerConfigService;
use crate::BridgeCore;

/// Common API of all bridge canisters.
pub trait BridgeCanister: Canister {
    /// Gets the bridge core state.
    #[state_getter]
    fn core(&self) -> Rc<RefCell<BridgeCore>>;

    /// Gets the logs
    /// - `count` is the number of logs to return
    #[query(trait = true)]
    fn ic_logs(&self, count: usize, offset: usize) -> Result<Logs> {
        self.core().borrow().inspect_ic_logs();
        Ok(ic_log::take_memory_records(count, offset))
    }

    /// Updates the runtime configuration of the logger with a new filter in the same form as the `RUST_LOG`
    /// environment variable.
    /// Example of valid filters:
    /// - info
    /// - debug,crate1::mod1=error,crate1::mod2,crate2=debug
    #[update(trait = true)]
    fn set_logger_filter(&mut self, filter: String) -> Result<()> {
        self.core().borrow().inspect_set_logger_filter();
        LoggerConfigService.set_logger_filter(&filter)?;

        debug!("updated logger filter to {filter}");

        Ok(())
    }

    #[query(trait = true)]
    fn get_logger_settings(&self) -> LogSettings {
        LoggerConfigService.current_settings()
    }

    /// Returns principal of canister owner.
    #[query(trait = true)]
    fn get_owner(&self) -> Principal {
        self.core().borrow().config.get_owner()
    }

    /// Sets a new principal for canister owner.
    ///
    /// This method should be called only by current owner,
    /// else `Error::NotAuthorised` will be returned.
    #[update(trait = true)]
    fn set_owner(&mut self, owner: Principal) {
        let core = self.core();
        core.borrow_mut().set_owner(owner);

        info!("Bridge canister owner changed to {owner}");
    }

    /// Returns principal of EVM canister with which the minter canister works.
    #[query(trait = true)]
    fn get_evm_principal(&self) -> Principal {
        self.core().borrow().config.get_evm_principal()
    }

    /// Sets principal of EVM canister with which the minter canister works.
    ///
    /// This method should be called only by current owner,
    /// else `Error::NotAuthorised` will be returned.
    #[update(trait = true)]
    fn set_evm_principal(&mut self, evm: Principal) {
        let core = self.core();
        core.borrow().inspect_set_evm_principal();
        core.borrow_mut().config.set_evm_principal(evm);

        info!("Bridge canister EVM principal changed to {evm}");
    }

    /// Returns bridge contract address for EVM.
    /// If contract isn't initialized yet - returns None.
    #[query(trait = true)]
    fn get_bft_bridge_contract(&mut self) -> Option<H160> {
        self.core().borrow().config.get_bft_bridge_contract()
    }

    /// Set BFT bridge contract address.
    #[update(trait = true)]
    fn set_bft_bridge_contract(&mut self, address: H160) {
        let core = self.core();
        core.borrow().inspect_set_bft_bridge_contract();
        core.borrow_mut()
            .config
            .set_bft_bridge_contract(Some(address.clone()));

        info!("Bridge canister BFT bridge contract address changed to {address}");
    }

    /// Returns evm_address of the minter canister.
    #[allow(async_fn_in_trait)]
    #[update(trait = true)]
    async fn get_bridge_canister_evm_address(&mut self) -> Result<H160> {
        let signer = self.core().borrow().get_transaction_signer();
        signer
            .get_address()
            .await
            .map_err(|e| Error::Internal(format!("failed to get minter canister address: {e}")))
    }

    /// Initialize the bridge with the given parameters.
    ///
    /// This method should be called only once from the `#[init]` method of the canister.
    ///
    /// `_run_scheduler` callback is called in a timer and should start scheduler task execution
    /// round.
    fn init_bridge(
        &mut self,
        init_data: BridgeInitData,
        _run_scheduler: impl Fn(TaskOptions) + 'static,
    ) {
        self.core().borrow_mut().init(&init_data);

        if let Some(log_settings) = &init_data.log_settings {
            // Since this code is only run on initialization, we want to fail canister setup if
            // the specified parameters are invalid, so we panic in that case.
            LoggerConfigService
                .init(log_settings.clone())
                .expect("Failed to configure logger.");
        }

        #[cfg(target_arch = "wasm32")]
        self.start_timers(_run_scheduler);

        log::trace!("Bridge canister initialized: {init_data:?}");
    }

    /// Re-initializes the bridge after upgrade. This method should be called from the `#[post-upgrade]`
    /// method.
    fn bridge_post_upgrade(&mut self, _run_scheduler: impl Fn(TaskOptions) + 'static) {
        self.core().borrow_mut().reload();

        if let Err(err) = LoggerConfigService.reload() {
            ic_exports::ic_cdk::println!("Error configuring the logger. Err: {err:?}")
        }

        #[cfg(target_arch = "wasm32")]
        self.start_timers(_run_scheduler);

        debug!("Upgrade completed");
    }

    /// Starts scheduler timer.
    fn start_timers(&mut self, run_scheduler: impl Fn(TaskOptions) + 'static) {
        const GLOBAL_TIMER_INTERVAL: Duration = Duration::from_secs(2);
        ic_exports::ic_cdk_timers::set_timer_interval(GLOBAL_TIMER_INTERVAL, move || {
            let options = TaskOptions::default();
            run_scheduler(options);
        });
    }

    /// Returns IDL of the bridge API.
    fn get_idl() -> Idl {
        generate_idl!()
    }
}

generate_exports!(BridgeCanister);

#[cfg(test)]
#[cfg_attr(coverage_nightly, coverage(off))]
mod tests {
    use eth_signer::sign_strategy::SigningStrategy;
    use ic_canister::{canister_call, init};
    use ic_exports::ic_kit::{inject, MockContext};
    use ic_log::LogSettings;
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
            self.init_bridge(init_data, |_| {});
        }
    }

    impl BridgeCanister for TestBridge {
        fn core(&self) -> Rc<RefCell<BridgeCore>> {
            BridgeCore::get()
        }
    }
    impl PreUpdate for TestBridge {}

    fn owner() -> Principal {
        Principal::from_slice(&[1; 20])
    }

    fn bob() -> Principal {
        Principal::from_slice(&[2; 20])
    }

    async fn init_canister() -> TestBridge {
        let init_data = BridgeInitData {
            owner: owner(),
            evm_principal: bob(),
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
    async fn init_sets_config() {
        let owner = owner();
        let evm_principal = bob();
        let signing_strategy = SigningStrategy::Local {
            private_key: [42; 32],
        };
        let log_settings = LogSettings {
            enable_console: true,
            in_memory_records: Some(10),
            log_filter: Some("error".into()),
        };

        let canister = init_with_data(BridgeInitData {
            owner,
            evm_principal,
            signing_strategy: signing_strategy.clone(),
            log_settings: Some(log_settings.clone()),
        })
        .await;

        let stored_owner = canister_call!(canister.get_owner(), Principal)
            .await
            .unwrap();
        assert_eq!(stored_owner, owner);

        let stored_evm_principal = canister_call!(canister.get_evm_principal(), Principal)
            .await
            .unwrap();
        assert_eq!(stored_evm_principal, evm_principal);

        let stored_signing_strategy = canister
            .core()
            .borrow()
            .config
            .get_signing_strategy()
            .clone();
        assert_eq!(stored_signing_strategy, signing_strategy);

        let stored_log_settings = canister_call!(canister.get_logger_settings(), LogSettings)
            .await
            .unwrap();
        assert_eq!(
            stored_log_settings.enable_console,
            log_settings.enable_console
        );
        assert_eq!(
            stored_log_settings.in_memory_records,
            log_settings.in_memory_records
        );
        assert_eq!(stored_log_settings.log_filter, log_settings.log_filter);
    }

    #[tokio::test]
    #[should_panic(expected = "Evm principal cannot be an anonymous")]
    async fn init_rejects_anonymous_evm() {
        let init_data = BridgeInitData {
            owner: owner(),
            evm_principal: Principal::anonymous(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            log_settings: None,
        };
        let _ = init_with_data(init_data).await;
    }

    #[tokio::test]
    #[should_panic(expected = "Evm principal cannot be management canister")]
    async fn init_rejects_management_evm() {
        let init_data = BridgeInitData {
            owner: owner(),
            evm_principal: Principal::management_canister(),
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
            evm_principal: bob(),
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
    async fn set_evm_principal_works() {
        let mut canister = init_canister().await;

        inject::get_context().update_id(owner());
        let _ = canister_call!(canister.set_evm_principal(bob()), ()).await;

        // check if state updated
        let stored_evm = canister_call!(canister.get_evm_principal(), Principal)
            .await
            .unwrap();
        assert_eq!(stored_evm, bob());
    }

    #[tokio::test]
    #[should_panic(expected = "Running this method is only allowed for the owner of the canister")]
    async fn set_evm_principal_rejected_for_non_owner() {
        let mut canister = init_canister().await;

        let _ = canister_call!(canister.set_evm_principal(bob()), ()).await;
    }

    #[tokio::test]
    async fn set_bft_bridge_works() {
        let mut canister = init_canister().await;

        inject::get_context().update_id(owner());
        let address = H160::from_slice(&[42; 20]);
        let _ = canister_call!(canister.set_bft_bridge_contract(address.clone()), ()).await;

        // check if state updated
        let stored_bft = canister_call!(canister.get_bft_bridge_contract(), Option<H160>)
            .await
            .unwrap();
        assert_eq!(stored_bft, Some(address));
    }

    #[tokio::test]
    #[should_panic(expected = "Running this method is only allowed for the owner of the canister")]
    async fn set_bft_bridge_rejected_for_non_owner() {
        let mut canister = init_canister().await;

        let address = H160::from_slice(&[42; 20]);
        let _ = canister_call!(canister.set_bft_bridge_contract(address), ()).await;
    }

    #[tokio::test]
    async fn test_set_logger_filter() {
        let init_data = BridgeInitData {
            owner: owner(),
            evm_principal: bob(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            log_settings: Some(LogSettings {
                enable_console: false,
                in_memory_records: Some(1000),
                log_filter: Some("error".into()),
            }),
        };
        let mut canister = init_with_data(init_data).await;
        inject::get_context().update_id(owner());
        let new_filter = "info";
        let res = canister_call!(canister.set_logger_filter(new_filter.to_string()), ()).await;
        assert!(res.is_ok());

        let updated_settings = canister_call!(canister.get_logger_settings(), LogSettings)
            .await
            .unwrap();
        assert_eq!(updated_settings.log_filter, Some(new_filter.into()));
    }

    #[tokio::test]
    #[should_panic(expected = "Running this method is only allowed for the owner of the canister")]
    async fn set_log_filter_is_rejected_for_non_owner() {
        let mut canister = init_canister().await;

        let _ = canister_call!(canister.set_logger_filter("info".into()), ()).await;
    }

    #[tokio::test]
    async fn test_get_bridge_canister_evm_address() {
        let mut canister = init_canister().await;
        inject::get_context().update_id(owner());

        let evm_address = canister_call!(canister.get_bridge_canister_evm_address(), Result<H160>)
            .await
            .unwrap();

        assert!(evm_address.is_ok());
        assert_ne!(evm_address.unwrap(), H160::default());
    }

    #[tokio::test]
    #[should_panic(expected = "Running this method is only allowed for the owner of the canister")]
    async fn ic_logs_is_rejected_for_non_owner() {
        let mut canister = init_canister().await;

        let _ = canister_call!(canister.ic_logs(1000, 0), ()).await;
    }
}
