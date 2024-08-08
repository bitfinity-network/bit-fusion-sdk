use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use super::State;
use crate::bridge::Operation;

pub const TASK_LOCK_TIMEOUT: Duration = Duration::from_secs(60);

type OnDropHook<S> = Box<dyn FnOnce(&mut S)>;

/// Task lock.
///
/// This struct provides a way to acquire a lock on a `State` object, and ensure that a specific function is called when the lock is released.
///
pub struct TaskLock<S> {
    state: S,

    /// Hook that will be called when the lock is dropped.
    on_drop: Option<OnDropHook<S>>,
}

impl<S> TaskLock<S> {
    /// Creates a new `TaskLock` object that acquires a lock on the given `state` object, and calls the given `on_drop` function when the lock is released.
    ///
    /// # Arguments
    ///
    /// * `state` - The `State` object to acquire a lock on.
    /// * `on_drop` - A function that will be called when the lock is released. The function will be called with a mutable reference to the `State` object.
    pub fn new(state: S, on_drop: OnDropHook<S>) -> Self {
        TaskLock {
            state,
            on_drop: Some(on_drop),
        }
    }
}

impl<S> Drop for TaskLock<S> {
    /// Calls the `on_drop` function when the `TaskLock` object is dropped, and releases the lock on the `State` object.
    fn drop(&mut self) {
        if let Some(on_drop) = self.on_drop.take() {
            (on_drop)(&mut self.state);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;

    use ic_exports::ic_kit::{ic, MockContext};
    use ic_stable_structures::MemoryId;

    use super::*;
    use crate::memory::memory_by_id;
    use crate::runtime::default_state;
    use crate::runtime::state::config::ConfigStorage;
    use crate::runtime::state::tests::TestOp;

    #[test]
    fn test_task_lock_drop() {
        MockContext::new().inject();
        let state = Rc::new(AtomicBool::default());
        {
            let _lock = TaskLock::new(
                state.clone(),
                Box::new(|s: &mut Rc<AtomicBool>| s.store(true)),
            );
        }
    }

    #[test]
    fn test_task_lock_drop_with_refresh_evm_params() {
        MockContext::new().inject();

        let state: Rc<RefCell<State<TestOp>>> = default_state(Rc::new(RefCell::new(
            ConfigStorage::default(memory_by_id(MemoryId::new(5))),
        )));

        state.borrow_mut().refreshing_evm_params_ts = Some(ic::time());

        {
            let on_drop = |state: &mut State<TestOp>| {
                state.refreshing_evm_params_ts = None;
            };
            let _task_lock = TaskLock::new(state.clone(), Some(Box::new(on_drop)));
        }

        assert_eq!(state.borrow().refreshing_evm_params_ts, None);
    }

    #[test]
    fn test_task_lock_multiple_drops() {
        MockContext::new().inject();

        let state: Rc<RefCell<State<TestOp>>> = default_state(Rc::new(RefCell::new(
            ConfigStorage::default(memory_by_id(MemoryId::new(5))),
        )));

        let drop_count = Rc::new(RefCell::new(0));
        {
            let drop_count_clone = drop_count.clone();
            let on_drop = move |_state: &mut State<TestOp>| {
                *drop_count_clone.borrow_mut() += 1;
            };
            let _task_lock = TaskLock::new(state.clone(), Some(Box::new(on_drop)));
        }
        assert_eq!(*drop_count.borrow(), 1);
    }
}
