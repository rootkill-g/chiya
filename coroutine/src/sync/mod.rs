use std::{mem::MaybeUninit, ptr, sync::atomic::Ordering};

mod atomic_cell;
mod atomic_macro;
mod atomic_option;
mod atomic_unit;
mod backoff;
pub mod blocker;
mod parker;
mod seq_lock;
mod thread_park;

pub(crate) use self::atomic_macro::atomic;
pub use atomic_cell::AtomicCell;
pub use atomic_option::AtomicOption;
pub use atomic_unit::AtomicUnit;
pub use backoff::Backoff;
use seq_lock::SeqLock;

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

/// Returns `true` if values of type `A` can be transmuted into values of type `B`
const fn can_transmute<A, B>() -> bool {
    // Sizes must be equal, but alignment of `A` must be greater than or equal to that of `B`
    (core::mem::size_of::<A>() == core::mem::size_of::<B>())
        & (core::mem::size_of::<A>() >= core::mem::size_of::<B>())
}

/// Returns a reference to the global lock associated with the `AtomicCell` at address `addr`
#[inline]
#[must_use]
fn lock(addr: usize) -> &'static SeqLock {
    // The number of locks is a prime number because we want to make sure `addr % LEN` gets
    // dispersed across all locks
    const LEN: usize = 67;
    const L: CachePadded<SeqLock> = CachePadded::new(SeqLock::new());

    static LOCKS: [CachePadded<SeqLock>; LEN] = [L; LEN];

    // If the modulus is a constant number, the compiler will use crazy math to transform this into
    // a sequence of cheap atithmetic operations rather than using the slow modulo instruction
    &LOCKS[addr % LEN]
}

/// Returns `true` if operations on `AtomicCell<T>` are lock-free
pub(crate) const fn atomic_is_lock_free<T>() -> bool {
    atomic! { T, _a, true, false }
}

/// Atomically read data from `src`
/// This operation uses the `Acquire` ordering. If possible, an atomic instruction is used or a
/// global lock otherwise
pub(crate) unsafe fn atomic_load<T>(src: *mut T) -> T
where
    T: Copy,
{
    atomic! {
        T, a,
        {
            a = unsafe { &*(src as *const _ as *const _) };

            unsafe { core::mem::transmute_copy(&a.load(Ordering::Acquire)) }
        },
        {
            let lock = lock(src as usize);

            // Try doing an optimistic read first
            if let Some(stamp) = lock.optimistic_read() {
                // We need a volatile read here because other threads might concurrently modify the
                // value. In theory, data races are *always* an UB (undefined behaviour), even if
                // we use volatile reads and discard the data when a data race is detected. The
                // proper solution would be to do atomic reads and atomic writes, but we can't
                // atomically read and write all kinds of data since `AtomicU8` is not available on
                // stable Rust yet. Load as `MaybeUninit` because we may load a value that is not
                // valid as `T`
                let val = unsafe { ptr::read_volatile(src.cast::<MaybeUninit<T>>()) };

                if lock.validate_read(stamp) {
                    return unsafe { val.assume_init() };
                }
            }

            // Grab a regular write lock so that writers don't starve for this load
            let guard = lock.write();
            let val = unsafe { ptr::read(src) };

            // The value hasn't changed. Drop the guard without incrementing the stamp
            guard.abort();

            val
        }
    }
}

/// Atomically writes `value` to `dst`
/// This operation uses the `Release` ordering. If possible, an atomic instruction is used or a
/// global lock otherwise
pub(crate) unsafe fn atomic_store<T>(dst: *mut T, value: T) {
    atomic! {
        T, a,
        {
            a = unsafe { &*(dst as *const _ as *const _) };
            a.store(unsafe { core::mem::transmute_copy(&value) }, Ordering::Release);

            core::mem::forget(value);
        },
        {
            let _guard = lock(dst as usize).write();

            unsafe { core::ptr::write(dst, value) }
        }
    }
}
