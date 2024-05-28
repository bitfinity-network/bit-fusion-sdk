use crate::core::deposit::{DefaultRuneDeposit, RuneDeposit};
use crate::interface::DepositError;
use crate::rune_info::RuneName;
use crate::scheduler::{PersistentScheduler, RuneBridgeTask};
use crate::state::{ScheduledDeposit, State};
use candid::CandidType;
use did::{H160, H256};
use ic_exports::ic_kit::ic;
use ic_task_scheduler::scheduler::TaskScheduler;
use ic_task_scheduler::task::TaskOptions;
use minter_did::order::SignedMintOrder;
use serde::Deserialize;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Duration;

pub mod deposit;
pub mod index_provider;
pub mod utxo_provider;
pub mod withdrawal;

pub(crate) struct RuneBridgeCore<D: RuneDeposit = DefaultRuneDeposit> {
    state: Rc<RefCell<State>>,
    scheduler: Rc<RefCell<PersistentScheduler>>,
    deposit: D,
}

#[derive(Debug, Clone, Copy, CandidType, Deserialize)]
pub enum DepositState {
    Scheduled { at_timestamp: u64 },
}

#[derive(Debug, Clone, CandidType, Deserialize)]
pub enum DepositResult {
    MintOrderSigned {
        mint_order: Box<SignedMintOrder>,
        rune_name: RuneName,
        amount: u128,
    },
    MintRequested {
        tx_id: H256,
        rune_name: RuneName,
        amount: u128,
    },
    Minted {
        tx_id: H256,
        rune_name: RuneName,
        amount: u128,
    },
}

impl RuneBridgeCore<DefaultRuneDeposit> {
    pub fn new(state: Rc<RefCell<State>>, scheduler: Rc<RefCell<PersistentScheduler>>) -> Self {
        let deposit = DefaultRuneDeposit::new(state.clone());
        Self {
            state,
            scheduler,
            deposit,
        }
    }
}

impl<D: RuneDeposit> RuneBridgeCore<D> {
    pub fn schedule_deposit(
        &self,
        eth_dst_address: H160,
        amounts: Option<HashMap<RuneName, u128>>,
    ) -> DepositState {
        let task_id = self
            .get_scheduled_deposit(&eth_dst_address)
            .map(|scheduled| scheduled.task_id)
            .unwrap_or_else(|| self.add_deposit_task(eth_dst_address.clone()));

        let scheduled = self.set_scheduled_deposit(eth_dst_address, amounts, task_id);
        DepositState::Scheduled {
            at_timestamp: scheduled.at_timestamp,
        }
    }

    pub async fn run_scheduled_deposit(
        &self,
        eth_address: H160,
    ) -> Result<Vec<DepositResult>, DepositError> {
        let Some(scheduled) = self
            .state
            .borrow_mut()
            .scheduled_deposits_mut()
            .remove(&eth_address)
        else {
            return Err(DepositError::NotScheduled);
        };

        let result = self.deposit.deposit(&eth_address, &scheduled.amounts).await;

        if self.should_retry_deposit(&result, scheduled.at_timestamp) {
            let task_id = self.add_deposit_task(eth_address.clone());
            self.set_scheduled_deposit(eth_address.clone(), scheduled.amounts, task_id);

            log::info!(
                "Deposit request for address {eth_address} was rescheduled with task id {task_id}"
            );
        }

        result
    }

    fn should_retry_deposit(
        &self,
        deposit_result: &Result<Vec<DepositResult>, DepositError>,
        request_ts: u64,
    ) -> bool {
        match deposit_result {
            Err(DepositError::NothingToDeposit)
            | Err(DepositError::NoRunesToDeposit)
            | Err(DepositError::NotEnoughBtc { .. }) => {
                // Probably some or all of the UTXOs sent by the user have not been mined into a
                // block yet. So we want to wait for mempool timeout in this case.
                (ic::time().saturating_sub(request_ts) as u128) < self.mempool_timeout().as_nanos()
            }
            Err(DepositError::Pending { .. }) => {
                // Some of the UTXOs sent by the users are not confirmed yet. Await for enough
                // confirmations. No need for timeout here. If the transaction is dropped without
                // being confirmed, the deposit state will return into mempool checking values.
                true
            }
            _ => false,
        }
    }

    fn mempool_timeout(&self) -> Duration {
        self.state.borrow().mempool_timeout()
    }

    fn deposit_task_options(&self) -> TaskOptions {
        TaskOptions::new()
            .with_max_retries_policy(0)
            .with_execute_after_timestamp_in_secs(ic::time() / 10u64.pow(9))
    }

    fn get_scheduled_deposit(&self, address: &H160) -> Option<ScheduledDeposit> {
        self.state
            .borrow()
            .scheduled_deposits()
            .get(address)
            .cloned()
    }

    fn add_deposit_task(&self, address: H160) -> u32 {
        let task = RuneBridgeTask::Deposit {
            eth_dst_address: address,
        };

        self.scheduler
            .borrow_mut()
            .append_task(task.into_scheduled(self.deposit_task_options()))
    }

    fn set_scheduled_deposit(
        &self,
        address: H160,
        amounts: Option<HashMap<RuneName, u128>>,
        task_id: u32,
    ) -> ScheduledDeposit {
        let scheduled = ScheduledDeposit {
            task_id,
            at_timestamp: ic::time(),
            eth_dst_address: address.clone(),
            amounts,
        };

        self.state
            .borrow_mut()
            .scheduled_deposits_mut()
            .insert(address, scheduled.clone());

        scheduled
    }

    #[cfg(test)]
    fn scheduled_deposits(&self) -> HashMap<H160, ScheduledDeposit> {
        self.state.borrow().scheduled_deposits.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{MEMORY_MANAGER, PENDING_TASKS_MEMORY_ID};
    use crate::scheduler::TasksStorage;
    use ic_exports::ic_kit::MockContext;
    use ic_task_scheduler::scheduler::TaskScheduler;
    use std::str::FromStr;

    fn empty_core() -> RuneBridgeCore<MockDeposit> {
        let _ctx = MockContext::new().inject();

        let state = Rc::new(RefCell::new(State::default()));
        let pending_tasks =
            TasksStorage::new(MEMORY_MANAGER.with(|mm| mm.get(PENDING_TASKS_MEMORY_ID)));
        let scheduler = Rc::new(RefCell::new(PersistentScheduler::new(pending_tasks)));

        RuneBridgeCore {
            state,
            scheduler,
            deposit: MockDeposit {},
        }
    }

    fn core_with_scheduled() -> (RuneBridgeCore<MockDeposit>, H160) {
        let core = empty_core();
        let address = eth_address(1);
        core.schedule_deposit(address.clone(), None);

        (core, address)
    }

    fn eth_address(seed: u8) -> H160 {
        H160::from_slice(&[seed; H160::BYTE_SIZE])
    }

    fn rune_name(name: &str) -> RuneName {
        RuneName::from_str(name).unwrap()
    }

    thread_local! {
        static DEPOSIT_RESULT: RefCell<Result<Vec<DepositResult>, DepositError>> = const { RefCell::new(Err(DepositError::NotScheduled)) };
    }

    struct MockDeposit {}

    impl MockDeposit {
        fn set_result(deposit_result: Result<Vec<DepositResult>, DepositError>) {
            DEPOSIT_RESULT.with(move |v| *v.borrow_mut() = deposit_result)
        }

        fn set_success() {
            Self::set_result(Ok(vec![DepositResult::Minted {
                tx_id: H256::from_slice(&[1; H256::BYTE_SIZE]),
                amount: 1_000_000,
                rune_name: RuneName::from_str("TEST").unwrap(),
            }]));
        }

        fn set_empty() {
            Self::set_result(Err(DepositError::NothingToDeposit));
        }

        fn set_pending() {
            Self::set_result(Err(DepositError::Pending {
                min_confirmations: 12,
                current_confirmations: 6,
            }))
        }
    }

    impl RuneDeposit for MockDeposit {
        async fn deposit(
            &self,
            _eth_address: &H160,
            _amounts: &Option<HashMap<RuneName, u128>>,
        ) -> Result<Vec<DepositResult>, DepositError> {
            DEPOSIT_RESULT.with(|v| v.borrow().clone())
        }
    }

    #[test]
    fn schedule_deposit_adds_tasks_to_scheduler() {
        const TASK_COUNT: usize = 37;

        let core = empty_core();

        for i in 0..TASK_COUNT {
            let address = eth_address(i as u8 + 1);
            let _ = core.schedule_deposit(address.clone(), None);

            let scheduled = core.scheduled_deposits();
            assert_eq!(scheduled.len(), i + 1);
            assert!(matches!(
                core.scheduler
                    .borrow()
                    .get_task(scheduled[&address].task_id)
                    .unwrap()
                    .task(),
                RuneBridgeTask::Deposit {
                    eth_dst_address,
                } if *eth_dst_address == address
            ));
        }
    }

    #[test]
    fn schedule_deposit_does_not_add_duplicate_tasks() {
        const TASK_COUNT: usize = 13;

        let core = empty_core();

        for i in 0..TASK_COUNT {
            let address = eth_address(i as u8 + 1);
            let _ = core.schedule_deposit(address.clone(), None);
        }

        for i in 0..TASK_COUNT {
            let address = eth_address(i as u8 + 1);
            let _ = core.schedule_deposit(address.clone(), None);

            let scheduled = core.scheduled_deposits();
            assert_eq!(scheduled.len(), TASK_COUNT);
        }
    }

    #[test]
    fn schedule_deposit_updates_timestamp_for_duplicate_task() {
        let core = empty_core();

        let ctx = MockContext::new().inject();

        let address = eth_address(1);
        let DepositState::Scheduled {
            at_timestamp: first_ts,
        } = core.schedule_deposit(address.clone(), None);

        ctx.add_time(100);

        let DepositState::Scheduled {
            at_timestamp: second_ts,
        } = core.schedule_deposit(address.clone(), None);

        assert!(second_ts > first_ts);

        assert_eq!(core.scheduled_deposits()[&address].at_timestamp, second_ts);
    }

    #[test]
    fn schedule_deposit_updates_amounts_for_duplicate_task() {
        let (core, address) = core_with_scheduled();

        let amounts: HashMap<RuneName, u128> = [(rune_name("TEST"), 100500)].into_iter().collect();
        core.schedule_deposit(address.clone(), Some(amounts.clone()));

        assert_eq!(core.scheduled_deposits()[&address].amounts, Some(amounts));
    }

    #[test]
    fn schedule_deposit_task_options() {
        let (core, address) = core_with_scheduled();

        let task_id = core.scheduled_deposits()[&address].task_id;
        let task = core
            .scheduler
            .borrow()
            .get_task(task_id)
            .expect("deposit task not found");

        let current_ts = ic::time() / 10u64.pow(9);
        let expected_options = TaskOptions::new()
            .with_max_retries_policy(0)
            .with_execute_after_timestamp_in_secs(current_ts);

        assert_eq!(*task.options(), expected_options);
    }

    #[tokio::test]
    async fn run_scheduled_removes_entry_if_successful() {
        let (core, address) = core_with_scheduled();

        MockDeposit::set_success();
        core.run_scheduled_deposit(address).await.unwrap();

        assert!(core.scheduled_deposits().is_empty());
    }

    #[tokio::test]
    async fn run_scheduled_reschedules_if_possible_mempool_tx() {
        let (core, address) = core_with_scheduled();
        let ctx = MockContext::new().inject();
        MockDeposit::set_empty();

        let task_id_before = core.scheduled_deposits()[&address].task_id;
        core.run_scheduled_deposit(address.clone())
            .await
            .unwrap_err();

        let task_id_after = core.scheduled_deposits()[&address].task_id;
        assert_ne!(task_id_after, task_id_before);
        assert_eq!(core.scheduled_deposits().len(), 1);
        assert!(
            matches!(core.scheduler.borrow().get_task(task_id_after).unwrap().task(), RuneBridgeTask::Deposit { eth_dst_address } if *eth_dst_address == address )
        );

        ctx.add_time(core.state.borrow().mempool_timeout().as_nanos() as u64 / 2);
        core.run_scheduled_deposit(address.clone())
            .await
            .unwrap_err();
        assert_eq!(core.scheduled_deposits().len(), 1);
    }

    #[tokio::test]
    async fn run_scheduled_does_not_reschedules_timed_out_mempoool() {
        let (core, address) = core_with_scheduled();

        let timeout = core.state.borrow().mempool_timeout();
        let ctx = MockContext::new().inject();

        ctx.add_time(timeout.as_nanos() as u64);

        MockDeposit::set_empty();
        core.run_scheduled_deposit(address).await.unwrap_err();

        assert!(core.scheduled_deposits().is_empty());
    }

    #[tokio::test]
    async fn run_scheduled_reschedules_not_confirmed() {
        let (core, address) = core_with_scheduled();

        let timeout = core.state.borrow().mempool_timeout();
        let ctx = MockContext::new().inject();

        ctx.add_time(timeout.as_nanos() as u64 * 2);

        MockDeposit::set_pending();

        let task_id_before = core.scheduled_deposits()[&address].task_id;

        core.run_scheduled_deposit(address.clone())
            .await
            .unwrap_err();

        let task_id_after = core.scheduled_deposits()[&address].task_id;
        assert_ne!(task_id_after, task_id_before);
        assert_eq!(core.scheduled_deposits().len(), 1);
        assert!(
            matches!(core.scheduler.borrow().get_task(task_id_after).unwrap().task(), RuneBridgeTask::Deposit { eth_dst_address } if *eth_dst_address == address )
        );
    }
}
