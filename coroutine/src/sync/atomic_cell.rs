use core::{
    cell::UnsafeCell,
    fmt,
    mem::{ManuallyDrop, MaybeUninit},
};
use std::sync::atomic::{self, Ordering};

use super::{
    AtomicUnit, atomic, atomic_is_lock_free, atomic_load, atomic_store, can_transmute, lock,
};

#[repr(transparent)]
pub struct AtomicCell<T> {
    value: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T: Send> Send for AtomicCell<T> {}
unsafe impl<T: Sync> Sync for AtomicCell<T> {}

impl<T> AtomicCell<T> {
    /// Creates a new atomic cell initialized with `value`
    pub const fn new(value: T) -> AtomicCell<T> {
        AtomicCell {
            value: UnsafeCell::new(MaybeUninit::new(value)),
        }
    }

    /// Consumes the atomic cell and returns the underlying value
    pub fn into_inner(self) -> T {
        let this = ManuallyDrop::new(self);

        // SAFETY:
        // - Passing `self` by value guarantees that no other threads are concurrently accessing
        // the atomic data
        // - The raw pointer passed in is valid because we got it from an owned value
        // - `ManuallyDrop` prevents double dropping of `T`
        unsafe { this.as_ptr().read() }
    }

    /// Returns `true` if the operations on values of this type are lock-free
    pub const fn is_lock_free() -> bool {
        atomic_is_lock_free::<T>()
    }

    /// Stores `value` into the atomic cell
    pub fn store(&self, value: T) {
        if std::mem::needs_drop::<T>() {
            drop(self.swap(value));
        } else {
            unsafe { atomic_store(self.as_ptr(), value) };
        }
    }

    /// Stores `value` into the atomic cell and returns the previous value
    pub fn swap(&self, value: T) -> T {
        unsafe { atomic_swap(self.as_ptr(), value) }
    }

    /// Returns a raw pointer to the underlying data in this atomic cell
    #[inline]
    pub fn as_ptr(&self) -> *mut T {
        self.value.get().cast::<T>()
    }
}

impl<T: Default> AtomicCell<T> {
    /// Takes the value of the atomic cell, leaving `Default::default()` in its place
    pub fn take(&self) -> T {
        self.swap(Default::default())
    }
}

impl<T: Copy> AtomicCell<T> {
    /// Loads a value from the atomic cell
    pub fn load(&self) -> T {
        unsafe { atomic_load(self.as_ptr()) }
    }
}

impl<T: Copy + Eq> AtomicCell<T> {
    /// If the current value equals `current`, stores `new` into the atomic cell
    pub fn compare_exchange(&self, current: T, new: T) -> Result<T, T> {
        unsafe { atomic_compare_exchange_weak(self.as_ptr(), current, new) }
    }

    /// Fetches the value and applies a function to it that returns an optional new value
    /// Returns a `Result` of `Ok(previous_value)` if the function returned `Some(_)`, else
    /// `Err(previous_value)`
    pub fn fetch_update<F>(&self, mut f: F) -> Result<T, T>
    where
        F: FnMut(T) -> Option<T>,
    {
        let mut previous = self.load();

        while let Some(next) = f(previous) {
            match self.compare_exchange(previous, next) {
                x @ Ok(_) => return x,
                Err(next_previous) => previous = next_previous,
            }
        }

        Err(previous)
    }
}

/// `MaybeUninit` prevents `T` from being dropped, so we need to implement `Drop` for `AtomicCell`
/// to avoid leaks of non-`Copy` types
impl<T> Drop for AtomicCell<T> {
    fn drop(&mut self) {
        if std::mem::needs_drop::<T>() {
            // SAFETY:
            // - The mutable references guarantees that no other threads are concurrently accessing
            // the atomic data
            // - The raw pointer passed in is valid because we got it from a reference
            // - `MaybeUninit` prevents double dropping of `T`
            unsafe { self.as_ptr().drop_in_place() };
        }
    }
}

impl AtomicCell<bool> {
    /// Applies logical `and` to the current value and returns the previous value
    #[inline]
    pub fn fetch_and(&self, value: bool) -> bool {
        atomic! {
            bool, _a,
            {
                let a = unsafe { &*(self.as_ptr() as *const atomic::AtomicBool) };

                a.fetch_and(value, Ordering::AcqRel)
            },
            {
                let _guard = lock(self.as_ptr() as usize).write();
                let val = unsafe { &mut *(self.as_ptr()) };
                let old = *val;

                *val &= value;

                old
            }
        }
    }

    /// Applies logical `nand` to the current value and returns the previous value
    #[inline]
    pub fn fetch_nand(&self, value: bool) -> bool {
        atomic! {
            bool, _a,
            {
                let a = unsafe { &*(self.as_ptr() as *const atomic::AtomicBool) };

                a.fetch_nand(value, Ordering::AcqRel)
            },
            {
                let _guard = lock(self.as_ptr() as usize).write();
                let val = unsafe { &mut *(self.as_ptr()) };
                let old = *val;

                *val = !(old & value);

                old
            }
        }
    }

    /// Applies logical `or` to the current value and returns the previous value
    #[inline]
    pub fn fetch_or(&self, value: bool) -> bool {
        atomic! {
            bool, _a,
            {
                let a = unsafe { &*(self.as_ptr() as *const atomic::AtomicBool) };

                a.fetch_or(value, Ordering::AcqRel)
            },
            {
                let _guard = lock(self.as_ptr() as usize).write();
                let val = unsafe { &mut *(self.as_ptr()) };
                let old = *val;

                *val |= value;

                old
            }
        }
    }

    /// Applies logical `xor` to the current value and returns the previous value
    #[inline]
    pub fn fetch_xor(&self, value: bool) -> bool {
        atomic! {
            bool, _a,
            {
                let a = unsafe { &*(self.as_ptr() as *const atomic::AtomicBool) };

                a.fetch_xor(value, Ordering::AcqRel)
            },
            {
                let _guard = lock(self.as_ptr() as usize).write();
                let val = unsafe { &mut *(self.as_ptr()) };
                let old = *val;

                *val ^= value;

                old
            }
        }
    }
}

impl<T: Default> Default for AtomicCell<T> {
    fn default() -> AtomicCell<T> {
        AtomicCell::new(T::default())
    }
}

impl<T> From<T> for AtomicCell<T> {
    #[inline]
    fn from(value: T) -> AtomicCell<T> {
        AtomicCell::new(value)
    }
}

impl<T: Copy + fmt::Debug> fmt::Debug for AtomicCell<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AtomicCell")
            .field("value", &self.load())
            .finish()
    }
}
