use std::{
    any::Any,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

use crate::sync::AtomicOption;

pub struct Join {
    /// The coroutine thats waiting for this join handler
    pub(crate) to_wake: AtomicOption<Arc<Blocker>>,

    /// The flaf indicate if the host coroutine is not finished
    /// When set to false, the coroutine is done
    pub(crate) state: AtomicBool,

    /// Set the panic error, this communicates with JoinHandle to return panic info
    pub(crate) panic: Arc<AtomicOption<Box<dyn Any + Send>>>,
}

/// Join resource type
impl Join {
    /// Creates new Join
    pub fn new(panic: Arc<AtomicOption<Box<dyn Any + Send>>>) -> Join {
        Join {
            to_wake: AtomicOption::none(),
            state: AtomicBool::new(true),
            panic,
        }
    }

    /// Sets the panic information for the coroutine
    pub fn set_panic_data(&self, panic: Arc<AtomicOption<Box<dyn Any + Send>>>) {
        self.panic.store(panic);
    }

    pub fn trigger(&self) {
        self.state.store(false, Ordering::Release);

        if let Some(blocker) = self.to_wake.take() {
            blocker.unpark()
        }
    }

    pub(crate) fn wait(&self) {
        if self.state.load(Ordering::Acquire) {
            let current_blocker = Blocker::current();

            // Register the blocker first
            self.to_wake.store(current_blocker.clone());

            // Re-check the state
            if self.state.load(Ordering::Acquire) {
                // Successfully register the blocker
                current_blocker.park(None).ok();
            } else {
                self.to_wake.take();
            }
        }
    }
}
