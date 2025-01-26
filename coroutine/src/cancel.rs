use std::{
    io,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use crate::{sync::AtomicOption, unlikely::unlikely, yield_now::get_coroutine_para};

pub trait CancelIo {
    type Data;

    fn new() -> Self;

    #[allow(dead_code)]
    fn set(&self, io_data: Self::Data);

    fn clear(&self);

    unsafe fn cancel(&self) -> Option<io::Result<()>>;
}

pub struct CancelIoImpl;

impl CancelIo for CancelIoImpl {
    type Data = ();

    fn new() -> CancelIoImpl {
        CancelIoImpl
    }

    fn set(&self, _: Self::Data) {}

    fn clear(&self) {}

    unsafe fn cancel(&self) -> Option<io::Result<()>> {
        None
    }
}

/// Each coroutine has it's own Cancel data
pub struct CancelImpl<T: CancelIo> {
    // First bit is used when need to cancel the coroutine
    // Higher bits are used to disable cancel
    state: AtomicUsize,

    // The io data when the coroutine is suspended
    io: T,

    // Other suspended type would register the coroutine itself
    // Can't set io and coroutine at the same time!
    // Most of the time this is park based API
    coroutine: AtomicOption<Arc<AtomicOption<CoroutineImpl>>>,
}

impl<T: CancelIo> Default for CancelImpl<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T: CancelIo> CancelImpl<T> {
    pub fn new() -> Self {
        Self {
            state: AtomicUsize::new(0),
            io: T::new(),
            coroutine: AtomicOption::none(),
        }
    }

    // Check if the coroutine cancel flag is set
    pub fn is_cancelled(&self) -> bool {
        self.state.load(Ordering::Acquire) == 1
    }

    // Check if the coroutine cancel is disabled
    pub fn is_disabled(&self) -> bool {
        self.state.load(Ordering::Acquire) >= 2
    }

    // Disabled the cancel bit
    pub fn disable_cancel(&self) {
        self.state.fetch_add(2, Ordering::Release);
    }

    // Enable the cancel bit
    pub fn enable_cancel(&self) {
        self.state.fetch_sub(2, Ordering::Release);
    }

    // Panic if cancel bit again
    pub fn check_cancel(&self) {
        if unlikely(self.state.load(Ordering::Acquire) == 1) {
            // Before panic clear the last coroutine error
            // This would affect future new coroutine that reuse the instance
            get_coroutine_para();
        }
    }

    // Cancel for coroutine
    #[cold]
    pub unsafe fn cancel(&self) {
        unsafe {
            self.state.fetch_or(1, Ordering::Release);

            if let Some(Ok(())) = self.io.cancel() {
                // Successfully cancelled
                return;
            }

            if let Some(coroutine) = self.coroutine.take() {
                if let Some(mut coroutine) = coroutine.take() {
                    // This is not safe. Kernel may still need to use the overlapped
                    // Set the Cancel result for the coroutine
                    set_coroutine_parameter(
                        &mut coroutine,
                        io::Error::new(io::ErrorKind::Other, "Cancelled"),
                    );
                    get_scheduler().schedule(coroutine);
                }
            }
        }
    }
}

pub type Cancel = CancelImpl<CancelIoImpl>;
