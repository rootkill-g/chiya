use core::{marker::PhantomData, mem};

use crate::{mutex::Mutex, raw_mutex::RawMutex};

/// An RAII implementation of a "scoped lock" of a mutex. When this structure is dropeed (falls out
/// of scope), the lock will be unlocked
#[clippy::has_significant_drop]
#[must_use = "if unused the Mutex will immediately unlock"]
pub struct MutexGuard<'a, R: RawMutex, T: ?Sized> {
    mutex: &'a Mutex<R, T>,
    marker: PhantomData<(&'a mut T, R::GuardMarker)>,
}

unsafe impl<'a, R: RawMutex + Sync + 'a, T: ?Sized + Sync + 'a> Sync for MutexGuard<'a, R, T> {}

impl<'a, R: RawMutex + 'a, T: ?Sized + 'a> MutexGuard<'a, R, T> {
    /// Returns a reference to the original `Mutex` object
    pub fn mutex(s: &Self) -> &'a Mutex<R, T> {
        s.mutex
    }

    /// Makes a new `MappedMutexGuard` for a component of the locked data
    #[inline]
    pub fn map<U: ?Sized, F>(s: Self, f: F) -> MappedMutexGuard<'a, R, U>
    where
        F: FnOnce(&mut T) -> &mut U,
    {
        let raw = &s.mutex.raw;
        let data = f(unsafe { &mut *s.mutex.data.get() });

        mem::forget(s);

        MappedMutexGuard {
            raw,
            data,
            marker: PhantomData,
        }
    }

    /// Attempts to make a new `MappedMutexGuard` for a component of the locked data. The original
    /// guard is returned if the closure returns `None`.
    // NOTE: This operation cannot fail as the `MutexGuard` passed
    #[inline]
    pub fn try_map<U: ?Sized, F>(s: Self, f: F) -> Result<MappedMutexGuard<'a, R, U>, Self>
    where
        F: FnOnce(&mut T) -> Option<&mut U>,
    {
        let raw = &s.mutex.raw;
        let data = match f(unsafe { &mut *s.mutex.data.get() }) {
            Some(data) => data,
            None => return Err(s),
        };
    }

    /// Temporarily unlocks the mutex to execute given function
    // NOTE: This is safe because `&mut` guarantees that there exists no other referenes to the
    // data protected by the mutex
    #[inline]
    pub fn unlocked<F, U>(s: &mut Self, f: F) -> U
    where
        F: FnOnce() -> U,
    {
        // SAFETY: A MutexGuard always holds the lock
        unsafe {
            s.mutex.raw.unlock();
        }

        defer!(s.mutex.raw.lock());

        f()
    }

    /// Leaks the mutex guard and returns a mutable reference to the data protected by the mutex
    #[inline]
    pub fn leak(s: Self) -> &'a mut T {
        let r = unsafe { &mut *s.mutex.data.get() };

        mem::forget(s);

        r
    }
}
