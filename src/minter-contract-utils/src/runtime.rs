pub mod scheduler;
pub mod state;

use std::{cell::RefCell, rc::Rc};

use eth_signer::sign_strategy::TransactionSigner;
use ic_stable_structures::{stable_structures::DefaultMemoryImpl, VirtualMemory};

use crate::{
    bridge::{BftResult, Error, Operation, OperationContext},
    evm_bridge::EvmParams,
    evm_link::EvmLink,
};

use self::{scheduler::BridgeScheduler, state::State};

pub type Mem = VirtualMemory<DefaultMemoryImpl>;
pub type RuntimeState<Op> = Rc<RefCell<State<Mem, Op>>>;

pub struct BridgeRuntime<Op: Operation> {
    state: RuntimeState<Op>,
    scheduler: BridgeScheduler<Mem, Op>,
}

impl<Op: Operation> OperationContext for RuntimeState<Op> {
    fn get_evm_link(&self) -> EvmLink {
        self.borrow().config.get_evm_link()
    }

    fn get_bridge_contract_address(&self) -> BftResult<did::H160> {
        self.borrow()
            .config
            .get_bft_bridge_contract()
            .ok_or_else(|| Error::Initialization("bft bridge contract not initialized".into()))
    }

    fn get_evm_params(&self) -> BftResult<EvmParams> {
        self.borrow()
            .config
            .get_evm_params()
            .ok_or_else(|| Error::Initialization("evm params not initialized".into()))
    }

    fn get_signer(&self) -> impl TransactionSigner {
        self.borrow().signer.get_transaction_signer()
    }
}
