//! This library provides type-safe [`Mutex`] and [`RwLock`]

#![no_std]
#![warn(missing_docs)]

mod arc_mutex_guard;
mod mutex;
mod mutex_guard;
mod raw_mutex;
mod raw_mutex_fair;
mod raw_mutex_timed;
mod rwlock;

#[cfg(feature = "arc_lock")]
extern crate alloc;
