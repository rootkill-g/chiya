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

unsafe impl<R: RawMutex + Send, T: ?Sized> Send for Mutex<R, T> {}
unsafe impl<R: RawMutex + Sync, T: ?Sized> Sync for Mutex<R, T> {}

}
