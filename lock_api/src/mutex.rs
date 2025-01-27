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

use crate::raw_mutex::RawMutex;

pub struct Mutex<R, T: ?Sized> {
    raw: R,
    data: UnsafeCell<T>,
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
