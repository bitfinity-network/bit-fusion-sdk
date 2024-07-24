use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;

use super::State;
use crate::bridge::Operation;

pub const TASK_LOCK_TIMEOUT: Duration = Duration::from_secs(60);

type OnDropHook<Op> = Box<dyn FnOnce(&mut State<Op>)>;

/// Task lock.
///
/// This struct provides a way to acquire a lock on a `State` object, and ensure that a specific function is called when the lock is released.
///
pub struct TaskLock<Op: Operation> {
    state: Rc<RefCell<State<Op>>>,

    /// Hook that will be called when the lock is dropped.
    on_drop: Option<OnDropHook<Op>>,
}

impl<Op: Operation> TaskLock<Op> {
    /// Creates a new `TaskLock` object that acquires a lock on the given `state` object, and calls the given `on_drop` function when the lock is released.
    ///
    /// # Arguments
    ///
    /// * `state` - The `State` object to acquire a lock on.
    /// * `on_drop` - A function that will be called when the lock is released. The function will be called with a mutable reference to the `State` object.
    pub fn new(state: Rc<RefCell<State<Op>>>, on_drop: Option<OnDropHook<Op>>) -> Self {
        TaskLock { state, on_drop }
    }
}

impl<Op: Operation> Drop for TaskLock<Op> {
    /// Calls the `on_drop` function when the `TaskLock` object is dropped, and releases the lock on the `State` object.
    fn drop(&mut self) {
        let mut state = self.state.borrow_mut();

        if let Some(on_drop) = self.on_drop.take() {
            (on_drop)(&mut state);
        }
    }
}

#[cfg(test)]
mod tests {
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
        let state: Rc<RefCell<State<TestOp>>> = default_state(Rc::new(RefCell::new(
            ConfigStorage::default(memory_by_id(MemoryId::new(10))),
        )));

        let drop_called = Rc::new(RefCell::new(false));
        {
            let drop_called = drop_called.clone();
            let on_drop = move |_: &mut State<TestOp>| {
                *drop_called.borrow_mut() = true;
            };
            let _task_lock = TaskLock::new(state.clone(), Some(Box::new(on_drop)));
        }

        assert!(*drop_called.borrow());
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
