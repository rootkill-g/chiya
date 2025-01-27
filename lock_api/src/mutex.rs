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
