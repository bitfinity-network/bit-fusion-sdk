use std::fmt;

use candid::CandidType;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{StableUnboundedMap, VirtualMemory};
use ic_task_scheduler::scheduler::Scheduler;
use ic_task_scheduler::task::ScheduledTask;
use serde::{Serialize, Deserialize};

use crate::client::EvmLink;
use crate::memory::{MEMORY_MANAGER, PENDING_TASKS_MEMORY_ID};
use crate::tasks::PersistentTask;

pub use self::config::{Config, ConfigData, BridgeSide};

mod config;

type Storage =
    StableUnboundedMap<u32, ScheduledTask<PersistentTask>, VirtualMemory<DefaultMemoryImpl>>;

type PersistentScheduler = Scheduler<PersistentTask, Storage>;

pub struct State {
    pub config: Config,
    pub scheduler: PersistentScheduler,
}

impl Default for State {
    fn default() -> Self {
        let pending_tasks = Storage::new(MEMORY_MANAGER.with(|mm| mm.get(PENDING_TASKS_MEMORY_ID)));
        Self {
            config: Default::default(),
            scheduler: PersistentScheduler::new(pending_tasks),
        }
    }
}

impl fmt::Debug for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("State")
            .field("config", &self.config)
            .field("scheduler", &"PersistentScheduler")
            .finish()
    }
}

impl State {
    pub fn init(&mut self, settings: Settings) {
        self.config.init(settings);
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, CandidType)]
pub struct Settings {
    pub base_evm_link: EvmLink,
    pub wrapped_evm_link: EvmLink,
}