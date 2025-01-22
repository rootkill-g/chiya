use std::{
    any::Any,
    sync::{Arc, atomic::Ordering},
    thread::Result,
};

use crate::{Coroutine, join::Join, runtime::Error};

/// JoinHandle for Coroutine
pub struct JoinHandle<T> {
    coroutine: Coroutine,
    join: Arc<Join>,
    packet: Arc<AtomicOption<T>>,
    panic: Arc<AtomicOption<Box<dyn Any + Send>>>,
}

unsafe impl<T: Send> Send for JoinHandle<T> {}
unsafe impl<T: Sync> Sync for JoinHandle<T> {}

/// Create a JoinHandle
pub fn make_join_handle<T>(
    coroutine: Coroutine,
    join: Arc<Join>,
    packet: Arc<AtomicOption<T>>,
    panic: Arc<AtomicOption<Box<dyn Any + Send>>>,
) -> JoinHandle<T> {
    JoinHandle {
        coroutine,
        join,
        packet,
        panic,
    }
}

impl<T> JoinHandle<T> {
    /// Returns a reference to the underlying coroutine
    pub fn coroutine(&self) -> &Coroutine {
        &self.coroutine
    }

    /// Return true if the coroutine is finished
    pub fn is_done(&self) -> bool {
        !self.join.state.load(Ordering::Acquire)
    }

    /// Block until the coroutine is done
    pub fn wait(&self) {
        self.join.wait();
    }

    /// Join the coroutine, returning the result produced
    pub fn join(self) -> Result<T> {
        self.join.wait();

        // Take the result
        self.packet
            .take()
            .ok_or_else(|| self.panic.take().unwrap_or_else(|| Box::new(Error::Cancel)))
    }
}
