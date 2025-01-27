use crate::raw_mutex::RawMutex;

/// Additional methods for mutexes which support fair unlocking
///
/// Fair unlocking means that a lock is handed directly over to the next waiting thread if there is
/// one, without giving other threads the opportunity to "steal" the lock in the meantime. This is
/// typically slower than unfair unlocking, but maybe necessary in certain circumstances.
pub unsafe trait RawMutexFair: RawMutex {
    /// Unlock this mutex using a fair lock protocol
    unsafe fn unlock_fair(&self);

    /// Temporarily yields the mutex to a waiting thread if there is one.
    unsafe fn bump(&self) {
        unsafe {
            self.unlock_fair();

            self.lock();
        }
    }
}
