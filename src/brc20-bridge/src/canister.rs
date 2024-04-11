use std::cell::RefCell;
use std::rc::Rc;

use candid::Principal;
use ic_canister::{generate_idl, init, Canister, Idl, PreUpdate};
use ic_metrics::{Metrics, MetricsStorage};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{StableUnboundedMap, VirtualMemory};
use ic_task_scheduler::scheduler::Scheduler;
use ic_task_scheduler::task::ScheduledTask;

use crate::memory::{MEMORY_MANAGER, PENDING_TASKS_MEMORY_ID};
use crate::scheduler::Brc20Task;
use crate::state::State;

#[derive(Canister, Clone, Debug)]
pub struct Brc20Bridge {
    #[id]
    id: Principal,
}

impl PreUpdate for Brc20Bridge {}

impl Brc20Bridge {
    #[init]
    pub fn init(&mut self) {
        todo!()
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}

impl Metrics for Brc20Bridge {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
    }
}

pub type TasksStorage =
    StableUnboundedMap<u32, ScheduledTask<Brc20Task>, VirtualMemory<DefaultMemoryImpl>>;

pub type PersistentScheduler = Scheduler<Brc20Task, TasksStorage>;

thread_local! {
    pub static STATE: Rc<RefCell<State>> = Rc::default();

    pub static SCHEDULER: Rc<RefCell<PersistentScheduler>> = Rc::new(RefCell::new({
        let pending_tasks =
            TasksStorage::new(MEMORY_MANAGER.with(|mm| mm.get(PENDING_TASKS_MEMORY_ID)));
            PersistentScheduler::new(pending_tasks)
    }));
}

pub fn get_state() -> Rc<RefCell<State>> {
    STATE.with(|state| state.clone())
}

pub fn get_scheduler() -> Rc<RefCell<PersistentScheduler>> {
    SCHEDULER.with(|scheduler| scheduler.clone())
}
