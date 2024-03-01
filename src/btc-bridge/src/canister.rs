use std::cell::RefCell;
use std::rc::Rc;

use candid::Principal;
use ic_canister::{generate_idl, init, update, Canister, Idl, PreUpdate};
use ic_metrics::{Metrics, MetricsStorage};

use crate::state::State;

#[derive(Canister, Clone, Debug)]
pub struct BtcBridge {
    #[id]
    id: Principal,
}

impl PreUpdate for BtcBridge {}

impl BtcBridge {
    #[init]
    pub fn init(&mut self) {
        todo!()
    }

    #[update]
    pub async fn update_balance(&self) -> String {
        todo!()
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}

impl Metrics for BtcBridge {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
    }
}

thread_local! {
    pub static STATE: Rc<RefCell<State>> = Rc::default();
}

pub fn get_state() -> Rc<RefCell<State>> {
    STATE.with(|state| state.clone())
}
