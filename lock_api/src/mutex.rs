use core::{
    cell::UnsafeCell,
    fmt,
    marker::PhantomData,
    mem,
    ops::{Deref, DerefMut},
};

#[cfg(feature = "arc_lock")]
use alloc::sync::Arc;
#[cfg(feature = "arc_lock")]
use core::mem::ManuallyDrop;
#[cfg(feature = "arc_lock")]
use core::ptr;

#[cfg(feature = "serde")]
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{raw_mutex::RawMutex, raw_mutex_fair::RawMutexFair, raw_mutex_timed::RawMutexTimed};

pub struct Mutex<R, T: ?Sized> {
    pub(crate) raw: R,
    pub(crate) data: UnsafeCell<T>,
}

unsafe impl<R: RawMutex + Send, T: ?Sized + Send> Send for Mutex<R, T> {}
unsafe impl<R: RawMutex + Sync, T: ?Sized + Send> Sync for Mutex<R, T> {}

impl<R: RawMutex, T> Mutex<R, T> {
    /// Creates a new mutex in an unlocked state ready for use
    #[cfg(has_const_fn_trait_bound)]
    #[inline]
    pub const fn new(value: T) -> Mutex<R, T> {
        Mutex {
            raw: R::INIT,
            data: UnsafeCell::new(value),
        }
    }

    /// Creates a new mutex in an unlocked state ready for use
    #[cfg(not(has_const_fn_trait_bound))]
    #[inline]
    pub fn new(value: T) -> Mutex<R, T> {
        Mutex {
            raw: R::INIT,
            data: UnsafeCell::new(value),
        }
    }

    /// Consumes this mutex, returning the underlying data
    #[inline]
    pub fn into_inner(self) -> T {
        self.data.into_inner()
    }
}

impl<R, T> Mutex<R, T> {
    /// Creates a new mutex based on a pre-existing raw mutex
    #[inline]
    pub const fn from_raw(raw_mutex: R, value: T) -> Mutex<R, T> {
        Mutex {
            raw: raw_mutex,
            data: UnsafeCell::new(value),
        }
    }

    /// Creates a new mutex based on a pre-existing raw mutex
    /// This allows creating a mutex in a constant context on stable Rust
    #[inline]
    pub const fn const_new(raw_mutex: R, value: T) -> Mutex<R, T> {
        Self::from_raw(raw_mutex, value)
    }
}

impl<R: RawMutex, T: ?Sized> Mutex<R, T> {
    /// Creates a new `MutexGuard` without checking if the mutex is locked
    // NOTE: This method can only be called if the thread logically holds the lock
    #[inline]
    pub unsafe fn make_guard_unchecked(&self) -> MutexGuard<'_, R, T> {
        unsafe {
            MutexGuard {
                mutex: self,
                marker: PhantomData,
            }
        }
    }

    /// Acquires a mutex, blocking the current thread until it is able to achieve the lock
    #[inline]
    pub fn lock(&self) -> MutexGuard<'_, R, T> {
        self.raw.lock();

        // SAFETY: The lock is held, as required
        unsafe { self.make_guard_unchecked() }
    }

    /// Attempts to acquire this lock
    /// If the lock cannot be acquired at this time, then `None` is returned
    /// Otherwise an RAII guard is returned. The lock will be unlocked when the guard is dropped
    //
    // NOTE: This function does not block
    #[inline]
    pub fn try_lock(&self) -> Option<MutexGuard<'_, R, T>> {
        if self.raw.try_lock() {
            // SAFETY: The lock is held, as required
            Some(unsafe { self.make_guard_unchecked() })
        } else {
            None
        }
    }

    /// Returns a mutable reference to the underlying data
    /// Since this call borrows the `Mutex` mutably, no actual locking need to take place
    /// - The mutable borrow statically guarantees no lock exist
    #[inline]
    pub fn get_mut(&mut self) -> &mut T {
        unsafe { &mut *self.data.get() }
    }

    /// Checks whether the mutex is currently locked
    #[inline]
    pub fn is_locked(&self) -> bool {
        self.raw.is_locked()
    }

    /// Forcibly unlocks the mutex
    #[inline]
    pub unsafe fn force_unlock(&self) {
        unsafe {
            self.raw.unlock();
        }
    }

    /// Returns the underlying raw mutex object
    #[inline]
    pub unsafe fn raw(&self) -> &R {
        &self.raw
    }

    /// Returns the raw pointer to the underlying data
    // NOTE: This is useful when combined with `mem::forget` to hold a lock without the need to
    // maintain a `MutexGuard` object alive, for example when dealing with FFI
    #[inline]
    pub fn data_ptr(&self) -> *mut T {
        self.data.get()
    }

    /// Creates a new `ArcMutexGuard` without checking if the mutex is locked
    #[cfg(feature = "arc_lock")]
    #[inline]
    unsafe fn make_arc_guard_unchecked(self: &Arc<Self>) -> ArcMutexGuard<R, T> {
        ArcMutexGuard {
            mutex: self.clone(),
            marker: PhantomData,
        }
    }

    /// Acquires a lock through an `Arc`
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn lock_arc(self: &Arc<Self>) -> ArcMutexGuard<R, T> {
        self.raw.lock();

        // SAFETY: the locking guarantee is upheld
        unsafe { self.make_arc_guard_unchecked() }
    }
}

impl<R: RawMutexFair, T: ?Sized> Mutex<R, T> {
    /// Forcibly unlocks the mutex using a fair unlock protocol
    // NOTE: This is useful when combined with `mem::forget` to hold a lock without the need to
    // maintain a `MutexGuard` object alive, for example when dealing with FFI
    #[inline]
    pub unsafe fn force_unlock_fair(&self) {
        unsafe {
            self.raw.unlock_fair();
        }
    }
}

impl<R: RawMutexTimed, T: ?Sized> Mutex<R, T> {
    /// Attempts to acquire this lock until a timeout is reached
    #[inline]
    pub fn try_lock_for(&self, timeout: R::Duration) -> Option<MutexGuard<'_, R, T>> {
        if self.raw.try_lock_for(timeout) {
            // SAFETY: The lock is held, as required
            Some(unsafe { self.make_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to acquire this lock until a timeout is reached
    #[inline]
    pub fn try_lock_until(&self, timeout: R::Instant) -> Option<MutexGuard<'_, R, T>> {
        if self.raw.try_lock_until(timeout) {
            // SAFETY: The lock is held, as required
            Some(unsafe { self.make_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to acquire this lock through an `Arc` until a timeout is reached
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_lock_arc_for(
        self: &Arc<Self>,
        timeout: R::Duration,
    ) -> Option<ArcMutexGuard<'_, R, T>> {
        if self.raw.try_lock_for(timeout) {
            // SAFETY: Locking guarantee is upheld
            Some(unsafe { self.make_arc_guard_unchecked() })
        } else {
            None
        }
    }

    /// Attempts to acquire this lock through an `Arc` until a timeout is reached
    #[cfg(feature = "arc_lock")]
    #[inline]
    pub fn try_lock_arc_until(
        self: &Arc<Self>,
        timeout: R::Instant,
    ) -> Option<ArcMutexGuard<'_, R, T>> {
        if self.raw.try_lock_until(timeout) {
            // SAFETY: Locking guarantee is upheld
            Some(unsafe { self.make_arc_guard_unchecked() })
        } else {
            None
        }
    }
}
