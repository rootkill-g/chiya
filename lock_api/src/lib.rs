//! This library provides type-safe [`Mutex`] and [`RwLock`]

#![no_std]
#![warn(missing_docs)]

mod mutex;
mod raw_mutex;
mod raw_mutex_fair;
mod raw_mutex_timed;
mod rwlock;
