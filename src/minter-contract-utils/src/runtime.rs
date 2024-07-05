pub mod scheduler;
pub mod state;

use self::state::State;

pub struct BridgeRuntime<Mem: Memory> {
    state: State<Mem>,
    scheduler: BridgeScheduler<Mem>,
}

impl BridgeRuntime {}
