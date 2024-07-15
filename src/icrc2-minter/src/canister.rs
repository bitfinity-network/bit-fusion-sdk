use std::cell::RefCell;
use std::rc::Rc;

use bridge_canister::memory::{memory_by_id, StableMemory};
use bridge_canister::operation_store::{OperationStore, OperationsMemory};
use bridge_canister::runtime::state::config::ConfigStorage;
use bridge_canister::runtime::state::SharedConfig;
use bridge_canister::BridgeCanister;
use bridge_did::error::{BftResult, Error};
use bridge_did::id256::Id256;
use bridge_did::init::BridgeInitData;
use bridge_did::op_id::OperationId;
use bridge_did::order::SignedMintOrder;
use candid::Principal;
use did::build::BuildData;
use did::H160;
use ic_canister::{
    generate_idl, init, post_upgrade, query, update, Canister, Idl, MethodType, PreUpdate,
};
use ic_exports::ic_cdk::api::management_canister::http_request::{HttpResponse, TransformArgs};
use ic_exports::ic_kit::ic;
use ic_metrics::{Metrics, MetricsStorage};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{StableBTreeMap, VirtualMemory};
use ic_storage::IcStorage;
use ic_task_scheduler::scheduler::{Scheduler, TaskScheduler};
use ic_task_scheduler::task::{InnerScheduledTask, TaskOptions, TaskStatus};
use log::*;

use crate::constant::{
    OPERATIONS_COUNTER_MEMORY_ID, OPERATIONS_LOG_MEMORY_ID, OPERATIONS_MAP_MEMORY_ID,
    OPERATIONS_MEMORY_ID, PENDING_TASKS_MEMORY_ID,
};
use crate::memory::MEMORY_MANAGER;
use crate::operation::OperationState;
use crate::state::State;
use crate::tasks::BridgeTask;

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
        get_scheduler()
            .borrow_mut()
            .on_completion_callback(log_task_execution_error);
    }

    #[post_upgrade]
    pub fn post_upgrade(&mut self) {
        self.bridge_post_upgrade(Self::run_scheduler);
    }

    fn run_scheduler(log_task_options: TaskOptions) {
        let scheduler = get_scheduler();
        scheduler
            .borrow_mut()
            .append_task(BridgeTask::CollectEvmEvents.into_scheduled(log_task_options));

        let task_execution_result = scheduler.borrow_mut().run(());

        if let Err(err) = task_execution_result {
            error!("Failed to run tasks: {err:?}",);
        }
    }

    /// Returns `(nonce, mint_order)` pairs for the given sender id.
    /// Offset, if set, defines the starting index of the page,
    /// Count, if set, defines the number of elements in the page.
    #[query]
    pub fn list_mint_orders(
        &self,
        wallet_address: H160,
        src_token: Id256,
        offset: Option<usize>,
        count: Option<usize>,
    ) -> Vec<(u32, SignedMintOrder)> {
        Self::token_mint_orders(wallet_address, src_token, offset, count)
    }

    /// Returns `(nonce, mint_order)` pairs for the given sender id and operation_id.
    #[query]
    pub fn get_mint_order(
        &self,
        wallet_address: H160,
        src_token: Id256,
        operation_id: u32,
    ) -> Option<SignedMintOrder> {
        Self::token_mint_orders(wallet_address, src_token, None, None)
            .into_iter()
            .find(|(nonce, _)| *nonce == operation_id)
            .map(|(_, mint_order)| mint_order)
    }

    #[query]
    /// Returns the list of operations for the given wallet address.
    /// Offset, if set, defines the starting index of the page,
    /// Count, if set, defines the number of elements in the page.
    pub fn get_operations_list(
        &self,
        wallet_address: H160,
        offset: Option<usize>,
        count: Option<usize>,
    ) -> Vec<(OperationId, OperationState)> {
        get_operations_store().get_for_address(&wallet_address, offset, count)
    }

    /// Adds the provided principal to the whitelist.
    #[update]
    pub fn add_to_whitelist(&mut self, icrc2_principal: Principal) -> BftResult<()> {
        let state = get_state();

        Self::access_control_inspect_message_check(ic::caller(), icrc2_principal)?;

        let mut state = state.borrow_mut();

        state.access_list.add(icrc2_principal)?;

        Ok(())
    }

    /// Remove a icrc2 principal token from the access list
    #[update]
    pub fn remove_from_whitelist(&mut self, icrc2_principal: Principal) -> BftResult<()> {
        let state = get_state();

        Self::access_control_inspect_message_check(ic::caller(), icrc2_principal)?;

        let mut state = state.borrow_mut();

        state.access_list.remove(&icrc2_principal);

        Ok(())
    }

    /// Returns the list of all principals in the whitelist.
    #[query]
    fn get_whitelist(&self) -> Vec<Principal> {
        get_state().borrow().access_list.get_all_principals()
    }

    fn access_control_inspect_message_check(
        owner: Principal,
        icrc2_principal: Principal,
    ) -> BftResult<()> {
        inspect_check_is_owner(owner)?;
        check_anonymous_principal(icrc2_principal)?;

        Ok(())
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

    /// Get mint orders for the given wallet address and token;
    /// if `offset` and `count` are provided, returns a page of mint orders.
    fn token_mint_orders(
        wallet_address: H160,
        src_token: Id256,
        offset: Option<usize>,
        count: Option<usize>,
    ) -> Vec<(u32, SignedMintOrder)> {
        get_operations_store()
            .get_for_address(&wallet_address, None, None)
            .into_iter()
            .filter_map(|(operation_id, status)| {
                status
                    .get_signed_mint_order(Some(src_token))
                    .map(|mint_order| (operation_id.nonce(), *mint_order))
            })
            .skip(offset.unwrap_or_default())
            .take(count.unwrap_or(usize::MAX))
            .collect()
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

type TasksStorage =
    StableBTreeMap<u32, InnerScheduledTask<BridgeTask>, VirtualMemory<DefaultMemoryImpl>>;
type PersistentScheduler = Scheduler<BridgeTask, TasksStorage>;

fn log_task_execution_error(task: InnerScheduledTask<BridgeTask>) {
    match task.status() {
        TaskStatus::Failed {
            timestamp_secs,
            error,
        } => {
            log::error!(
                "task #{} execution failed: {error} at {timestamp_secs}",
                task.id()
            )
        }
        TaskStatus::TimeoutOrPanic { timestamp_secs } => {
            log::error!("task #{} panicked at {timestamp_secs}", task.id())
        }
        status_change => {
            log::trace!("task #{} status changed: {status_change:?}", task.id())
        }
    };
}

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

pub fn get_operations_store() -> OperationStore<StableMemory, OperationState> {
    let mem = OperationsMemory {
        id_counter: memory_by_id(OPERATIONS_COUNTER_MEMORY_ID),
        incomplete_operations: memory_by_id(OPERATIONS_MEMORY_ID),
        operations_log: memory_by_id(OPERATIONS_LOG_MEMORY_ID),
        operations_map: memory_by_id(OPERATIONS_MAP_MEMORY_ID),
    };

    OperationStore::with_memory(mem, None)
}

#[cfg(test)]
mod test {
    use candid::Principal;
    use did::U256;
    use eth_signer::sign_strategy::SigningStrategy;
    use ic_canister::{canister_call, Canister};
    use ic_exports::ic_kit::{inject, MockContext};

    use super::*;
    use crate::operation::DepositOperationState;
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
            evm_principal: Principal::anonymous(),
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
        assert_eq!(stored_evm, Principal::anonymous());
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

    #[tokio::test]
    async fn test_should_paginate_token_mint_orders() {
        fn eth_address(seed: u8) -> H160 {
            H160::from([seed; H160::BYTE_SIZE])
        }

        let token_id = eth_address(0);
        let token_id_id256 = Id256::from_evm_address(&token_id, 5);

        let op_state = OperationState::Deposit(DepositOperationState::MintOrderSigned {
            token_id: token_id_id256,
            amount: U256::one(),
            signed_mint_order: Box::new(SignedMintOrder([0; 334])),
        });

        let token_id_other = eth_address(1);
        let token_id_other_id256 = Id256::from_evm_address(&token_id_other, 5);

        let op_state_other = OperationState::Deposit(DepositOperationState::MintOrderSigned {
            token_id: token_id_other_id256,
            amount: U256::one(),
            signed_mint_order: Box::new(SignedMintOrder([0; 334])),
        });

        const COUNT: usize = 42;
        const COUNT_OTHER: usize = 10;

        let canister = init_canister().await;

        inject::get_context().update_id(owner());
        let mut op_store = get_operations_store();

        let owner = eth_address(2);
        let owner_other = eth_address(3);

        for _ in 0..COUNT {
            op_store.new_operation(owner.clone(), op_state.clone());
        }

        for _ in 0..COUNT_OTHER {
            op_store.new_operation(owner_other.clone(), op_state_other.clone());
        }

        // get orders for the first token
        let orders = canister_call!(
            canister.list_mint_orders(owner.clone(), token_id_id256, None, Some(COUNT)),
            Vec<(u32, SignedMintOrder)>
        )
        .await
        .unwrap();

        assert_eq!(orders.len(), COUNT);

        // get with offset
        let orders = canister_call!(
            canister.list_mint_orders(owner.clone(), token_id_id256, Some(10), Some(20)),
            Vec<(u32, SignedMintOrder)>
        )
        .await
        .unwrap();
        assert_eq!(orders.len(), 20);

        // get with offset to the end
        let orders = canister_call!(
            canister.list_mint_orders(owner.clone(), token_id_id256, Some(COUNT - 5), Some(100)),
            Vec<(u32, SignedMintOrder)>
        )
        .await
        .unwrap();
        assert_eq!(orders.len(), 5);

        // get orders with no limit
        let orders = canister_call!(
            canister.list_mint_orders(owner.clone(), token_id_id256, None, None),
            Vec<(u32, SignedMintOrder)>
        )
        .await
        .unwrap();
        assert_eq!(orders.len(), COUNT);

        // get orders with offset but no limit
        let orders = canister_call!(
            canister.list_mint_orders(owner.clone(), token_id_id256, Some(10), None),
            Vec<(u32, SignedMintOrder)>
        )
        .await
        .unwrap();
        assert_eq!(orders.len(), COUNT - 10);

        // get orders for the second token but `owner`
        let orders = canister_call!(
            canister.list_mint_orders(owner, token_id_other_id256, None, None),
            Vec<(u32, SignedMintOrder)>
        )
        .await
        .unwrap();
        assert!(orders.is_empty());

        // get orders for the second token
        let orders = canister_call!(
            canister.list_mint_orders(owner_other.clone(), token_id_other_id256, None, None),
            Vec<(u32, SignedMintOrder)>
        )
        .await
        .unwrap();
        assert_eq!(orders.len(), COUNT_OTHER);
    }
}
