use super::{limit, Buf, Chain, Limit, UninitSlice};
#[cfg(feature = "std")]
use crate::buf::{writer, Writer};
use crate::{panic_advance, panic_does_not_fit};
use alloc::{boxed::Box, vec::Vec};
use core::{
    mem::{self, MaybeUninit},
    ptr, usize,
};

/// A trait for values that provide sequential write access to bytes
pub unsafe trait BufMut {
    /// Returns the number of bytes that can be written from the current position until the end of
    /// the buffer is reached
    fn remaining_mut(&self) -> usize;

    /// Advance the internal cursor of the BufMut
    unsafe fn advance_mut(&mut self, cnt: usize);

    /// Returns true if there is space in `self` for more bytes
    #[inline]
    fn has_remaining_mut(&self) -> bool {
        self.remaining_mut() > 0
    }

    /// Returns a mutable slice starting at the current BufMut position and of length between 0 and
    /// `BufMut::remaining_mut()`. Note that this **can** be shorter than the whole remainder of
    /// the buffer (this allows non-continuous implementation)
    #[cfg_attr(docrs, doc(alias = "bytes_mut"))]
    fn chunk_mut(&mut self) -> &mut UninitSlice;

    /// Transfer bytes into `self` from `src` and advance the cursor by the number of bytes written
    #[inline]
    fn put<T>(&mut self, mut src: T)
    where
        T: super::Buf,
        Self: Sized,
    {
        if self.remaining_mut() < src.remaining() {
            panic_advance(src.remaining(), self.remaining_mut());
        }

        while src.has_remaining() {
            let s = src.chunk();
            let d = self.chunk_mut();
            let cnt = usize::min(s.len(), d.len());

            d[..cnt].copy_from_slice(&s[..cnt]);

            // SAFETY: `cnt` was just initialized in `self`
            unsafe { self.advance_mut(cnt) };

            src.advance(cnt);
        }
    }

    /// Transfer bytes into `self` from `src` and advance the cursor by the number of bytes written
    #[inline]
    fn put_slice(&mut self, mut src: &[u8]) {
        if self.remaining_mut() < src.len() {
            panic_advance(src.len(), self.remaining_mut());
        }

        while !src.is_empty() {
            let dst = self.chunk_mut();
            let cnt = usize::min(src.len(), dst.len());

            dst[..cnt].copy_from_slice(&src[..cnt]);
            src = &src[cnt..];

            // SAFETY: We just initialized `cnt` bytes in `self`
            unsafe { self.advance_mut(cnt) };
        }
    }

    /// Puts `cnt` bytes `val` into `self`
    #[inline]
    fn put_bytes(&mut self, val: u8, mut cnt: usize) {
        if self.remaining_mut() < cnt {
            panic_advance(cnt, self.remaining_mut());
        }

        while cnt > 0 {
            let dst = self.chunk_mut();
            let dst_len = usize::min(dst.len(), cnt);

            // SAFETY: The pointer is valid for `dst_len <= dst.len()` bytes.
            unsafe { core::ptr::write_bytes(dst.as_mut_ptr(), val, dst_len) };

            // SAFETY: We just initialized `dst_len` bytes in `dst`
            unsafe { self.advance_mut(dst_len) };

            cnt -= dst_len
        }
    }

    /// Writes an unsigned 8-bit integer into `self`
    #[inline]
    fn put_u8(&mut self, n: u8) {
        let src = [n];
        self.put_slice(&src);
    }

    /// Writes an signed 8-bit integer into `self`
    #[inline]
    fn put_i8(&mut self, n: i8) {
        let src = [n as u8];
        self.put_slice(&src)
    }

    /// Write unsigned 16 bit integer to `self` in big-endian byte order
    #[inline]
    fn put_u16(&mut self, n: u16) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Write unsigned 16 bit integer to `self` in little-endian byte order
    #[inline]
    fn put_u16_le(&mut self, n: u16) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Write unsigned 16 bit integer to `self` in native-endian byte order
    #[inline]
    fn put_u16_ne(&mut self, n: u16) {
        self.put_slice(&n.to_ne_bytes())
    }

    /// Write signed 16 bit integer to `self` in big-endian byte order
    #[inline]
    fn put_i16(&mut self, n: i16) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Write signed 16 bit integer to `self` in little-endian byte order
    #[inline]
    fn put_i16_le(&mut self, n: i16) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Write signed 16 bit integer to `self` in native-endian byte order
    #[inline]
    fn put_i16_ne(&mut self, n: i16) {
        self.put_slice(&n.to_ne_bytes())
    }

    /// Write unsigned 32 bit integer to `self` in big-endian byte order
    #[inline]
    fn put_u32(&mut self, n: u32) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Write unsigned 32 bit integer to `self` in little-endian byte order
    #[inline]
    fn put_u32_le(&mut self, n: u32) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Write unsigned 32 bit integer to `self` in native-endian byte order
    #[inline]
    fn put_u32_ne(&mut self, n: u32) {
        self.put_slice(&n.to_ne_bytes())
    }

    /// Write signed 32 bit integer to `self` in big-endian byte order
    #[inline]
    fn put_i32(&mut self, n: i32) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Write signed 32 bit integer to `self` in little-endian byte order
    #[inline]
    fn put_i32_le(&mut self, n: i32) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Write signed 32 bit integer to `self` in native-endian byte order
    #[inline]
    fn put_i32_ne(&mut self, n: i32) {
        self.put_slice(&n.to_ne_bytes())
    }

    /// Write unsigned 64 bit integer to `self` in big-endian byte order
    #[inline]
    fn put_u64(&mut self, n: u64) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Write unsigned 64 bit integer to `self` in little-endian byte order
    #[inline]
    fn put_u64_le(&mut self, n: u64) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Write unsigned 64 bit integer to `self` in native-endian byte order
    #[inline]
    fn put_u64_ne(&mut self, n: u64) {
        self.put_slice(&n.to_ne_bytes())
    }

    /// Write signed 64 bit integer to `self` in big-endian byte order
    #[inline]
    fn put_i64(&mut self, n: i64) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Write signed 64 bit integer to `self` in little-endian byte order
    #[inline]
    fn put_i64_le(&mut self, n: i64) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Write signed 64 bit integer to `self` in native-endian byte order
    #[inline]
    fn put_i64_ne(&mut self, n: i64) {
        self.put_slice(&n.to_ne_bytes())
    }

    /// Write unsigned 128 bit integer to `self` in big-endian byte order
    #[inline]
    fn put_u128(&mut self, n: u128) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Write unsigned 128 bit integer to `self` in little-endian byte order
    #[inline]
    fn put_u128_le(&mut self, n: u128) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Write unsigned 128 bit integer to `self` in native-endian byte order
    #[inline]
    fn put_u128_ne(&mut self, n: u128) {
        self.put_slice(&n.to_ne_bytes())
    }

    /// Write signed 128 bit integer to `self` in big-endian byte order
    #[inline]
    fn put_i128(&mut self, n: i128) {
        self.put_slice(&n.to_be_bytes())
    }

    /// Write signed 128 bit integer to `self` in little-endian byte order
    #[inline]
    fn put_i128_le(&mut self, n: i128) {
        self.put_slice(&n.to_le_bytes())
    }

    /// Write signed 128 bit integer to `self` in native-endian byte order
    #[inline]
    fn put_i128_ne(&mut self, n: i128) {
        self.put_slice(&n.to_ne_bytes())
    }

    /// Writes an unsigned n-byte integer to `self` in big-endian byte order.
    #[inline]
    fn put_uint(&mut self, n: u64, nbytes: usize) {
        let start = match mem::size_of_val(&n).checked_sub(nbytes) {
            Some(start) => start,
            None => panic_does_not_fit(nbytes, mem::size_of_val(&n)),
        };

        self.put_slice(&n.to_be_bytes()[start..]);
    }

    /// Writes an unsigned n-byte integer to `self` in little-endian byte order.
    #[inline]
    fn put_uint_le(&mut self, n: u64, nbytes: usize) {
        let slice = n.to_le_bytes();
        let slice = match slice.get(..nbytes) {
            Some(slice) => slice,
            None => panic_does_not_fit(nbytes, slice.len()),
        };

        self.put_slice(slice);
    }

    /// Writes an unsigned n-byte integer to `self` in native-endian byte order.
    #[inline]
    fn put_uint_ne(&mut self, n: u64, nbytes: usize) {
        if cfg!(target_endian = "big") {
            self.put_uint(n, nbytes)
        } else {
            self.put_uint_le(n, nbytes)
        }
    }

    /// Writes low `nbytes` of a signed integer to `self` in big-endian byte order.
    #[inline]
    fn put_int(&mut self, n: i64, nbytes: usize) {
        let start = match mem::size_of_val(&n).checked_sub(nbytes) {
            Some(start) => start,
            None => panic_does_not_fit(nbytes, mem::size_of_val(&n)),
        };

        self.put_slice(&n.to_be_bytes()[start..]);
    }

    /// Writes low `nbytes` of a signed integer to `self` in little-endian byte order.
    #[inline]
    fn put_int_le(&mut self, n: i64, nbytes: usize) {
        let slice = n.to_le_bytes();
        let slice = match slice.get(..nbytes) {
            Some(slice) => slice,
            None => panic_does_not_fit(nbytes, slice.len()),
        };

        self.put_slice(slice);
    }

    /// Writes low `nbytes` of a signed integer to `self` in native-endian byte order.
    #[inline]
    fn put_int_ne(&mut self, n: i64, nbytes: usize) {
        if cfg!(target_endian = "big") {
            self.put_int(n, nbytes)
        } else {
            self.put_int_le(n, nbytes)
        }
    }

    /// Writes an IEEE754 single-precision (4 bytes) floating point number
    /// to `self` in big-endian byte order.
    #[inline]
    fn put_f32(&mut self, n: f32) {
        self.put_u32(n.to_bits());
    }

    /// Writes an IEEE754 single-precision (4 bytes) floating point number
    /// to `self` in little-endian byte order.
    #[inline]
    fn put_f32_le(&mut self, n: f32) {
        self.put_u32_le(n.to_bits());
    }

    /// Writes an IEEE754 single-precision (4 bytes) floating point number
    /// to `self` in native-endian byte order.
    #[inline]
    fn put_f32_ne(&mut self, n: f32) {
        self.put_u32_ne(n.to_bits());
    }

    /// Writes an IEEE754 double-precision (8 bytes) floating point number
    /// to `self` in big-endian byte order.
    #[inline]
    fn put_f64(&mut self, n: f64) {
        self.put_u64(n.to_bits());
    }

    /// Writes an IEEE754 double-precision (8 bytes) floating point number
    /// to `self` in little-endian byte order.
    #[inline]
    fn put_f64_le(&mut self, n: f64) {
        self.put_u64_le(n.to_bits());
    }

    /// Writes an IEEE754 double-precision (8 bytes) floating point number
    /// to `self` in native-endian byte order.
    #[inline]
    fn put_f64_ne(&mut self, n: f64) {
        self.put_u64_ne(n.to_bits());
    }

    /// Creates an adapter which can write at most `limit` bytes to `self`
    #[inline]
    fn limit(self, limit: usize) -> Limit<Self>
    where
        Self: Sized,
    {
        limit::new(self, limit)
    }

    /// Creates an adapter which implements `Write` trait for `self`
    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    #[inline]
    fn writer(self) -> Writer<Self>
    where
        Self: Sized,
    {
        writer::new(self)
    }

    /// Creates an adapter which will chain this buffer with another
    #[inline]
    fn chain_mut<U>(self, next: U) -> Chain<Self, U>
    where
        U: BufMut,
        Self: Sized,
    {
        Chain::new(self, next)
    }
}

macro_rules! deref_forward_bufmut {
    () => {
        #[inline]
        fn remaining_mut(&self) -> usize {
            (**self).remaining_mut()
        }

        #[inline]
        fn chunk_mut(&mut self) -> &mut UninitSlice {
            (**self).chunk_mut()
        }

        #[inline]
        unsafe fn advance_mut(&mut self, cnt: usize) {
            unsafe { (**self).advance_mut(cnt) }
        }

        #[inline]
        fn put_slice(&mut self, src: &[u8]) {
            (**self).put_slice(src)
        }

        #[inline]
        fn put_u8(&mut self, n: u8) {
            (**self).put_u8(n)
        }

        #[inline]
        fn put_i8(&mut self, n: i8) {
            (**self).put_i8(n)
        }

        #[inline]
        fn put_u16(&mut self, n: u16) {
            (**self).put_u16(n)
        }

        #[inline]
        fn put_u16_le(&mut self, n: u16) {
            (**self).put_u16_le(n)
        }

        #[inline]
        fn put_u16_ne(&mut self, n: u16) {
            (**self).put_u16_ne(n)
        }

        #[inline]
        fn put_i16(&mut self, n: i16) {
            (**self).put_i16(n)
        }

        #[inline]
        fn put_i16_le(&mut self, n: i16) {
            (**self).put_i16_le(n)
        }

        #[inline]
        fn put_i16_ne(&mut self, n: i16) {
            (**self).put_i16_ne(n)
        }

        #[inline]
        fn put_u32(&mut self, n: u32) {
            (**self).put_u32(n)
        }

        #[inline]
        fn put_u32_le(&mut self, n: u32) {
            (**self).put_u32_le(n)
        }

        #[inline]
        fn put_u32_ne(&mut self, n: u32) {
            (**self).put_u32_ne(n)
        }

        #[inline]
        fn put_i32(&mut self, n: i32) {
            (**self).put_i32(n)
        }

        #[inline]
        fn put_i32_le(&mut self, n: i32) {
            (**self).put_i32_le(n)
        }

        #[inline]
        fn put_i32_ne(&mut self, n: i32) {
            (**self).put_i32_ne(n)
        }

        #[inline]
        fn put_u64(&mut self, n: u64) {
            (**self).put_u64(n)
        }

        #[inline]
        fn put_u64_le(&mut self, n: u64) {
            (**self).put_u64_le(n)
        }

        #[inline]
        fn put_u64_ne(&mut self, n: u64) {
            (**self).put_u64_ne(n)
        }

        #[inline]
        fn put_i64(&mut self, n: i64) {
            (**self).put_i64(n)
        }

        #[inline]
        fn put_i64_le(&mut self, n: i64) {
            (**self).put_i64_le(n)
        }

        #[inline]
        fn put_i64_ne(&mut self, n: i64) {
            (**self).put_i64_ne(n)
        }
    };
}

unsafe impl<T> BufMut for &mut T
where
    T: BufMut + ?Sized,
{
    deref_forward_bufmut!();
}

unsafe impl<T> BufMut for Box<T>
where
    T: BufMut + ?Sized,
{
    deref_forward_bufmut!();
}

unsafe impl BufMut for &mut [u8] {
    #[inline]
    fn remaining_mut(&self) -> usize {
        self.len()
    }

    #[inline]
    fn chunk_mut(&mut self) -> &mut UninitSlice {
        UninitSlice::new(self)
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        if self.len() < cnt {
            panic_advance(cnt, self.len())
        }

        // Lifetime dance taken from `impl Write for &mut [u8]`
        let (_, b) = core::mem::replace(self, &mut []).split_at_mut(cnt);

        *self = b;
    }

    #[inline]
    fn put_slice(&mut self, src: &[u8]) {
        if self.len() < src.len() {
            panic_advance(src.len(), self.len());
        }

        self[..src.len()].copy_from_slice(src);

        // SAFETY: we just initialized `src.len()` bytes
        unsafe { self.advance_mut(src.len()) };
    }

    #[inline]
    fn put_bytes(&mut self, val: u8, cnt: usize) {
        if self.len() < cnt {
            panic_advance(cnt, self.len());
        }

        // SAFETY: We just checked that the pointer is valid for `cnt` bytes
        unsafe {
            ptr::write_bytes(self.as_mut_ptr(), val, cnt);

            self.advance_mut(cnt);
        }
    }
}

unsafe impl BufMut for &mut [MaybeUninit<u8>] {
    #[inline]
    fn remaining_mut(&self) -> usize {
        self.len()
    }

    #[inline]
    fn chunk_mut(&mut self) -> &mut UninitSlice {
        UninitSlice::uninit(self)
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        if self.len() < cnt {
            panic_advance(cnt, self.len());
        }

        // Lifetime dance taken from `impl Write for &mut [u8]`
        let (_, b) = core::mem::replace(self, &mut []).split_at_mut(cnt);

        *self = b;
    }

    #[inline]
    fn put_slice(&mut self, src: &[u8]) {
        if self.len() < src.len() {
            panic_advance(src.len(), self.len());
        }

        // SAFETY: We just checked that the pointer is valid for `src.len()` bytes
        unsafe {
            ptr::copy_nonoverlapping(src.as_ptr(), self.as_mut_ptr().cast(), src.len());

            self.advance_mut(src.len());
        }
    }

    #[inline]
    fn put_bytes(&mut self, val: u8, cnt: usize) {
        if self.len() < cnt {
            panic_advance(cnt, self.len());
        }

        // SAFETY: We just checked that the pointer is valid for `cnt` bytes
        unsafe {
            ptr::write_bytes(self.as_mut_ptr() as *mut u8, val, cnt);

            self.advance_mut(cnt);
        }
    }
}

unsafe impl BufMut for Vec<u8> {
    #[inline]
    fn remaining_mut(&self) -> usize {
        // A vector can never have more that isize::MAX bytes
        core::isize::MAX as usize - self.len()
    }

    #[inline]
    fn chunk_mut(&mut self) -> &mut UninitSlice {
        if self.capacity() == self.len() {
            // Grow the Vec
            self.reserve(64);
        }

        let cap = self.capacity();
        let len = self.len();
        let ptr = self.as_mut_ptr();

        // SAFETY: Since `ptr` is valid for `cap` bytes, `ptr.add(len)` must be valid for
        // `cap - len` bytes. The subtraction won't underflow since `len <= cap`
        unsafe { UninitSlice::from_raw_parts_mut(ptr.add(len), cap - len) }
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        unsafe {
            let len = self.len();
            let remaining = self.capacity() - len;

            if remaining < cnt {
                panic_advance(cnt, remaining);
            }

            // Addition will not overflow since the sum is at most the capacity
            self.set_len(len + cnt);
        }
    }

    #[inline]
    fn put<T>(&mut self, mut src: T)
    where
        T: Buf,
        Self: Sized,
    {
        // In case src isn't contiguous, reserve upfront
        self.reserve(src.remaining());

        while src.has_remaining() {
            let s = src.chunk();
            let l = s.len();

            self.extend_from_slice(s);

            src.advance(l);
        }
    }

    #[inline]
    fn put_slice(&mut self, src: &[u8]) {
        self.extend_from_slice(src);
    }

    #[inline]
    fn put_bytes(&mut self, val: u8, cnt: usize) {
        // If the addition overflows, then the resize will fail
        let new_len = self.len().saturating_add(cnt);

        self.resize(new_len, val);
    }
}

// The existence of this function makes the compiler catch if the BufMut trait is "object-safe" or not
fn _assert_trait_object(_b: &dyn BufMut) {}
