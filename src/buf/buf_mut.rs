use super::{limit, Chain, Limit, UninitSlice};
#[cfg(feature = "std")]
use crate::buf::{writer, Writer};
use crate::{panic_advance, panic_does_not_fit};
use alloc::{boxed::Box, vec::Vec};
use core::{mem, ptr, usize};

pub unsafe trait BufMut {
    fn remaining_mut(&self) -> usize;

    unsafe fn advance_mut(&mut self, cnt: usize);

    #[inline]
    fn has_remaining_mut(&self) -> bool {
        self.remaining_mut() > 0
    }

    #[cfg_attr(docrs, doc(alias = "bytes_mut"))]
    fn chunk_mut(&mut self) -> &mut UninitSlice;

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

    #[inline]
    fn put_u8(&mut self, n: u8) {
        let src = [n];
        self.put_slice(&src);
    }

    #[inline]
    fn put_i8(&mut self, n: i8) {
        let src = [n as u8];
        self.put_slice(&src)
    }

    // Write unsigned 16 bit integer to `self` in big-endian byte order
    #[inline]
    fn put_u16(&mut self, n: u16) {
        self.put_slice(&n.to_be_bytes())
    }

    // Write unsigned 16 bit integer to `self` in little-endian byte order
    #[inline]
    fn put_u16_le(&mut self, n: u16) {
        self.put_slice(&n.to_le_bytes())
    }

    // Write unsigned 16 bit integer to `self` in native-endian byte order
    #[inline]
    fn put_u16_ne(&mut self, n: u16) {
        self.put_slice(&n.to_ne_bytes())
    }

    // Write signed 16 bit integer to `self` in big-endian byte order
    #[inline]
    fn put_i16(&mut self, n: i16) {
        self.put_slice(&n.to_be_bytes())
    }

    // Write signed 16 bit integer to `self` in little-endian byte order
    #[inline]
    fn put_i16_le(&mut self, n: i16) {
        self.put_slice(&n.to_le_bytes())
    }

    // Write signed 16 bit integer to `self` in native-endian byte order
    #[inline]
    fn put_i16_ne(&mut self, n: i16) {
        self.put_slice(&n.to_ne_bytes())
    }

    // Write unsigned 32 bit integer to `self` in big-endian byte order
    #[inline]
    fn put_u32(&mut self, n: u32) {
        self.put_slice(&n.to_be_bytes())
    }

    // Write unsigned 32 bit integer to `self` in little-endian byte order
    #[inline]
    fn put_u32_le(&mut self, n: u32) {
        self.put_slice(&n.to_le_bytes())
    }

    // Write unsigned 32 bit integer to `self` in native-endian byte order
    #[inline]
    fn put_u32_ne(&mut self, n: u32) {
        self.put_slice(&n.to_ne_bytes())
    }

    // Write signed 32 bit integer to `self` in big-endian byte order
    #[inline]
    fn put_i32(&mut self, n: i32) {
        self.put_slice(&n.to_be_bytes())
    }

    // Write signed 32 bit integer to `self` in little-endian byte order
    #[inline]
    fn put_i32_le(&mut self, n: i32) {
        self.put_slice(&n.to_le_bytes())
    }

    // Write signed 32 bit integer to `self` in native-endian byte order
    #[inline]
    fn put_i32_ne(&mut self, n: i32) {
        self.put_slice(&n.to_ne_bytes())
    }

    // Write unsigned 64 bit integer to `self` in big-endian byte order
    #[inline]
    fn put_u64(&mut self, n: u64) {
        self.put_slice(&n.to_be_bytes())
    }

    // Write unsigned 64 bit integer to `self` in little-endian byte order
    #[inline]
    fn put_u64_le(&mut self, n: u64) {
        self.put_slice(&n.to_le_bytes())
    }

    // Write unsigned 64 bit integer to `self` in native-endian byte order
    #[inline]
    fn put_u64_ne(&mut self, n: u64) {
        self.put_slice(&n.to_ne_bytes())
    }

    // Write signed 64 bit integer to `self` in big-endian byte order
    #[inline]
    fn put_i64(&mut self, n: i64) {
        self.put_slice(&n.to_be_bytes())
    }

    // Write signed 64 bit integer to `self` in little-endian byte order
    #[inline]
    fn put_i64_le(&mut self, n: i64) {
        self.put_slice(&n.to_le_bytes())
    }

    // Write signed 64 bit integer to `self` in native-endian byte order
    #[inline]
    fn put_i64_ne(&mut self, n: i64) {
        self.put_slice(&n.to_ne_bytes())
    }

    // Write unsigned 128 bit integer to `self` in big-endian byte order
    #[inline]
    fn put_u128(&mut self, n: u128) {
        self.put_slice(&n.to_be_bytes())
    }

    // Write unsigned 128 bit integer to `self` in little-endian byte order
    #[inline]
    fn put_u128_le(&mut self, n: u128) {
        self.put_slice(&n.to_le_bytes())
    }

    // Write unsigned 128 bit integer to `self` in native-endian byte order
    #[inline]
    fn put_u128_ne(&mut self, n: u128) {
        self.put_slice(&n.to_ne_bytes())
    }

    // Write signed 128 bit integer to `self` in big-endian byte order
    #[inline]
    fn put_i128(&mut self, n: i128) {
        self.put_slice(&n.to_be_bytes())
    }

    // Write signed 128 bit integer to `self` in little-endian byte order
    #[inline]
    fn put_i128_le(&mut self, n: i128) {
        self.put_slice(&n.to_le_bytes())
    }

    // Write signed 128 bit integer to `self` in native-endian byte order
    #[inline]
    fn put_i128_ne(&mut self, n: i128) {
        self.put_slice(&n.to_ne_bytes())
    }

    // Writes an unsigned n-byte integer to `self` in big-endian byte order.
    #[inline]
    fn put_uint(&mut self, n: u64, nbytes: usize) {
        let start = match mem::size_of_val(&n).checked_sub(nbytes) {
            Some(start) => start,
            None => panic_does_not_fit(nbytes, mem::size_of_val(&n)),
        };

        self.put_slice(&n.to_be_bytes()[start..]);
    }

    // Writes an unsigned n-byte integer to `self` in little-endian byte order.
    #[inline]
    fn put_uint_le(&mut self, n: u64, nbytes: usize) {
        let slice = n.to_le_bytes();
        let slice = match slice.get(..nbytes) {
            Some(slice) => slice,
            None => panic_does_not_fit(nbytes, slice.len()),
        };

        self.put_slice(slice);
    }

    // Writes an unsigned n-byte integer to `self` in native-endian byte order.
    #[inline]
    fn put_uint_ne(&mut self, n: u64, nbytes: usize) {
        if cfg!(target_endian = "big") {
            self.put_uint(n, nbytes)
        } else {
            self.put_uint_le(n, nbytes)
        }
    }

    // Writes low `nbytes` of a signed integer to `self` in big-endian byte order.
    #[inline]
    fn put_int(&mut self, n: i64, nbytes: usize) {
        let start = match mem::size_of_val(&n).checked_sub(nbytes) {
            Some(start) => start,
            None => panic_does_not_fit(nbytes, mem::size_of_val(&n)),
        };

        self.put_slice(&n.to_be_bytes()[start..]);
    }

    // Writes low `nbytes` of a signed integer to `self` in little-endian byte order.
    #[inline]
    fn put_int_le(&mut self, n: i64, nbytes: usize) {
        let slice = n.to_le_bytes();
        let slice = match slice.get(..nbytes) {
            Some(slice) => slice,
            None => panic_does_not_fit(nbytes, slice.len()),
        };

        self.put_slice(slice);
    }

    // Writes low `nbytes` of a signed integer to `self` in native-endian byte order.
    #[inline]
    fn put_int_ne(&mut self, n: i64, nbytes: usize) {
        if cfg!(target_endian = "big") {
            self.put_int(n, nbytes)
        } else {
            self.put_int_le(n, nbytes)
        }
    }

    // Writes an IEEE754 single-precision (4 bytes) floating point number
    // to `self` in big-endian byte order.
    #[inline]
    fn put_f32(&mut self, n: f32) {
        self.put_u32(n.to_bits());
    }

    // Writes an IEEE754 single-precision (4 bytes) floating point number
    // to `self` in little-endian byte order.
    #[inline]
    fn put_f32_le(&mut self, n: f32) {
        self.put_u32_le(n.to_bits());
    }

    // Writes an IEEE754 single-precision (4 bytes) floating point number
    // to `self` in native-endian byte order.
    #[inline]
    fn put_f32_ne(&mut self, n: f32) {
        self.put_u32_ne(n.to_bits());
    }

    // Writes an IEEE754 double-precision (8 bytes) floating point number
    // to `self` in big-endian byte order.
    #[inline]
    fn put_f64(&mut self, n: f64) {
        self.put_u64(n.to_bits());
    }

    // Writes an IEEE754 double-precision (8 bytes) floating point number
    // to `self` in little-endian byte order.
    #[inline]
    fn put_f64_le(&mut self, n: f64) {
        self.put_u64_le(n.to_bits());
    }

    // Writes an IEEE754 double-precision (8 bytes) floating point number
    // to `self` in native-endian byte order.
    #[inline]
    fn put_f64_ne(&mut self, n: f64) {
        self.put_u64_ne(n.to_bits());
    }

    #[inline]
    fn limit(self, limit: usize) -> Limit<Self>
    where
        Self: Sized,
    {
        limit::new(self, limit)
    }

    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    #[inline]
    fn writer(self) -> Writer<Self>
    where
        Self: Sized,
    {
        writer::new(self)
    }
}
