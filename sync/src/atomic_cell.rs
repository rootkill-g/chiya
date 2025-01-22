use core::{
    cell::UnsafeCell,
    cmp,
    mem::{ManuallyDrop, MaybeUninit},
};
use std::sync::atomic::{self, Ordering};

#[repr(transparent)]
pub struct AtomicCell<T> {
    value: UnsafeCell<MaybeUninit<T>>,
}

unsafe impl<T: Send> Send for AtomicCell<T> {}
unsafe impl<T: Sync> Sync for AtomicCell<T> {}

impl<T> AtomicCell<T> {
    /// Creates a new atomic cell initialized with `value`
    pub fn new(value: T) -> AtomicCell<T> {
        AtomicCell {
            value: UnsafeCell::new(MaybeUninit::new(value)),
        }
    }

    /// Consumes the atomic cell and returns the underlying value
    pub fn into_inner(self) -> T {
        let this = ManuallyDrop::new(self);

        // SAFETY:
        // - Passing `self` by value guarantees that no other threads are concurrently accessing
        // the atomic data
        // - The raw pointer passed in is valid because we got it from an owned value
        // - `ManuallyDrop` prevents double dropping of `T`
        unsafe { this.as_ptr().read() }
    }

    /// Returns `true` if the operations on values of this type are lock-free
    pub const fn is_lock_free() -> bool {
        atomic_is_lock_free::<T>()
    }

    /// Stores `value` into the atomic cell
    pub fn store(&self, value: T) {
        if std::mem::needs_drop::<T>() {
            drop(self.swap(value));
        } else {
            unsafe { atomic_store(self.as_ptr(), value) };
        }
    }

    /// Stores `value` into the atomic cell and returns the previous value
    pub fn swap(&self, value: T) -> T {
        unsafe { atomic_swap(self.as_ptr(), value) }
    }

    /// Returns a raw pointer to the underlying data in this atomic cell
    #[inline]
    pub fn as_ptr(&self) -> *mut T {
        self.value.get().cast::<T>()
    }
}

impl<T: Default> AtomicCell<T> {
    /// Takes the value of the atomic cell, leaving `Default::default()` in its place
    pub fn take(&self) -> T {
        self.swap(Default::default())
    }
}

impl<T: Copy> AtomicCell<T> {
    /// Loads a value from the atomic cell
    pub fn load(&self) -> T {
        unsafe { atomic_load(self.as_ptr()) }
    }
}

impl<T: Copy + Eq> AtomicCell<T> {
    /// If the current value equals `current`, stores `new` into the atomic cell
    pub fn compare_exchange(&self, current: T, new: T) -> Result<T, T> {
        unsafe { atomic_compare_exchange_weak(self.as_ptr(), current, new) }
    }

    /// Fetches the value and applies a function to it that returns an optional new value
    /// Returns a `Result` of `Ok(previous_value)` if the function returned `Some(_)`, else
    /// `Err(previous_value)`
    pub fn fetch_update<F>(&self, mut f: F) -> Result<T, T>
    where
        F: FnMut(T) -> Option<T>,
    {
        let mut previous = self.load();

        while let Some(next) = f(previous) {
            match self.compare_exchange(previous, next) {
                x @ Ok(_) => return x,
                Err(next_previous) => previous = next_previous,
            }
        }

        Err(previous)
    }
}

/// `MaybeUninit` prevents `T` from being dropped, so we need to implement `Drop` for `AtomicCell`
/// to avoid leaks of non-`Copy` types
impl<T> Drop for AtomicCell<T> {
    fn drop(&mut self) {
        if std::mem::needs_drop::<T>() {
            // SAFETY:
            // - The mutable references guarantees that no other threads are concurrently accessing
            // the atomic data
            // - The raw pointer passed in is valid because we got it from a reference
            // - `MaybeUninit` prevents double dropping of `T`
            unsafe { self.as_ptr().drop_in_place() };
        }
    }
}

macro_rules! atomic {
    // If values of type `$t` can be transmuted into values of the primitive atomic type
    // `$atomic`, declares variables `$a` of type `$atomic` and executes `$atomic_op`, breaking
    // out of the loop
    (@check, $t:ty, $atomic:ty, $a:ident, $atomic_op:expr) => {
        if can_transmute::<$t, $atomic>() {
            let $a: &$atomic;

            break $atomic_op;
        }
    };

    // If values of type `$t` can be transmuted into values of a primitive atomic type, declares
    // variable `$a` of that type and executes `$atomic_op`. Otherwise, just executes
    // `$fallback_op`
    ($t:ty, $a:ident, $atomic_op:expr, $fallback_op:expr) => {
        loop {
            atomic!(@check, $t, AtomicUnit, $a, $atomic_op);

            atomic!(@check, $t, atomic::AtomicU8, $a, $atomic_op);
            atomic!(@check, $t, atomic::AtomicU16, $a, $atomic_op);
            atomic!(@check, $t, atomic::AtomicU32, $a, $atomic_op);
            #[cfg(target_has_atomic = "64")]
            atomic!(@check, $t, atomic::AtomicU64, $a, $atomic_op);
            // TODO: AtomicU128 is unstable
            // atomic!(@check, $t, atomic::AtomicU128, $a, $atomic_op);

            break $fallback_op;
        }
    };
}

macro_rules! impl_arithmetic {
    ($t:ty, fallback, $example:tt) => {
        impl AtomicCell<$t> {
            /// Increments the current value by `val` and returns the previous value.
            ///
            /// The addition wraps on overflow.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_add(3), 7);
            /// assert_eq!(a.load(), 10);
            /// ```
            #[inline]
            pub fn fetch_add(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value = value.wrapping_add(val);
                old
            }

            /// Decrements the current value by `val` and returns the previous value.
            ///
            /// The subtraction wraps on overflow.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_sub(3), 7);
            /// assert_eq!(a.load(), 4);
            /// ```
            #[inline]
            pub fn fetch_sub(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value = value.wrapping_sub(val);
                old
            }

            /// Applies bitwise "and" to the current value and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_and(3), 7);
            /// assert_eq!(a.load(), 3);
            /// ```
            #[inline]
            pub fn fetch_and(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value &= val;
                old
            }

            /// Applies bitwise "nand" to the current value and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_nand(3), 7);
            /// assert_eq!(a.load(), !(7 & 3));
            /// ```
            #[inline]
            pub fn fetch_nand(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value = !(old & val);
                old
            }

            /// Applies bitwise "or" to the current value and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_or(16), 7);
            /// assert_eq!(a.load(), 23);
            /// ```
            #[inline]
            pub fn fetch_or(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value |= val;
                old
            }

            /// Applies bitwise "xor" to the current value and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_xor(2), 7);
            /// assert_eq!(a.load(), 5);
            /// ```
            #[inline]
            pub fn fetch_xor(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value ^= val;
                old
            }

            /// Compares and sets the maximum of the current value and `val`,
            /// and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_max(2), 7);
            /// assert_eq!(a.load(), 7);
            /// ```
            #[inline]
            pub fn fetch_max(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value = cmp::max(old, val);
                old
            }

            /// Compares and sets the minimum of the current value and `val`,
            /// and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_min(2), 7);
            /// assert_eq!(a.load(), 2);
            /// ```
            #[inline]
            pub fn fetch_min(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value = cmp::min(old, val);
                old
            }
        }
    };
    ($t:ty, $atomic:ident, $example:tt) => {
        impl AtomicCell<$t> {
            /// Increments the current value by `val` and returns the previous value.
            ///
            /// The addition wraps on overflow.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_add(3), 7);
            /// assert_eq!(a.load(), 10);
            /// ```
            #[inline]
            pub fn fetch_add(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const atomic::$atomic) };
                        a.fetch_add(val, Ordering::AcqRel)
                    },
                    {
                        let _guard = lock(self.as_ptr() as usize).write();
                        let value = unsafe { &mut *(self.as_ptr()) };
                        let old = *value;
                        *value = value.wrapping_add(val);
                        old
                    }
                }
            }

            /// Decrements the current value by `val` and returns the previous value.
            ///
            /// The subtraction wraps on overflow.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_sub(3), 7);
            /// assert_eq!(a.load(), 4);
            /// ```
            #[inline]
            pub fn fetch_sub(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const atomic::$atomic) };
                        a.fetch_sub(val, Ordering::AcqRel)
                    },
                    {
                        let _guard = lock(self.as_ptr() as usize).write();
                        let value = unsafe { &mut *(self.as_ptr()) };
                        let old = *value;
                        *value = value.wrapping_sub(val);
                        old
                    }
                }
            }

            /// Applies bitwise "and" to the current value and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_and(3), 7);
            /// assert_eq!(a.load(), 3);
            /// ```
            #[inline]
            pub fn fetch_and(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const atomic::$atomic) };
                        a.fetch_and(val, Ordering::AcqRel)
                    },
                    {
                        let _guard = lock(self.as_ptr() as usize).write();
                        let value = unsafe { &mut *(self.as_ptr()) };
                        let old = *value;
                        *value &= val;
                        old
                    }
                }
            }

            /// Applies bitwise "nand" to the current value and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_nand(3), 7);
            /// assert_eq!(a.load(), !(7 & 3));
            /// ```
            #[inline]
            pub fn fetch_nand(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const atomic::$atomic) };
                        a.fetch_nand(val, Ordering::AcqRel)
                    },
                    {
                        let _guard = lock(self.as_ptr() as usize).write();
                        let value = unsafe { &mut *(self.as_ptr()) };
                        let old = *value;
                        *value = !(old & val);
                        old
                    }
                }
            }

            /// Applies bitwise "or" to the current value and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_or(16), 7);
            /// assert_eq!(a.load(), 23);
            /// ```
            #[inline]
            pub fn fetch_or(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const atomic::$atomic) };
                        a.fetch_or(val, Ordering::AcqRel)
                    },
                    {
                        let _guard = lock(self.as_ptr() as usize).write();
                        let value = unsafe { &mut *(self.as_ptr()) };
                        let old = *value;
                        *value |= val;
                        old
                    }
                }
            }

            /// Applies bitwise "xor" to the current value and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_xor(2), 7);
            /// assert_eq!(a.load(), 5);
            /// ```
            #[inline]
            pub fn fetch_xor(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const atomic::$atomic) };
                        a.fetch_xor(val, Ordering::AcqRel)
                    },
                    {
                        let _guard = lock(self.as_ptr() as usize).write();
                        let value = unsafe { &mut *(self.as_ptr()) };
                        let old = *value;
                        *value ^= val;
                        old
                    }
                }
            }

            /// Compares and sets the maximum of the current value and `val`,
            /// and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_max(9), 7);
            /// assert_eq!(a.load(), 9);
            /// ```
            #[inline]
            pub fn fetch_max(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const atomic::$atomic) };
                        a.fetch_max(val, Ordering::AcqRel)
                    },
                    {
                        let _guard = lock(self.as_ptr() as usize).write();
                        let value = unsafe { &mut *(self.as_ptr()) };
                        let old = *value;
                        *value = cmp::max(old, val);
                        old
                    }
                }
            }

            /// Compares and sets the minimum of the current value and `val`,
            /// and returns the previous value.
            ///
            /// # Examples
            ///
            /// ```
            /// use crossbeam_utils::atomic::AtomicCell;
            ///
            #[doc = $example]
            ///
            /// assert_eq!(a.fetch_min(2), 7);
            /// assert_eq!(a.load(), 2);
            /// ```
            #[inline]
            pub fn fetch_min(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const atomic::$atomic) };
                        a.fetch_min(val, Ordering::AcqRel)
                    },
                    {
                        let _guard = lock(self.as_ptr() as usize).write();
                        let value = unsafe { &mut *(self.as_ptr()) };
                        let old = *value;
                        *value = cmp::min(old, val);
                        old
                    }
                }
            }
        }
    };
}

impl_arithmetic!(u8, AtomicU8, "let a = AtomicCell::new(7u8);");
impl_arithmetic!(i8, AtomicI8, "let a = AtomicCell::new(7i8);");
impl_arithmetic!(u16, AtomicU16, "let a = AtomicCell::new(7u16);");
impl_arithmetic!(i16, AtomicI16, "let a = AtomicCell::new(7i16);");

impl_arithmetic!(u32, AtomicU32, "let a = AtomicCell::new(7u32);");
impl_arithmetic!(i32, AtomicI32, "let a = AtomicCell::new(7i32);");

#[cfg(target_has_atomic = "64")]
impl_arithmetic!(u64, AtomicU64, "let a = AtomicCell::new(7u64);");
#[cfg(target_has_atomic = "64")]
impl_arithmetic!(i64, AtomicI64, "let a = AtomicCell::new(7i64);");
#[cfg(not(target_has_atomic = "64"))]
impl_arithmetic!(u64, fallback, "let a = AtomicCell::new(7u64);");
#[cfg(not(target_has_atomic = "64"))]
impl_arithmetic!(i64, fallback, "let a = AtomicCell::new(7i64);");

// TODO: AtomicU128 is unstable
// impl_arithmetic!(u128, AtomicU128, "let a = AtomicCell::new(7u128);");
// impl_arithmetic!(i128, AtomicI128, "let a = AtomicCell::new(7i128);");
impl_arithmetic!(u128, fallback, "let a = AtomicCell::new(7u128);");
impl_arithmetic!(i128, fallback, "let a = AtomicCell::new(7i128);");

impl_arithmetic!(usize, AtomicUsize, "let a = AtomicCell::new(7usize);");
impl_arithmetic!(isize, AtomicIsize, "let a = AtomicCell::new(7isize);");

impl AtomicCell<bool> {
    /// Applies logical `and` to the current value and returns the previous value
    #[inline]
    pub fn fetch_and(&self, value: bool) -> bool {
        atomic! {
            bool, _a,
            {
                let a = unsafe { &*self.as_ptr() as *const atomic::AtomicBool };

                a.fetch_and(value, Ordering::AcqRel)
            },
            {
                let _guard = lock(self.as_ptr() as usize).write();
                let val = unsafe { &mut *(self.as_ptr()) };
                let old = *val;

                *val &= value;

                old
            }
        }
    }

    /// Applies logical `nand` to the current value and returns the previous value
    #[inline]
    pub fn fetch_nand(&self, value: bool) -> bool {
        atomic! {
            bool, _a,
            {
                let a = unsafe { &*(self.as_ptr() as *const atomic::AtomicBool) };

                a.fetch_nand(value, Ordering::AcqRel)
            },
            {
                let _guard = lock(self.as_ptr() as usize).write();
                let val = unsafe { &mut *(self.as_ptr()) };
                let old = &val;

                *val = !(old & value);

                old
            }
        }
    }
}
