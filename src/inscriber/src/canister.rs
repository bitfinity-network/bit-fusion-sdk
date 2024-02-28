use std::cell::RefCell;
use std::rc::Rc;

use ic_canister::{generate_idl, init, Canister, Idl, PreUpdate};
use ic_exports::candid::Principal;
use ic_metrics::{Metrics, MetricsStorage};

#[derive(Canister, Clone, Debug)]
pub struct Inscriber {
    #[id]
    id: Principal,
}

impl PreUpdate for Inscriber {}

impl Inscriber {
    #[init]
    pub fn init(&self) {
        todo!()
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}

impl Metrics for Inscriber {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
    }
}
