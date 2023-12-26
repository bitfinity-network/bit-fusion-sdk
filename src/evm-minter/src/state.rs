use std::fmt;

use candid::CandidType;
use eth_signer::sign_strategy::{
    ManagementCanisterSigner, SigningKeyId, SigningStrategy, TxSigner,
};
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{CellStructure, StableCell, StableUnboundedMap, VirtualMemory};
use ic_task_scheduler::scheduler::Scheduler;
use ic_task_scheduler::task::ScheduledTask;
use minter_contract_utils::mint_orders::MintOrders;
use serde::{Deserialize, Serialize};

pub use self::config::{BridgeSide, Config, ConfigData};
use crate::client::EvmLink;
use crate::memory::{
    MEMORY_MANAGER, MINT_ORDERS_MEMORY_ID, PENDING_TASKS_MEMORY_ID, SIGNER_MEMORY_ID,
};
use crate::tasks::BridgeTask;

mod config;

type TasksStorage =
    StableUnboundedMap<u32, ScheduledTask<BridgeTask>, VirtualMemory<DefaultMemoryImpl>>;
type SignerStorage = StableCell<TxSigner, VirtualMemory<DefaultMemoryImpl>>;

type PersistentScheduler = Scheduler<BridgeTask, TasksStorage>;

pub struct State {
    pub config: Config,
    pub scheduler: PersistentScheduler,
    pub signer: SignerStorage,
    pub mint_orders: MintOrders<VirtualMemory<DefaultMemoryImpl>>,
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

        let mint_orders = MintOrders::new(MEMORY_MANAGER.with(|mm| mm.get(MINT_ORDERS_MEMORY_ID)));

        Self {
            config: Default::default(),
            scheduler: PersistentScheduler::new(pending_tasks),
            signer,
            mint_orders,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, CandidType)]
pub struct Settings {
    pub base_evm_link: EvmLink,
    pub wrapped_evm_link: EvmLink,
    pub signing_strategy: SigningStrategy,
}
