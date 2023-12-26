use std::fmt;

use candid::CandidType;
use eth_signer::sign_strategy::{
    ManagementCanisterSigner, SigningKeyId, SigningStrategy, TxSigner,
};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{CellStructure, StableCell, StableUnboundedMap, VirtualMemory};
use ic_task_scheduler::scheduler::Scheduler;
use ic_task_scheduler::task::ScheduledTask;
use serde::{Deserialize, Serialize};

pub use self::config::{BridgeSide, Config, ConfigData};
use crate::client::EvmLink;
use crate::memory::{MEMORY_MANAGER, NONCE_MEMORY_ID, PENDING_TASKS_MEMORY_ID, SIGNER_MEMORY_ID};
use crate::tasks::BridgeTask;

mod config;

type TasksStorage =
    StableUnboundedMap<u32, ScheduledTask<BridgeTask>, VirtualMemory<DefaultMemoryImpl>>;
type SignerStorage = StableCell<TxSigner, VirtualMemory<DefaultMemoryImpl>>;
type NonceStorage = StableCell<u32, VirtualMemory<DefaultMemoryImpl>>;

type PersistentScheduler = Scheduler<BridgeTask, TasksStorage>;

pub struct State {
    pub config: Config,
    pub scheduler: PersistentScheduler,
    pub signer: SignerStorage,
    pub nonce: NonceStorage,
}

impl Default for State {
    fn default() -> Self {
        let pending_tasks =
            TasksStorage::new(MEMORY_MANAGER.with(|mm| mm.get(PENDING_TASKS_MEMORY_ID)));

        let default_signer =
            TxSigner::ManagementCanister(ManagementCanisterSigner::new(SigningKeyId::Test, vec![]));
        let signer = SignerStorage::new(
            MEMORY_MANAGER.with(|mm| mm.get(SIGNER_MEMORY_ID)),
            default_signer,
        )
        .expect("failed to initialize transaction signer");

        let nonce = NonceStorage::new(MEMORY_MANAGER.with(|mm| mm.get(NONCE_MEMORY_ID)), 0)
            .expect("failed to initialize nonce storage");

        Self {
            config: Default::default(),
            scheduler: PersistentScheduler::new(pending_tasks),
            signer,
            nonce,
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
        let signer = settings
            .signing_strategy
            .clone()
            .make_signer(0)
            .expect("failed to make signer according to settings");

        self.config.init(settings);
        self.signer.set(signer).expect("failed to set signer");
    }

    pub fn next_nonce(&mut self) -> u32 {
        let next_nonce = *self.nonce.get();
        self.nonce
            .set(next_nonce + 1)
            .expect("failed to store updated nonce");
        next_nonce
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, CandidType)]
pub struct Settings {
    pub base_evm_link: EvmLink,
    pub wrapped_evm_link: EvmLink,
    pub signing_strategy: SigningStrategy,
}
