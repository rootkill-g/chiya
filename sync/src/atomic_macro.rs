use core::cmp;
use std::sync::atomic::Ordering;

use crate::{AtomicCell, AtomicUnit, can_transmute, lock};

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

            atomic!(@check, $t, crate::primitive::sync::atomic::AtomicU8, $a, $atomic_op);
            atomic!(@check, $t, crate::primitive::sync::atomic::AtomicU16, $a, $atomic_op);
            atomic!(@check, $t, crate::primitive::sync::atomic::AtomicU32, $a, $atomic_op);
            #[cfg(target_has_atomic = "64")]
            atomic!(@check, $t, crate::primitive::sync::atomic::AtomicU64, $a, $atomic_op);
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
            #[inline]
            pub fn fetch_or(&self, val: $t) -> $t {
                let _guard = lock(self.as_ptr() as usize).write();
                let value = unsafe { &mut *(self.as_ptr()) };
                let old = *value;
                *value |= val;
                old
            }

            /// Applies bitwise "xor" to the current value and returns the previous value.
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
            /// The addition wraps on overflow.
            #[inline]
            pub fn fetch_add(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const crate::primitive::sync::atomic::$atomic) };
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
            /// The subtraction wraps on overflow.
            #[inline]
            pub fn fetch_sub(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const crate::primitive::sync::atomic::$atomic) };
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
            #[inline]
            pub fn fetch_and(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const crate::primitive::sync::atomic::$atomic) };
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
            #[inline]
            pub fn fetch_nand(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const crate::primitive::sync::atomic::$atomic) };
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
            #[inline]
            pub fn fetch_or(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const crate::primitive::sync::atomic::$atomic) };
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
            #[inline]
            pub fn fetch_xor(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const crate::primitive::sync::atomic::$atomic) };
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
            #[inline]
            pub fn fetch_max(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const crate::primitive::sync::atomic::$atomic) };
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
            #[inline]
            pub fn fetch_min(&self, val: $t) -> $t {
                atomic! {
                    $t, _a,
                    {
                        let a = unsafe { &*(self.as_ptr() as *const crate::primitive::sync::atomic::$atomic) };
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

// TODO: AtomicU128 is unstable
// impl_arithmetic!(u128, AtomicU128, "let a = AtomicCell::new(7u128);");
// impl_arithmetic!(i128, AtomicI128, "let a = AtomicCell::new(7i128);");
impl_arithmetic!(u128, fallback, "let a = AtomicCell::new(7u128);");
impl_arithmetic!(i128, fallback, "let a = AtomicCell::new(7i128);");

impl_arithmetic!(usize, AtomicUsize, "let a = AtomicCell::new(7usize);");
impl_arithmetic!(isize, AtomicIsize, "let a = AtomicCell::new(7isize);");

pub(crate) use atomic;
pub(crate) use impl_arithmetic;
