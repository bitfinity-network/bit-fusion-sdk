use std::cell::RefCell;
use std::rc::Rc;

use candid::Principal;
use did::build::BuildData;
use did::{H160, U256};
use eth_signer::sign_strategy::TransactionSigner;
use ethereum_json_rpc_client::{Client, EthJsonRpcClient};
use ic_canister::{
    generate_idl, init, post_upgrade, query, update, Canister, Idl, MethodType, PreUpdate,
};
use ic_exports::ic_kit::ic;
use ic_exports::icrc_types::icrc1::account::Account;
use ic_log::writer::Logs;
use ic_metrics::{Metrics, MetricsStorage};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{StableUnboundedMap, VirtualMemory};
use ic_task_scheduler::retry::BackoffPolicy;
use ic_task_scheduler::scheduler::{Scheduler, TaskScheduler};
use ic_task_scheduler::task::{ScheduledTask, TaskOptions};
use log::*;
use minter_did::error::{Error, Result};
use minter_did::id256::Id256;
use minter_did::init::InitData;
use minter_did::order::{self, SignedMintOrder};
use minter_did::reason::Icrc2Burn;

use crate::build_data::canister_build_data;
use crate::constant::{PENDING_TASKS_MEMORY_ID, TASK_RETRY_DELAY_SECS};
use crate::memory::MEMORY_MANAGER;
use crate::state::{Settings, State};
use crate::tasks::{BridgeTask, BurntIcrc2Data};
use crate::tokens::{bft_bridge, icrc1, icrc2};

mod inspect;

/// A canister to transfer funds between IC token canisters and EVM canister contracts.
#[derive(Canister, Clone)]
pub struct MinterCanister {
    #[id]
    id: Principal,
}

impl PreUpdate for MinterCanister {
    fn pre_update(&self, _method_name: &str, _method_type: MethodType) {}
}

impl MinterCanister {
    /// Initializes the timers
    pub fn set_timers(&mut self) {
        // This block of code only need to be run in the wasm environment
        #[cfg(target_family = "wasm")]
        {
            self.update_metrics_timer(std::time::Duration::from_secs(60 * 60));
        }
    }

    /// Initialize the canister with given data.
    #[init]
    pub fn init(&mut self, init_data: InitData) {
        let state = get_state();
        let mut state = state.borrow_mut();

        if let Err(err) = state
            .logger_config_service
            .init(init_data.log_settings.clone().unwrap_or_default())
        {
            ic_exports::ic_cdk::println!("error configuring the logger. Err: {err:?}")
        }

        info!("starting minter canister");
        debug!("minter canister init data: [{init_data:?}]");

        check_anonymous_principal(init_data.owner).expect("anonymous principal not allowed");

        let settings = Settings {
            owner: init_data.owner,
            evm_principal: init_data.evm_principal,
            signing_strategy: init_data.signing_strategy,
            spender_principal: init_data.spender_principal,
        };

        state.config.reset(settings);

        self.set_timers();
    }

    #[post_upgrade]
    pub fn post_upgrade(&mut self) {
        let state = get_state();
        let mut state = state.borrow_mut();

        if let Err(err) = state.logger_config_service.reload() {
            ic_exports::ic_cdk::println!("error configuring the logger. Err: {err:?}")
        }

        self.set_timers();
        debug!("upgrade completed");
    }

    /// set_logger_filter inspect_message check
    pub fn set_logger_filter_inspect_message_check(
        principal: Principal,
        state: &State,
    ) -> Result<()> {
        inspect_check_is_owner(principal, state)
    }

    /// Updates the runtime configuration of the logger with a new filter in the same form as the `RUST_LOG`
    /// environment variable.
    /// Example of valid filters:
    /// - info
    /// - debug,crate1::mod1=error,crate1::mod2,crate2=debug
    #[update]
    pub fn set_logger_filter(&mut self, filter: String) -> Result<()> {
        let state = get_state();
        let mut state = state.borrow_mut();

        MinterCanister::set_logger_filter_inspect_message_check(ic::caller(), &state)?;
        state.logger_config_service.set_logger_filter(&filter);

        debug!("updated logger filter to {filter}");

        Ok(())
    }

    /// ic_logs inspect_message check
    pub fn ic_logs_inspect_message_check(principal: Principal, state: &State) -> Result<()> {
        inspect_check_is_owner(principal, state)
    }

    /// Gets the logs
    /// - `count` is the number of logs to return
    #[update]
    pub fn ic_logs(&self, count: usize, offset: usize) -> Result<Logs> {
        MinterCanister::ic_logs_inspect_message_check(ic::caller(), &get_state().borrow());

        // Request execution
        Ok(ic_log::take_memory_records(count, offset))
    }

    /// Returns principal of canister owner.
    #[query]
    pub fn get_owner(&self) -> Principal {
        get_state().borrow().config.get_owner()
    }

    /// set_owner inspect_message check
    pub fn set_owner_inspect_message_check(
        principal: Principal,
        owner: Principal,
        state: &State,
    ) -> Result<()> {
        check_anonymous_principal(owner)?;
        inspect_check_is_owner(principal, state)
    }

    /// Sets a new principal for canister owner.
    ///
    /// This method should be called only by current owner,
    /// else `Error::NotAuthorised` will be returned.
    #[update]
    pub fn set_owner(&mut self, owner: Principal) -> Result<()> {
        let state = get_state();
        let mut state = state.borrow_mut();

        MinterCanister::set_owner_inspect_message_check(ic::caller(), owner, &state)?;
        state.config.set_owner(owner);

        info!("minter canister owner changed to {owner}");
        Ok(())
    }

    /// Returns principal of EVM canister with which the minter canister works.
    #[query]
    pub fn get_evm_principal(&self) -> Principal {
        get_state().borrow().config.get_evm_principal()
    }

    /// set_evm_principal inspect_message check
    pub fn set_evm_principal_inspect_message_check(
        principal: Principal,
        evm: Principal,
        state: &State,
    ) -> Result<()> {
        check_anonymous_principal(evm)?;
        inspect_check_is_owner(principal, state)
    }

    /// Sets principal of EVM canister with which the minter canister works.
    ///
    /// This method should be called only by current owner,
    /// else `Error::NotAuthorised` will be returned.
    #[update]
    pub fn set_evm_principal(&mut self, evm: Principal) -> Result<()> {
        let state = get_state();
        let mut state = state.borrow_mut();

        MinterCanister::set_evm_principal_inspect_message_check(ic::caller(), evm, &state)?;
        state.config.set_evm_principal(evm);

        info!("EVM principal changed to {evm}");
        Ok(())
    }

    /// Returns bridge contract address for EVM.
    /// If contract isn't initialized yet - returns None.
    #[query]
    pub fn get_bft_bridge_contract(&mut self) -> Option<H160> {
        get_state().borrow().config.get_bft_bridge_contract()
    }

    /// register_evmc_bft_bridge inspect_message check
    pub fn register_evmc_bft_bridge_inspect_message_check(
        principal: Principal,
        bft_bridge_address: H160,
        state: &State,
    ) -> Result<()> {
        inspect_check_is_owner(principal, state)?;
        if bft_bridge_address == H160::default() {
            return Err(Error::Internal(
                "BFTBridge contract address shouldn' be zero".into(),
            ));
        }

        if let Some(address) = state.config.get_bft_bridge_contract() {
            return Err(Error::BftBridgeAlreadyRegistered(address));
        }

        Ok(())
    }

    /// Registers BftBridge contract for EVM canister.
    /// This method is available for canister owner only.
    #[update]
    pub async fn register_evmc_bft_bridge(&self, bft_bridge_address: H160) -> Result<()> {
        let state = get_state();
        let state = state.borrow();

        Self::register_evmc_bft_bridge_inspect_message_check(
            ic::caller(),
            bft_bridge_address.clone(),
            &state,
        );

        let client = state.config.get_evm_client();
        self.register_evm_bridge(&client, bft_bridge_address)
            .await?;

        Ok(())
    }

    /// Returns `(nonce, mint_order)` pairs for the given sender id.
    #[query]
    pub async fn list_mint_orders(
        &self,
        sender: Id256,
        src_token: Id256,
    ) -> Vec<(u32, SignedMintOrder)> {
        get_state().borrow().mint_orders.get_all(sender, src_token)
    }

    /// Returns `(nonce, mint_order)` pairs for the given sender id and operation_id.
    #[query]
    pub async fn get_mint_order(
        &self,
        sender: Id256,
        src_token: Id256,
        operation_id: u32,
    ) -> Option<SignedMintOrder> {
        get_state()
            .borrow()
            .mint_orders
            .get(sender, src_token, operation_id)
    }

    /// burn_icrc2 inspect_message check
    pub fn burn_icrc2_inspect_message_check(reason: &Icrc2Burn) -> Result<()> {
        inspect_mint_reason(reason)
    }

    /// Create signed withdraw order data according to the given withdraw `reason`.
    /// A token to mint will be selected automatically by the `reason`.
    /// Returns operation id.
    #[update]
    pub async fn burn_icrc2(&mut self, reason: Icrc2Burn) -> Result<u32> {
        debug!("creating ERC20 mint order with reason {reason:?}");

        let caller = ic::caller();
        let state = get_state();
        MinterCanister::burn_icrc2_inspect_message_check(&reason)?;

        let caller_account = Account {
            owner: caller,
            subaccount: reason.from_subaccount,
        };

        let token_info = icrc1::query_token_info_or_read_from_cache(reason.icrc2_token_principal)
            .await
            .ok_or(Error::InvalidBurnOperation(
                "failed to get token info".into(),
            ))?;

        let name = order::fit_str_to_array(&token_info.name);
        let symbol = order::fit_str_to_array(&token_info.symbol);

        icrc2::burn(
            reason.icrc2_token_principal,
            caller_account,
            (&reason.amount).into(),
            true,
        )
        .await?;

        let operation_id = state.borrow_mut().next_nonce();

        let options = TaskOptions::default()
            .with_backoff_policy(BackoffPolicy::Fixed {
                secs: TASK_RETRY_DELAY_SECS,
            })
            .with_max_retries_policy(u32::MAX);
        get_scheduler().borrow_mut().append_task(
            BridgeTask::PrepareMintOrder(BurntIcrc2Data {
                sender: caller,
                amount: reason.amount,
                operation_id,
                name,
                symbol,
                decimals: token_info.decimals,
                src_token: reason.icrc2_token_principal,
                recipient_address: reason.recipient_address,
            })
            .into_scheduled(options),
        );

        Ok(operation_id)
    }

    /// Returns evm_address of the minter canister.
    #[update]
    pub async fn get_minter_canister_evm_address(&mut self) -> Result<H160> {
        let signer = get_state().borrow().signer.get_transaction_signer();
        signer
            .get_address()
            .await
            .map_err(|e| Error::Internal(format!("failed to get minter canister address: {e}")))
    }

    async fn register_evm_bridge(
        &self,
        evm: &EthJsonRpcClient<impl Client>,
        bft_bridge: H160,
    ) -> Result<()> {
        bft_bridge::check_bft_bridge_contract(evm, bft_bridge.clone(), get_state()).await?;

        get_state()
            .borrow_mut()
            .config
            .set_bft_bridge_contract(bft_bridge);

        Ok(())
    }

    /// Returns the build data of the canister
    #[query]
    pub fn get_canister_build_data(&self) -> BuildData {
        canister_build_data()
    }

    /// Returns candid IDL.
    /// This should be the last fn to see previous endpoints in macro.
    pub fn idl() -> Idl {
        generate_idl!()
    }
}

impl Metrics for MinterCanister {
    fn metrics(&self) -> Rc<RefCell<ic_metrics::MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
    }
}

/// inspect function to check whether provided principal is owner
fn inspect_check_is_owner(principal: Principal, state: &State) -> Result<()> {
    let owner = state.config.get_owner();

    if owner != principal {
        return Err(Error::NotAuthorized);
    }

    Ok(())
}

/// inspect function to check whether the provided principal is anonymous
fn check_anonymous_principal(principal: Principal) -> Result<()> {
    if principal == Principal::anonymous() {
        return Err(Error::AnonymousPrincipal);
    }

    Ok(())
}

/// Checks if addresses and amount are non-zero.
fn inspect_mint_reason(reason: &Icrc2Burn) -> Result<()> {
    if reason.amount == U256::zero() {
        return Err(Error::InvalidBurnOperation("amount is zero".into()));
    }

    if reason.recipient_address == H160::zero() {
        return Err(Error::InvalidBurnOperation(
            "recipient address is zero".into(),
        ));
    }

    Ok(())
}

type TasksStorage =
    StableUnboundedMap<u32, ScheduledTask<BridgeTask>, VirtualMemory<DefaultMemoryImpl>>;
type PersistentScheduler = Scheduler<BridgeTask, TasksStorage>;

thread_local! {
    pub static STATE: Rc<RefCell<State>> = Rc::default();

    pub static SCHEDULER: Rc<RefCell<PersistentScheduler>> = Rc::new(RefCell::new({
        let pending_tasks =
            TasksStorage::new(MEMORY_MANAGER.with(|mm| mm.get(PENDING_TASKS_MEMORY_ID)));
            PersistentScheduler::new(pending_tasks)
    }));
}

pub fn get_scheduler() -> Rc<RefCell<PersistentScheduler>> {
    SCHEDULER.with(|scheduler| scheduler.clone())
}

pub fn get_state() -> Rc<RefCell<State>> {
    STATE.with(|state| state.clone())
}

#[cfg(test)]
mod test {
    use candid::Principal;
    use eth_signer::sign_strategy::SigningStrategy;
    use ic_canister::{canister_call, Canister};
    use ic_exports::ic_kit::{inject, MockContext};
    use minter_did::error::Error;

    use super::*;
    use crate::MinterCanister;

    fn owner() -> Principal {
        Principal::from_slice(&[1; 20])
    }

    fn bob() -> Principal {
        Principal::from_slice(&[2; 20])
    }

    async fn init_canister() -> MinterCanister {
        MockContext::new().inject();

        const MOCK_PRINCIPAL: &str = "mfufu-x6j4c-gomzb-geilq";
        let mock_canister_id = Principal::from_text(MOCK_PRINCIPAL).expect("valid principal");
        let mut canister = MinterCanister::from_principal(mock_canister_id);

        let init_data = InitData {
            owner: owner(),
            evm_principal: Principal::anonymous(),
            spender_principal: Principal::anonymous(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            log_settings: None,
        };
        canister_call!(canister.init(init_data), ()).await.unwrap();
        canister
    }

    #[tokio::test]
    #[should_panic = "anonymous principal not allowed"]
    async fn disallow_anonymous_owner_in_init() {
        MockContext::new().inject();
        const MOCK_PRINCIPAL: &str = "mfufu-x6j4c-gomzb-geilq";
        let mock_canister_id = Principal::from_text(MOCK_PRINCIPAL).expect("valid principal");
        let mut canister = MinterCanister::from_principal(mock_canister_id);

        let init_data = InitData {
            owner: Principal::anonymous(),
            evm_principal: Principal::anonymous(),
            spender_principal: Principal::anonymous(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            log_settings: None,
        };
        canister_call!(canister.init(init_data), ()).await.unwrap();
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
        assert_eq!(stored_evm, Principal::anonymous());
    }

    #[tokio::test]
    async fn owner_access_control() {
        let mut canister = init_canister().await;

        // try to call with not owner id
        let set_error = canister_call!(canister.set_owner(bob()), Result<()>)
            .await
            .unwrap()
            .unwrap_err();
        assert_eq!(set_error, Error::NotAuthorized);

        // now we will try to call it with owner id
        inject::get_context().update_id(owner());
        canister_call!(canister.set_owner(bob()), Result<()>)
            .await
            .unwrap()
            .unwrap();

        // check if state updated
        let stored_owner = canister_call!(canister.get_owner(), Principal)
            .await
            .unwrap();
        assert_eq!(stored_owner, bob());
    }

    #[tokio::test]
    async fn evm_principal_access_control() {
        let mut canister = init_canister().await;

        // try to call with not owner id
        let set_error = canister_call!(canister.set_evm_principal(bob()), Result<()>)
            .await
            .unwrap()
            .unwrap_err();
        assert_eq!(set_error, Error::NotAuthorized);

        // now we will try to call it with owner id
        inject::get_context().update_id(owner());
        canister_call!(canister.set_evm_principal(bob()), Result<()>)
            .await
            .unwrap()
            .unwrap();

        // check if state updated
        let stored_owner = canister_call!(canister.get_evm_principal(), Principal)
            .await
            .unwrap();
        assert_eq!(stored_owner, bob());
    }

    #[tokio::test]
    async fn set_anonymous_principal_as_owner() {
        let mut canister = init_canister().await;

        inject::get_context().update_id(owner());

        let err = canister_call!(canister.set_owner(Principal::anonymous()), Result<()>)
            .await
            .unwrap()
            .unwrap_err();

        assert_eq!(err, Error::AnonymousPrincipal);
    }

    #[tokio::test]
    async fn set_evm_bft_bridge_should_fail_for_non_owner() {
        let canister = init_canister().await;

        let err = canister_call!(
            canister.register_evmc_bft_bridge(H160::from_slice(&[2u8; 20])),
            Result<()>
        )
        .await
        .unwrap()
        .unwrap_err();

        assert_eq!(err, Error::NotAuthorized);
    }

    // This test work fine if executed alone but could fail if executed with all other tests
    // due to the global nature of the global logger in Rust.
    // In fact, if the Rust log is already set, a second attempt to set it causes a panic
    #[ignore]
    #[tokio::test]
    async fn test_set_logger_filter() {
        MockContext::new().inject();
        const MOCK_PRINCIPAL: &str = "mfufu-x6j4c-gomzb-geilq";
        let mock_canister_id = Principal::from_text(MOCK_PRINCIPAL).expect("valid principal");
        let mut canister = MinterCanister::from_principal(mock_canister_id);

        let init_data = InitData {
            owner: Principal::anonymous(),
            evm_principal: Principal::anonymous(),
            spender_principal: Principal::anonymous(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            log_settings: None,
        };
        canister_call!(canister.init(init_data), ()).await.unwrap();

        {
            let info_message = format!("message-{}", rand::random::<u64>());
            let error_message = format!("message-{}", rand::random::<u64>());

            log::info!("{info_message}");
            log::error!("{error_message}");

            // Only the error message should be present
            let log_records = ic_log::take_memory_records(128, 0);
            assert!(!log_records
                .logs
                .iter()
                .any(|log| log.log.contains(&info_message)));
            assert!(log_records
                .logs
                .iter()
                .any(|log| log.log.contains(&error_message)));
        }
        // Set new logger filter
        let new_filter = "info";
        let res = canister_call!(
            canister.set_logger_filter(new_filter.to_string()),
            Result<()>
        )
        .await
        .unwrap();
        assert!(res.is_ok());

        {
            let info_message = format!("message-{}", rand::random::<u64>());
            let error_message = format!("message-{}", rand::random::<u64>());

            log::info!("{info_message}");
            log::error!("{error_message}");

            // All log messages should be present
            let log_records = ic_log::take_memory_records(128, 0);
            assert!(log_records
                .logs
                .iter()
                .any(|log| log.log.contains(&info_message)));
            assert!(log_records
                .logs
                .iter()
                .any(|log| log.log.contains(&error_message)));
        }
    }

    #[tokio::test]
    async fn test_ic_logs_is_access_controlled() {
        MockContext::new().inject();
        const MOCK_PRINCIPAL: &str = "mfufu-x6j4c-gomzb-geilq";
        let mock_canister_id = Principal::from_text(MOCK_PRINCIPAL).expect("valid principal");
        let mut canister = MinterCanister::from_principal(mock_canister_id);

        let init_data = InitData {
            owner: Principal::management_canister(),
            evm_principal: Principal::management_canister(),
            spender_principal: Principal::anonymous(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            log_settings: None,
        };
        canister_call!(canister.init(init_data), ()).await.unwrap();

        inject::get_context().update_id(Principal::management_canister());

        let logs = canister_call!(canister.ic_logs(10, 0), Result<Logs>)
            .await
            .unwrap();
        assert!(logs.is_ok());

        let init_data = InitData {
            owner: Principal::management_canister(),
            evm_principal: Principal::management_canister(),
            spender_principal: Principal::management_canister(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            log_settings: None,
        };
        canister_call!(canister.init(init_data), ()).await.unwrap();

        inject::get_context().update_id(Principal::anonymous());

        let logs = canister_call!(canister.ic_logs(10, 0), Result<Logs>)
            .await
            .unwrap();
        assert!(logs.is_err());
        assert_eq!(logs.unwrap_err(), Error::NotAuthorized);
    }

    #[tokio::test]
    async fn test_get_minter_canister_evm_address() {
        MockContext::new().inject();
        const MOCK_PRINCIPAL: &str = "mfufu-x6j4c-gomzb-geilq";
        let mock_canister_id = Principal::from_text(MOCK_PRINCIPAL).expect("valid principal");
        let mut canister = MinterCanister::from_principal(mock_canister_id);

        let init_data = InitData {
            owner: Principal::management_canister(),
            evm_principal: Principal::management_canister(),
            spender_principal: Principal::management_canister(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            log_settings: None,
        };
        canister_call!(canister.init(init_data), ()).await.unwrap();
        inject::get_context().update_id(Principal::management_canister());

        let evm_address = canister_call!(canister.get_minter_canister_evm_address(), Result<H160>)
            .await
            .unwrap();

        assert!(evm_address.is_ok());
    }

    #[tokio::test]
    async fn test_get_bft_bridge_contract() {
        MockContext::new().inject();
        const MOCK_PRINCIPAL: &str = "mfufu-x6j4c-gomzb-geilq";
        let mock_canister_id = Principal::from_text(MOCK_PRINCIPAL).expect("valid principal");
        let mut canister = MinterCanister::from_principal(mock_canister_id);

        let init_data = InitData {
            owner: Principal::management_canister(),
            evm_principal: Principal::management_canister(),
            spender_principal: Principal::management_canister(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            log_settings: None,
        };
        canister_call!(canister.init(init_data), ()).await.unwrap();

        inject::get_context().update_id(Principal::management_canister());

        let evm_address = canister_call!(canister.get_bft_bridge_contract(), Result<Option<H160>>)
            .await
            .unwrap();

        assert!(evm_address.is_some());
    }
}
