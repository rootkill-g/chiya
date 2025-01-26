use core::{cell::Cell, fmt};

use crate::sync::primitive::hint;

const SPIN_LIMIT: u32 = 6;
const YIELD_LIMIT: u32 = 10;

/// Performs exponential backoff in spin loops
pub struct Backoff {
    step: Cell<u32>,
}

impl Backoff {
    /// Creates a new `Backoff`
    pub fn new() -> Backoff {
        Backoff { step: Cell::new(0) }
    }

    /// Resets the `Backoff`
    #[inline]
    pub fn reset(&self) {
        self.step.set(0);
    }

    /// Backs off in a lock-free loop
    #[inline]
    pub fn spin(&self) {
        for _ in 0..1 << self.step.get().min(SPIN_LIMIT) {
            hint::spin_loop();
        }

        if self.step.get() <= SPIN_LIMIT {
            self.step.set(self.step.get() + 1);
        }
    }

    /// Backs off in a blocking loop
    #[inline]
    pub fn snooze(&self) {
        if self.step.get() <= SPIN_LIMIT {
            hint::spin_loop();
        } else {
            ::std::thread::yield_now();
        }

        if self.step.get() < YIELD_LIMIT {
            self.step.set(self.step.get() + 1);
        }
    }

    /// Returns `true` if exponential backoff has completed and blocking the thread is advised
    #[inline]
    pub fn is_completed(&self) -> bool {
        self.step.get() > YIELD_LIMIT
    }
}

impl fmt::Debug for Backoff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Backoff")
            .field("step", &self.step)
            .field("is_completed", &self.is_completed())
            .finish()
    }
}

impl Default for Backoff {
    fn default() -> Backoff {
        Backoff::new()
    }
}
