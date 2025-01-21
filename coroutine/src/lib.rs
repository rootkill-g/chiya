use std::{borrow::Cow, fmt, sync::Arc};

use cancel::Cancel;
use done::Done;
use park::Park;

mod builder;
mod cancel;
mod done;
mod event;
mod guard;
mod park;
mod register_context;
mod runtime;
mod spawn;

pub(crate) struct Inner {
    name: Option<Cow<'static, str>>,
    stack_size: usize,
    park: Park,
    cancel: Cancel,
}

#[derive(Clone)]
pub(crate) struct Coroutine {
    inner: Arc<Inner>,
}

impl Coroutine {
    fn new(name: impl Into<Cow<'static, str>>, stack_size: usize) -> Coroutine {
        Coroutine {
            inner: Arc::new(Inner {
                name: Some(name.into()),
                stack_size,
                park: Park::new(),
                cancel: Cancel::new(),
            }),
        }
    }

    // Gets the coroutine stack size
    pub fn stack_size(&self) -> usize {
        self.inner.stack_size
    }

    // Atomically makes the handle's token available if it is not already
    pub fn unpark(&self) {
        self.inner.park.unpark();
    }

    // Cancel a coroutine
    pub unsafe fn cancel(&self) {
        unsafe {
            self.inner.cancel.cancel();
        }
    }

    // Gets the name of the coroutine
    pub fn name(&self) -> Option<&str> {
        self.inner.name.as_deref()
    }

    // Get the internal cancel
    #[cfg(unix)]
    pub(crate) fn get_cancel(&self) -> &Cancel {
        &self.inner.cancel
    }
}

impl fmt::Debug for Coroutine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.name(), f)
    }
}

/// Run the coroutine
pub(crate) fn run_coroutine(mut coroutine: CoroutineImpl) {
    match coroutine.resume() {
        Some(event_subscriber) => event_subscriber.subscribe(coroutine),
        None => {
            // Panic happened here
            let local = unsafe { &mut *get_coroutine_local(&coroutine) };
            let join = local.get_join();

            // Set the panic data
            if let Some(panic) = coroutine.get_panic_data() {
                join.set_panic_data(panic);
            }

            // Trigger the join here
            join.trigger();

            Done::drop_coroutine(coroutine);
        }
    }
}
