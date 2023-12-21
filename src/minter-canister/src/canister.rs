use std::cell::RefCell;
use std::rc::Rc;

use candid::{Nat, Principal};
use did::{H160, U256};
use eth_signer::sign_strategy::TransactionSigner;
use ic_canister::{
    generate_idl, init, post_upgrade, query, update, virtual_canister_call, Canister, Idl,
    MethodType, PreUpdate,
};
use ic_exports::ic_cdk::api::management_canister::http_request::{HttpResponse, TransformArgs};
use ic_exports::ic_kit::ic;
use ic_exports::icrc_types::icrc1::account::Account;
use ic_exports::icrc_types::icrc2::approve::ApproveError;
use ic_exports::icrc_types::icrc2::transfer_from::TransferFromError;
use ic_metrics::{Metrics, MetricsStorage};
use log::*;
use minter_did::error::{Error, Result};
use minter_did::id256::Id256;
use minter_did::init::InitData;
use minter_did::order::SignedMintOrder;
use minter_did::reason::Icrc2Burn;

use crate::context::{get_base_context, Context, ContextImpl};
use crate::evm::{Evm, EvmCanisterImpl};
use crate::state::{Settings, State};
use crate::tokens::icrc1::IcrcTransferDst;
use crate::tokens::{bft_bridge, icrc1, icrc2};

mod inspect;

/// Type alias for the shared mutable context implementation we use in the canister
type SharedContext = Rc<RefCell<ContextImpl<EvmCanisterImpl>>>;

#[derive(Clone, Default)]
pub struct ContextWrapper(pub SharedContext);

/// A canister to transfer funds between IC token canisters and EVM canister contracts.
#[derive(Canister, Clone)]
pub struct MinterCanister {
    #[id]
    id: Principal,
    pub context: ContextWrapper,
}

impl PreUpdate for MinterCanister {
    fn pre_update(&self, _method_name: &str, _method_type: MethodType) {}
}

impl MinterCanister {
    fn with_state<R>(&self, f: impl FnOnce(&State) -> R) -> R {
        let ctx = self.context.0.borrow();
        let res = f(&ctx.get_state());
        res
    }

    fn with_state_mut<R>(&self, f: impl FnOnce(&mut State) -> R) -> R {
        let ctx = self.context.0.borrow();
        let res = f(&mut ctx.mut_state());
        res
    }

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
        self.with_state_mut(|s| {
            if let Err(err) = s
                .logger_config_service
                .init(init_data.log_settings.clone().unwrap_or_default())
            {
                ic_exports::ic_cdk::println!("error configuring the logger. Err: {err:?}")
            }
        });

        info!("starting minter canister");
        debug!("minter canister init data: [{init_data:?}]");

        check_anonymous_principal(init_data.owner).expect("anonymous principal not allowed");

        let settings = Settings {
            owner: init_data.owner,
            evm_principal: init_data.evm_principal,
            evm_gas_price: init_data.evm_gas_price,
            signing_strategy: init_data.signing_strategy,
            chain_id: init_data.evm_chain_id,
            bft_bridge_contract: init_data.bft_bridge_contract,
            spender_principal: init_data.spender_principal,
            process_transactions_results_interval: init_data.process_transactions_results_interval,
        };

        self.context.0.borrow_mut().get_state();
        self.with_state_mut(|s| s.reset(settings));

        self.set_timers();
    }

    #[post_upgrade]
    pub fn post_upgrade(&mut self) {
        self.with_state_mut(|s| {
            if let Err(err) = s.logger_config_service.reload() {
                ic_exports::ic_cdk::println!("error configuring the logger. Err: {err:?}")
            }
        });
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
        self.with_state_mut(|s| {
            MinterCanister::set_logger_filter_inspect_message_check(ic::caller(), s)?;
            s.logger_config_service.set_logger_filter(&filter)
        })?;

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
    pub fn ic_logs(&self, count: usize) -> Result<Vec<String>> {
        self.with_state(|s| MinterCanister::ic_logs_inspect_message_check(ic::caller(), s))?;

        // Request execution
        Ok(ic_log::take_memory_records(count))
    }

    /// Returns principal of canister owner.
    #[query]
    pub fn get_owner(&self) -> Principal {
        self.with_state(|s| s.config.get_owner())
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
        self.with_state_mut::<Result<()>>(|s| {
            MinterCanister::set_owner_inspect_message_check(ic::caller(), owner, s)?;
            s.config.set_owner(owner);
            Ok(())
        })?;
        info!("minter canister owner changed to {owner}");
        Ok(())
    }

    /// Returns principal of EVM canister with which the minter canister works.
    #[query]
    pub fn get_evm_principal(&self) -> Principal {
        self.with_state(|s| s.config.get_evm_principal())
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
        self.with_state_mut::<Result<()>>(|s| {
            MinterCanister::set_evm_principal_inspect_message_check(ic::caller(), evm, s)?;
            s.config.set_evm_principal(evm);
            Ok(())
        })?;
        info!("EVM principal changed to {evm}");
        Ok(())
    }

    /// Returns bridge contract address for EVM with the given chain id.
    /// If `chain_id` is None - returns bridge contract address for EVM canister.
    /// If contract isn't initialized yet - returns None.
    #[update]
    pub fn get_bft_bridge_contract(&mut self) -> Result<Option<H160>> {
        // deduct fee for endpoint query

        Ok(self
            .context
            .0
            .borrow()
            .get_state()
            .config
            .get_bft_bridge_contract())
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
        self.with_state::<Result<()>>(|state| {
            Self::register_evmc_bft_bridge_inspect_message_check(
                ic::caller(),
                bft_bridge_address.clone(),
                state,
            )
        })?;

        let evmc = self.context.0.borrow().get_evm_canister();
        self.register_evm_bridge(evmc.as_evm(), bft_bridge_address)
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
        self.with_state(|s| s.mint_orders.get_all(sender, src_token))
    }

    /// create_erc_20_mint_order inspect_message check
    pub fn create_erc_20_mint_order_inspect_message_check(
        _principal: Principal,
        reason: &Icrc2Burn,
        _state: &State,
    ) -> Result<()> {
        inspect_mint_reason(reason)
    }

    /// Create signed withdraw order data according to the given withdraw `reason`.
    /// A token to mint will be selected automatically by the `reason`.
    #[update]
    pub async fn create_erc_20_mint_order(&mut self, reason: Icrc2Burn) -> Result<SignedMintOrder> {
        debug!("creating ERC20 mint order with reason {reason:?}");

        let context = get_base_context(&self.context.0);
        MinterCanister::create_erc_20_mint_order_inspect_message_check(
            ic::caller(),
            &reason,
            &context.borrow().get_state(),
        )?;

        let token_service = context.borrow().get_evm_token_service();
        token_service
            .create_mint_order_for(ic::caller(), reason, &context)
            .await
    }

    /// Returns evm_address of the minter canister.
    #[update]
    pub async fn get_minter_canister_evm_address(&mut self) -> Result<H160> {
        // deduct fee for endpoint query

        let ctx = get_base_context(&self.context.0);
        let signer = ctx.borrow().get_state().signer.get_transaction_signer();
        signer
            .get_address()
            .await
            .map_err(|e| Error::Internal(format!("failed to get minter canister address: {e}")))
    }

    /// start_icrc2_mint inspect_message check
    pub fn start_icrc2_mint_inspect_message_check(
        _principal: Principal,
        user: &H160,
        _state: &State,
    ) -> Result<()> {
        if user.0.is_zero() {
            return Err(Error::InvalidBurnOperation("zero user address".into()));
        };
        Ok(())
    }

    /// Returns approved ICRC-2 amount.
    #[update]
    pub async fn start_icrc2_mint(&mut self, user: H160, operation_id: u32) -> Result<Nat> {
        let ctx = get_base_context(&self.context.0);

        MinterCanister::start_icrc2_mint_inspect_message_check(
            ic::caller(),
            &user,
            &ctx.borrow().get_state(),
        )?;

        let token_service = ctx.borrow().get_evm_token_service();
        let valid_burn = token_service
            .check_erc_20_burn(&user, operation_id, &ctx)
            .await?
            .try_map_dst(Principal::try_from)?;

        let chain_id = ctx.borrow().get_state().config.get_evmc_chain_id();
        let tx_subaccount = icrc2::approve_subaccount(
            user,
            operation_id,
            chain_id,
            valid_burn.to_token,
            valid_burn.recipient,
        );
        let spender = ctx.borrow().get_state().config.get_spender_principal();
        let spender_account = Account {
            owner: spender,
            subaccount: Some(tx_subaccount),
        };

        let approval_result = icrc2::approve_mint(
            valid_burn.to_token,
            spender_account,
            (&valid_burn.amount).into(),
            true,
        )
        .await;

        let allowance = match approval_result {
            Ok(success) => success.amount,
            Err(Error::Icrc2ApproveError(ApproveError::AllowanceChanged { current_allowance })) => {
                current_allowance
            }
            Err(e) => return Err(e),
        };

        Ok(allowance)
    }

    /// start_icrc2_mint inspect_message check
    pub fn finish_icrc2_mint_inspect_message_check(
        _caller: Principal,
        amount: &Nat,
        user: &H160,
        _state: &State,
    ) -> Result<()> {
        if amount == &Nat::from(0) {
            return Err(Error::InvalidBurnOperation("zero amount".into()));
        }

        if user.0.is_zero() {
            return Err(Error::InvalidBurnOperation("zero user address".into()));
        };
        Ok(())
    }

    /// Make `SpenderCanister` to transfer ICRC-2 tokens to the user according to the given `burn_tx`.
    /// Returns transfer ID in case of success.
    ///
    /// Before client can use this method, he should call `start_icrc2_mint` for the given `burn_tx`.
    /// After the approval, user should finalize Wrapped token burning, using `BFTBridge::finish_burn()`.
    #[update]
    async fn finish_icrc2_mint(
        &self,
        operation_id: u32,
        user: H160,
        token: Principal,
        recipient: Principal,
        amount: Nat,
    ) -> Result<Nat> {
        let ctx = get_base_context(&self.context.0);
        let caller = ic::caller();

        Self::finish_icrc2_mint_inspect_message_check(
            caller,
            &amount,
            &user,
            &ctx.borrow().get_state(),
        )?;

        let token_service = ctx.borrow().get_evm_token_service();
        token_service
            .check_erc_20_burn_finalized(&user, operation_id, &ctx)
            .await?;

        let spender_canister = ctx.borrow().get_state().config.get_spender_principal();

        let fee = icrc1::get_token_configuration(token).await?.fee;

        let chain_id = ctx.borrow().get_state().config.get_evmc_chain_id();
        let spender_subaccount =
            icrc2::approve_subaccount(user, operation_id, chain_id, token, recipient);

        let dst_info = IcrcTransferDst { token, recipient };
        let mut transfer_result = virtual_canister_call!(
            spender_canister,
            "finish_icrc2_mint",
            (dst_info.token, dst_info.recipient, spender_subaccount, amount.clone(), fee),
            std::result::Result<Nat, TransferFromError>
        )
        .await?;

        if let Err(TransferFromError::BadFee { expected_fee }) = transfer_result {
            // refresh cached token configuration if fee changed
            let _ = icrc1::refresh_token_configuration(dst_info.token).await;

            transfer_result = virtual_canister_call!(
                spender_canister,
                "finish_icrc2_mint",
                (dst_info, spender_subaccount, amount, expected_fee),
                std::result::Result<Nat, TransferFromError>
            )
            .await?;
        }

        Ok(transfer_result?)
    }

    /// Requirements for Http outcalls, used to ignore small differences in the data obtained
    /// by different nodes of the IC subnet to reach a consensus, more info:
    /// https://internetcomputer.org/docs/current/developer-docs/integrations/http_requests/http_requests-how-it-works#transformation-function
    #[query]
    fn transform(&self, raw: TransformArgs) -> HttpResponse {
        HttpResponse {
            status: raw.response.status,
            headers: raw.response.headers,
            body: raw.response.body,
        }
    }

    async fn register_evm_bridge(&self, evm: &dyn Evm, bft_bridge: H160) -> Result<()> {
        let context = get_base_context(&self.context.0);

        bft_bridge::check_bft_bridge_contract(evm, bft_bridge.clone(), &context).await?;

        self.context
            .0
            .borrow_mut()
            .mut_state()
            .config
            .set_bft_bridge_contract(bft_bridge);

        Ok(())
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

#[cfg(test)]
mod test {
    use candid::Principal;
    use eth_signer::sign_strategy::SigningStrategy;
    use ic_canister::{canister_call, Canister};
    use ic_exports::ic_kit::{inject, MockContext};
    use minter_did::error::Error;

    use super::*;
    use crate::constant::{DEFAULT_CHAIN_ID, DEFAULT_GAS_PRICE};
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
            evm_chain_id: DEFAULT_CHAIN_ID,
            bft_bridge_contract: None,
            evm_gas_price: DEFAULT_GAS_PRICE.into(),
            spender_principal: Principal::anonymous(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            process_transactions_results_interval: None,
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
            evm_chain_id: DEFAULT_CHAIN_ID,
            bft_bridge_contract: None,
            spender_principal: Principal::anonymous(),
            evm_gas_price: DEFAULT_GAS_PRICE.into(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            process_transactions_results_interval: None,
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
            evm_chain_id: DEFAULT_CHAIN_ID,
            bft_bridge_contract: None,
            evm_gas_price: DEFAULT_GAS_PRICE.into(),
            spender_principal: Principal::anonymous(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            process_transactions_results_interval: None,
            log_settings: None,
        };
        canister_call!(canister.init(init_data), ()).await.unwrap();

        {
            let info_message = format!("message-{}", rand::random::<u64>());
            let error_message = format!("message-{}", rand::random::<u64>());

            log::info!("{info_message}");
            log::error!("{error_message}");

            // Only the error message should be present
            let log_records = ic_log::take_memory_records(128);
            assert!(!log_records.iter().any(|log| log.contains(&info_message)));
            assert!(log_records.iter().any(|log| log.contains(&error_message)));
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
            let log_records = ic_log::take_memory_records(128);
            assert!(log_records.iter().any(|log| log.contains(&info_message)));
            assert!(log_records.iter().any(|log| log.contains(&error_message)));
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
            bft_bridge_contract: None,
            evm_chain_id: DEFAULT_CHAIN_ID,
            evm_gas_price: DEFAULT_GAS_PRICE.into(),
            spender_principal: Principal::anonymous(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            process_transactions_results_interval: None,
            log_settings: None,
        };
        canister_call!(canister.init(init_data), ()).await.unwrap();

        inject::get_context().update_id(Principal::management_canister());

        let logs = canister_call!(canister.ic_logs(10), Result<Vec<String>>)
            .await
            .unwrap();
        assert!(logs.is_ok());

        let init_data = InitData {
            owner: Principal::management_canister(),
            evm_principal: Principal::management_canister(),
            evm_chain_id: DEFAULT_CHAIN_ID,
            bft_bridge_contract: None,
            evm_gas_price: DEFAULT_GAS_PRICE.into(),
            spender_principal: Principal::management_canister(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            process_transactions_results_interval: None,
            log_settings: None,
        };
        canister_call!(canister.init(init_data), ()).await.unwrap();

        inject::get_context().update_id(Principal::anonymous());

        let logs = canister_call!(canister.ic_logs(10), Result<Vec<String>>)
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
            evm_chain_id: DEFAULT_CHAIN_ID,
            bft_bridge_contract: None,
            evm_gas_price: DEFAULT_GAS_PRICE.into(),
            spender_principal: Principal::management_canister(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            process_transactions_results_interval: None,
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
            evm_chain_id: DEFAULT_CHAIN_ID,
            bft_bridge_contract: None,
            evm_gas_price: DEFAULT_GAS_PRICE.into(),
            spender_principal: Principal::management_canister(),
            signing_strategy: SigningStrategy::Local {
                private_key: [1u8; 32],
            },
            process_transactions_results_interval: None,
            log_settings: None,
        };
        canister_call!(canister.init(init_data), ()).await.unwrap();

        inject::get_context().update_id(Principal::management_canister());

        let evm_address = canister_call!(canister.get_bft_bridge_contract(), Result<Option<H160>>)
            .await
            .unwrap();

        assert!(evm_address.is_ok());
    }
}
