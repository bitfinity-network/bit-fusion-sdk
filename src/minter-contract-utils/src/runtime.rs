pub mod scheduler;
pub mod state;

use std::cell::RefCell;
use std::rc::Rc;

use eth_signer::sign_strategy::TransactionSigner;
use ic_stable_structures::stable_structures::DefaultMemoryImpl;
use ic_stable_structures::{IcMemoryManager, MemoryId, StableBTreeMap, VirtualMemory};

use self::scheduler::BridgeScheduler;
use self::state::{State, StateMemory};
use crate::bridge::{BftResult, Error, Operation, OperationContext};
use crate::evm_bridge::EvmParams;
use crate::evm_link::EvmLink;

pub type Mem = VirtualMemory<DefaultMemoryImpl>;
pub type RuntimeState<Op> = Rc<RefCell<State<Mem, Op>>>;
pub type SharedConfig = state::SharedConfig<Mem>;

pub struct BridgeRuntime<Op: Operation> {
    state: RuntimeState<Op>,
    scheduler: BridgeScheduler<Mem, Op>,
}

impl<Op: Operation> BridgeRuntime<Op> {
    pub fn default(config: SharedConfig) -> Self {
        let tasks_storage = StableBTreeMap::new(memory_by_id(PENDING_TASKS_MEMORY_ID));
        Self {
            state: default_state(config),
            scheduler: BridgeScheduler::new(tasks_storage),
        }
    }

    pub fn update_state(&mut self, f: impl FnOnce(&mut State<Mem, Op>)) {
        let mut state = self.state.borrow_mut();
        f(&mut state);
    }

    pub fn run(&mut self) {
        let task_execution_result = self.scheduler.run(self.state.clone());

        if let Err(err) = task_execution_result {
            log::error!("task execution failed: {err}",);
        }
    }
}

impl<Op: Operation> OperationContext for RuntimeState<Op> {
    fn get_evm_link(&self) -> EvmLink {
        self.borrow().config.borrow().get_evm_link()
    }

    fn get_bridge_contract_address(&self) -> BftResult<did::H160> {
        self.borrow()
            .config
            .borrow()
            .get_bft_bridge_contract()
            .ok_or_else(|| Error::Initialization("bft bridge contract not initialized".into()))
    }

    fn get_evm_params(&self) -> BftResult<EvmParams> {
        self.borrow()
            .config
            .borrow()
            .get_evm_params()
            .ok_or_else(|| Error::Initialization("evm params not initialized".into()))
    }

    fn get_signer(&self) -> impl TransactionSigner {
        self.borrow().signer.get_transaction_signer()
    }
}

pub const SIGNER_MEMORY_ID: MemoryId = MemoryId::new(0);
pub const OPERATIONS_MEMORY_ID: MemoryId = MemoryId::new(1);
pub const OPERATIONS_LOG_MEMORY_ID: MemoryId = MemoryId::new(2);
pub const OPERATIONS_MAP_MEMORY_ID: MemoryId = MemoryId::new(3);
pub const PENDING_TASKS_MEMORY_ID: MemoryId = MemoryId::new(4);

thread_local! {
    pub static MEMORY_MANAGER: IcMemoryManager<DefaultMemoryImpl> = IcMemoryManager::init(DefaultMemoryImpl::default());
}

pub fn memory_by_id(id: MemoryId) -> Mem {
    MEMORY_MANAGER.with(|mm| mm.get(id))
}

fn state_memory() -> StateMemory<Mem> {
    StateMemory {
        signer_memory: memory_by_id(SIGNER_MEMORY_ID),
        incomplete_operations: memory_by_id(OPERATIONS_MEMORY_ID),
        operations_log: memory_by_id(OPERATIONS_LOG_MEMORY_ID),
        operations_map: memory_by_id(OPERATIONS_MAP_MEMORY_ID),
    }
}

fn default_state<Op: Operation>(config: SharedConfig) -> RuntimeState<Op> {
    let state = State::default(state_memory(), config);
    Rc::new(RefCell::new(state))
}
