use std::{cell::RefCell, rc::Rc, time::Duration};

use crate::state::State;

const EXPIRED_UTXO_TTL: Duration = Duration::from_secs(60 * 60 * 24 * 3); // 3 days

pub struct RemoveUsedUtxosTask {
    state: Rc<RefCell<State>>,
}

impl From<Rc<RefCell<State>>> for RemoveUsedUtxosTask {
    fn from(state: Rc<RefCell<State>>) -> Self {
        Self { state }
    }
}

impl RemoveUsedUtxosTask {
    pub async fn run(self) {
        let time_now = Duration::from_nanos(ic_exports::ic_cdk::api::time());
        let utxos_to_check = self
            .state
            .borrow()
            .ledger()
            .load_used_utxos()
            .into_iter()
            .filter(|(_, details, _)| {
                Duration::from_nanos(details.used_at) + EXPIRED_UTXO_TTL <= time_now
            })
            .collect::<Vec<_>>();
    }
}
