use std::io;

use crate::cancel::Cancel;

pub type EventResult = io::Error;

pub trait EventSource {
    /// Kernel handler of the Event
    fn subscribe(&mut self, coroutine_impl: CoroutineImpl);
    /// After yield back process
    fn yield_back(&self, cancel: &'static Cancel) {
        // After return back we should re-check the panic and clear it
        cancel.check_cancel();
    }
}

pub struct EventSubscriber {
    pub resource: *mut dyn EventSource,
}

unsafe impl Send for EventSubscriber {}
