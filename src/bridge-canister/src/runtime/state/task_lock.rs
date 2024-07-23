use std::time::Duration;

use ic_exports::ic_kit::ic;

const TASK_LOCK_TIMEOUT: Duration = Duration::from_secs(60);

/// This lock is used to prevent several tasks(updating evm params, and collecting logs) from running at the same time. When the
/// task starts, it takes the lock and holds it until the evm params are updates, so no other
/// task would receive logs from the same block numbers.
///
/// To prevent the lock to get stuck locked in case of panic after an async call, we set the timeout
/// of 1 minute, after which the lock is released even if the task didn't release it.
#[derive(Default)]
pub struct TaskLock {
    last_execution: Option<u64>,
}

impl TaskLock {
    /// Attempts to acquire a lock for the task. If the lock is already held and the timeout has not expired,
    /// this function will return `None`. Otherwise, it will update the `last_execution` timestamp and return
    /// the new timestamp.

    pub fn try_lock(&mut self) -> Option<Self> {
        match self.last_execution {
            Some(last_execution)
                if (last_execution + TASK_LOCK_TIMEOUT.as_nanos() as u64) >= ic::time() =>
            {
                None
            }

            _ => {
                let ts = ic::time();
                self.last_execution = Some(ts);
                Some(TaskLock {
                    last_execution: Some(ts),
                })
            }
        }
    }
}

impl Drop for TaskLock {
    fn drop(&mut self) {
        let curr = self.last_execution;
        if let Some(ts) = curr {
            if ts <= ic::time() {
                self.last_execution = None;
            }
        }
    }
}
