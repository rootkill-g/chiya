use std::{
    sync::{Condvar, Mutex},
    time::Duration,
};

use crate::park::ParkError;

#[derive(Debug)]
pub struct ThreadPark {
    lock: Mutex<usize>,
    cvar: Condvar,
}

impl ThreadPark {
    pub fn new() -> ThreadPark {
        ThreadPark {
            lock: Mutex::new(0),
            cvar: Condvar::new(),
        }
    }

    pub fn park_timeout(&self, dur: Option<Duration>) -> Result<(), ParkError> {
        let mut result = Ok(());
        let mut guard = self.lock.lock().unwrap();

        while *guard == 0 && result.is_ok() {
            match dur {
                None => {
                    self.cvar.wait(guard);
                }
                Some(d) => {
                    let t = self.cvar.wait_timeout(guard, d).unwrap().1;

                    if t.timed_out() {
                        result = Err(ParkError::Timeout);
                    }
                }
            }
        }

        // Must clear the status
        *guard = 0;

        result
    }

    pub fn unpark(&self) {
        let mut guard = self.lock.lock().unwrap();

        if *guard == 0 {
            *guard = 1;

            self.cvar.notify_one();
        }
    }
}
