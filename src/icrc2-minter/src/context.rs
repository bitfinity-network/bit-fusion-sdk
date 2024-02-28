use std::cell::{Ref, RefCell, RefMut};
use std::rc::Rc;

use crate::evm::EvmCanister;
use crate::state::{Settings, State};
use crate::tokens::evm::EvmTokensService;

/// Context to access the external traits
pub trait Context {
    /// Return a client to the EVM canister
    fn get_evm_canister(&self) -> Rc<dyn EvmCanister>;

    /// Returns EVM tokens service
    fn get_evm_token_service(&self) -> Rc<EvmTokensService>;

    /// Returns state reference
    fn get_state(&self) -> Ref<'_, State>;

    /// Returns mutable state reference
    fn mut_state(&self) -> RefMut<'_, State>;

    /// Resets context state to the default one
    fn reset(&mut self) {
        self.mut_state().reset(Settings::default());
        self.get_evm_canister().reset();
    }
}

#[derive(Default)]
pub struct ContextImpl<Evm: EvmCanister> {
    evm: Rc<Evm>,
    evm_token_service: Rc<EvmTokensService>,
    state: RefCell<State>,
}

impl<EvmImpl: EvmCanister + 'static> Context for ContextImpl<EvmImpl> {
    fn get_evm_canister(&self) -> Rc<dyn EvmCanister> {
        self.evm.clone()
    }

    fn get_evm_token_service(&self) -> Rc<EvmTokensService> {
        self.evm_token_service.clone()
    }

    fn get_state(&self) -> Ref<'_, State> {
        self.state.borrow()
    }

    fn mut_state(&self) -> RefMut<'_, State> {
        self.state.borrow_mut()
    }
}

pub fn get_base_context(context: &Rc<RefCell<impl Context + 'static>>) -> Rc<RefCell<dyn Context>> {
    let context: Rc<RefCell<dyn Context>> = context.clone();
    context
}
