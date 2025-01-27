/// Basic operations for a Mutex
/// Types implementing this trait can be used by `Mutex` to form a safe mutex type
pub unsafe trait RawMutex {
    /// Initial value for an unlocked mutex
    #[allow(clippy::declare_interior_mutable_const)]
    const INIT: Self;

    /// Marker type which determines whether a lock guard should be `Send`
    type GuardMarker;

    /// Acquires this mutex, blocking the current thread until it is able to achieve the lock
    fn lock(&self);

    /// Attempts to acquires this mutex, without blocking.
    /// Returns `true` if the lock was successfully acquired and `false` otherwise
    fn try_lock(&self) -> bool;

    /// Unlocks this mutex
    unsafe fn unlock(&self);

    /// Checks whether the mutex is currently locked
    #[inline]
    fn is_locked(&self) -> bool {
        let acquired_lock = self.try_lock();

        if acquired_lock {
            // SAFETY: The lock has been successfully acquired above
            unsafe {
                self.unlock();
            }
        }

        !acquired_lock
    }
}
