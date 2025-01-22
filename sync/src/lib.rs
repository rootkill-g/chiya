mod atomic_cell;
mod atomic_option;
mod backoff;
mod seq_lock;

pub use atomic_cell::AtomicCell;
pub use atomic_option::AtomicOption;
pub use backoff::Backoff;

#[allow(unused_imports)]
mod primitive {
    pub(crate) mod hint {
        pub(crate) use core::hint::spin_loop;
    }

    pub(crate) mod sync {
        pub(crate) use core::sync::atomic;
        pub(crate) use std::sync::{Arc, Condvar, Mutex};
    }
}
