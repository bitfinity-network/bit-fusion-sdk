pub mod scheduler;
pub mod state;

use std::{cell::RefCell, rc::Rc};

use ic_stable_structures::{
    stable_structures::{DefaultMemoryImpl, Memory},
    VirtualMemory,
};

use self::{scheduler::BridgeScheduler, state::State};

pub type Mem = VirtualMemory<DefaultMemoryImpl>;
pub type RuntimeState = Rc<RefCell<State<Mem>>>;

pub struct BridgeRuntime<Mem: Memory> {
    state: RuntimeState,
    scheduler: BridgeScheduler<Mem>,
}

impl BridgeRuntime {}
