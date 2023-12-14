use std::cell::RefCell;
use std::rc::Rc;

use candid::Principal;
use ic_canister::{generate_idl, init, post_upgrade, Canister, Idl, PreUpdate};
use ic_metrics::{Metrics, MetricsStorage};

use crate::state::{ConfigData, State};

#[derive(Canister, Clone, Debug)]
pub struct SpenderCanister {
    #[id]
    id: Principal,
}

impl PreUpdate for SpenderCanister {}

impl SpenderCanister {
    fn set_timers(&mut self) {
        // Set the metrics updating interval
        #[cfg(target_family = "wasm")]
        {
            self.update_metrics_timer(std::time::Duration::from_secs(60 * 60));
        }
    }

    #[init]
    pub fn init(&mut self, config: ConfigData) {
        State::init(config);
        self.set_timers();
    }

    #[post_upgrade]
    pub fn post_upgrade(&mut self) {
        self.set_timers();
    }

    pub fn idl() -> Idl {
        generate_idl!()
    }
}

impl Metrics for SpenderCanister {
    fn metrics(&self) -> Rc<RefCell<MetricsStorage>> {
        use ic_storage::IcStorage;
        MetricsStorage::get()
    }
}

#[cfg(test)]
mod test {}
