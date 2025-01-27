use crate::raw_mutex::RawMutex;

/// Additional methods for mutexes which support locking with timeouts
///
/// The `Duration` and `Instant` types are specified as associated types so that this trait is
/// usable even in `no_std` environments.
pub unsafe trait RawMutexTimed: RawMutex {
    /// Duration type used for `try_lock_for`
    type Duration;

    /// Instant type used for `try_lock_until`
    type Instant;

    /// Attempts to acquire this lock until a timeout is reached.
    fn try_lock_for(&self, timeout: Self::Duration) -> bool;

    /// Attempts to acquire this lock until a timeout is reached.
    fn try_lock_until(&self, timeout: Self::Instant) -> bool;
}
