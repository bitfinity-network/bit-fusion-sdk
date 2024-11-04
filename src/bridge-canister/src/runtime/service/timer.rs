use std::cell::Cell;
use std::time::Duration;

use bridge_did::error::BftResult;
use bridge_did::op_id::OperationId;
use ic_exports::ic_kit::ic;

use super::BridgeService;
use crate::runtime::state::Timestamp;

/// Service decorator to run the inner service with the given delay.
pub struct ServiceTimer<S> {
    inner: S,
    run_after: Cell<Option<Timestamp>>,
    delay: Duration,
}

impl<S> ServiceTimer<S> {
    /// Creates new instance of Self.
    pub fn new(inner: S, delay: Duration) -> Self {
        Self {
            inner,
            run_after: Default::default(),
            delay,
        }
    }

    fn time_to_run(&self, now: Timestamp) -> bool {
        let run_after = self.run_after.get().unwrap_or_default();
        now > run_after
    }
}

#[async_trait::async_trait(?Send)]
impl<S: BridgeService> BridgeService for ServiceTimer<S> {
    async fn run(&self) -> BftResult<()> {
        let now = ic::time();
        if !self.time_to_run(now) {
            return Ok(());
        }

        let run_after_ts = now + self.delay.as_nanos() as u64;
        self.run_after.set(Some(run_after_ts));
        self.inner.run().await
    }

    fn push_operation(&self, id: OperationId) -> BftResult<()> {
        self.inner.push_operation(id)
    }
}
