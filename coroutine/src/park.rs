use std::{
    ptr,
    sync::{
        Arc,
        atomic::{AtomicBool, AtomicPtr, Ordering},
    },
};

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub enum ParkError {
    Cancelled,
    Timeout,
}

pub struct Park {
    // The coroutine which is waiting for this park instance
    wait_coroutine: Arc<AtomicOption<CoroutineImpl>>,

    // When true - Park doesn't need to block
    state: AtomicBool,

    // Control how to deal with the cancellation
    check_cancel: AtomicBool,

    // Timeout setting in ms, 0 is none (park forever)
    timeout: AtomicDuration,

    // Timer handle - can be null
    timeout_handle: AtomicPtr<TimeoutHandle<Arc<AtomicOption<CoroutineImpl>>>>,

    // A flag if kernel is entered
    wait_kernel: AtomicBool,
}

impl Default for Park {
    fn default() -> Self {
        Park::new()
    }
}

impl Park {
    pub fn new() -> Park {
        Park {
            wait_coroutine: Arc::new(AtomicOption::none()),
            state: AtomicBool::new(false),
            check_cancel: AtomicBool::new(true),
            timeout: AtomicDuration::new(None),
            timeout_handle: AtomicPtr::new(ptr::null_mut()),
            wait_kernel: AtomicBool::new(false),
        }
    }

    // Ignore cancel, if true - caller have to do the check instead
    pub fn ignore_cancel(&self, ignore: bool) {
        self.check_cancel
            .store(!ignore, std::sync::atomic::Ordering::Relaxed);
    }

    // Unpark the underlying coroutine if any, push to the ready task queue
    #[inline]
    pub fn unpark(&self) {
        if !self.state.swap(true, Ordering::AcqRel) {
            self.wake_up(false);
        }
    }

    #[inline]
    fn wake_up(&self, b_sync: bool) {
        if let Some(coroutine) = self.wait_coroutine.take() {
            if b_sync {
                run_coroutine(coroutine);
            } else {
                get_scheduler().schedule(coroutine);
            }
        }
    }
}
