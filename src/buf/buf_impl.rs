use super::{take, Chain, Take};
#[cfg(feature = "std")]
use crate::saturating_sub_usize_u64;
use crate::{panic_advance, panic_does_not_fit};
use alloc::boxed::Box;
#[cfg(feature = "std")]
use std::io::IoSlice;

macro_rules! buf_get_impl {
    ($this:ident, $typ:tt::$conv:tt) => {{
        const SIZE: usize = core::mem::size_of::<$typ>();

        if $this.remaining() < SIZE {
            panic_advance(SIZE, $this.remaining());
        }

        // try to convert directly from the bytes
        // this Option<ret> trick is to avoid keeping a borrow on self
        // when advance() is called (mut borrow) and to call bytes() only once
        let ret = $this
            .chunk()
            .get(..SIZE)
            .map(|src| unsafe { $typ::$conv(*(src as *const _ as *const [_; SIZE])) });

        if let Some(ret) = ret {
            // if the direct conversion was possible, advance and return
            $this.advance(SIZE);
            return ret;
        } else {
            // if not we copy the bytes in a temp buffer then convert
            let mut buf = [0; SIZE];
            $this.copy_to_slice(&mut buf); // (do the advance)
            return $typ::$conv(buf);
        }
    }};
    (le => $this:ident, $typ:tt, $len_to_read:expr) => {{
        const SIZE: usize = core::mem::size_of::<$typ>();

        // The same trick as above does not improve the best case speed.
        // It seems to be linked to the way the method is optimised by the compiler
        let mut buf = [0; SIZE];

        let subslice = match buf.get_mut(..$len_to_read) {
            Some(subslice) => subslice,
            None => panic_does_not_fit(SIZE, $len_to_read),
        };

        $this.copy_to_slice(subslice);
        return $typ::from_le_bytes(buf);
    }};
    (be => $this:ident, $typ:tt, $len_to_read:expr) => {{
        const SIZE: usize = core::mem::size_of::<$typ>();

        let slice_at = match SIZE.checked_sub($len_to_read) {
            Some(slice_at) => slice_at,
            None => panic_does_not_fit(SIZE, $len_to_read),
        };

        let mut buf = [0; SIZE];
        $this.copy_to_slice(&mut buf[slice_at..]);
        return $typ::from_be_bytes(buf);
    }};
}

fn sign_extend(val: u64, nbytes: usize) -> i64 {
    let shift = (8 - nbytes) + 8;

    (val << shift) as i64 >> shift
}

pub trait Buf {
    fn remaining(&self) -> usize;

    #[cfg_attr(docsrs, doc(alias = "bytes"))]
    fn chunk(&self) -> &[u8];

    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn chunks_vectored<'a>(&'a self, dst: &mut [IoSlice<'a>]) -> usize {
        if dst.is_empty() {
            return 0;
        }

        if self.has_remaining() {
            dst[0] = IoSlice::new(self.chunk());
            1
        } else {
            0
        }
    }

    fn advance(&mut self, cnt: usize);

    fn has_remaining(&self) -> bool {
        self.remaining() > 0
    }

    fn copy_to_slice(&mut self, mut dst: &mut [u8]) {
        if self.remaining() < dst.len() {
            panic_advance(dst.len(), self.remaining());
        }

        while !dst.is_empty() {
            let src = self.chunk();
            let cnt = usize::min(src.len(), dst.len());

            dst[..cnt].copy_from_slice(&src[..cnt]);
            dst = &mut dst[cnt..];

            self.advance(cnt);
        }
    }

    fn get_u8(&mut self) -> u8 {
        if self.remaining() < 1 {
            panic_advance(1, 0);
        }
        let ret = self.chunk()[0];
        self.advance(1);
        ret
    }

    fn get_i8(&mut self) -> i8 {
        if self.remaining() < 1 {
            panic_advance(1, 0);
        }
        let ret = self.chunk()[0] as i8;
        self.advance(1);
        ret
    }

    fn get_u16(&mut self) -> u16 {
        buf_get_impl!(self, u16::from_be_bytes);
    }

    fn get_u16_le(&mut self) -> u16 {
        buf_get_impl!(self, u16::from_le_bytes);
    }

    fn get_u16_ne(&mut self) -> u16 {
        buf_get_impl!(self, u16::from_ne_bytes);
    }

    fn get_i16(&mut self) -> i16 {
        buf_get_impl!(self, i16::from_be_bytes);
    }

    fn get_i16_le(&mut self) -> i16 {
        buf_get_impl!(self, i16::from_le_bytes);
    }

    fn get_i16_ne(&mut self) -> i16 {
        buf_get_impl!(self, i16::from_ne_bytes);
    }

    fn get_u32(&mut self) -> u32 {
        buf_get_impl!(self, u32::from_be_bytes);
    }

    fn get_u32_le(&mut self) -> u32 {
        buf_get_impl!(self, u32::from_le_bytes);
    }

    fn get_u32_ne(&mut self) -> u32 {
        buf_get_impl!(self, u32::from_ne_bytes);
    }

    fn get_i32(&mut self) -> i32 {
        buf_get_impl!(self, i32::from_be_bytes);
    }

    fn get_i32_le(&mut self) -> i32 {
        buf_get_impl!(self, i32::from_le_bytes);
    }

    fn get_i32_ne(&mut self) -> i32 {
        buf_get_impl!(self, i32::from_ne_bytes);
    }

    fn get_u64(&mut self) -> u64 {
        buf_get_impl!(self, u64::from_be_bytes);
    }

    fn get_u64_le(&mut self) -> u64 {
        buf_get_impl!(self, u64::from_le_bytes);
    }

    fn get_u64_ne(&mut self) -> u64 {
        buf_get_impl!(self, u64::from_ne_bytes);
    }

    fn get_i64(&mut self) -> i64 {
        buf_get_impl!(self, i64::from_be_bytes);
    }

    fn get_i64_le(&mut self) -> i64 {
        buf_get_impl!(self, i64::from_le_bytes);
    }

    fn get_i64_ne(&mut self) -> i64 {
        buf_get_impl!(self, i64::from_ne_bytes);
    }

    fn get_u128(&mut self) -> u128 {
        buf_get_impl!(self, u128::from_be_bytes);
    }

    fn get_u128_le(&mut self) -> u128 {
        buf_get_impl!(self, u128::from_le_bytes);
    }

    fn get_u128_ne(&mut self) -> u128 {
        buf_get_impl!(self, u128::from_ne_bytes);
    }

    fn get_i128(&mut self) -> i128 {
        buf_get_impl!(self, i128::from_be_bytes);
    }

    fn get_i128_le(&mut self) -> i128 {
        buf_get_impl!(self, i128::from_le_bytes);
    }

    fn get_i128_ne(&mut self) -> i128 {
        buf_get_impl!(self, i128::from_ne_bytes);
    }

    fn get_uint(&mut self, nbytes: usize) -> u64 {
        buf_get_impl!(be => self, u64, nbytes);
    }

    fn get_uint_le(&mut self, nbytes: usize) -> u64 {
        buf_get_impl!(le => self, u64, nbytes);
    }

    fn get_uint_ne(&mut self, nbytes: usize) -> u64 {
        if cfg!(target_endian = "big") {
            self.get_uint(nbytes)
        } else {
            self.get_uint_le(nbytes)
        }
    }

    fn get_int(&mut self, nbytes: usize) -> i64 {
        sign_extend(self.get_uint(nbytes), nbytes)
    }

    fn get_int_le(&mut self, nbytes: usize) -> i64 {
        sign_extend(self.get_uint_le(nbytes), nbytes)
    }

    fn get_int_ne(&mut self, nbytes: usize) -> i64 {
        if cfg!(target_endian = "big") {
            self.get_int(nbytes)
        } else {
            self.get_int_le(nbytes)
        }
    }

    fn get_f32(&mut self) -> f32 {
        f32::from_bits(self.get_u32())
    }

    fn get_f32_le(&mut self) -> f32 {
        f32::from_bits(self.get_u32_le())
    }

    fn get_f32_ne(&mut self) -> f32 {
        f32::from_bits(self.get_u32_ne())
    }

    fn get_f64(&mut self) -> f64 {
        f64::from_bits(self.get_u64())
    }

    fn get_f64_le(&mut self) -> f64 {
        f64::from_bits(self.get_u64_le())
    }

    fn get_f64_ne(&mut self) -> f64 {
        f64::from_bits(self.get_u64_ne())
    }

    fn copy_to_bytes(&mut self, len: usize) -> crate::Bytes {
        use super::BufMut;

        if self.remaining() < len {
            panic_advance(len, self.remaining());
        }

        let mut ret = crate::BytesMut::with_capacity(len);

        ret.put(self.take(len));

        ret.freeze()
    }

    fn take(self, limit: usize) -> Take<Self>
    where
        Self: Sized,
    {
        take::new(self, limit)
    }

    fn chain<U>(self, next: U) -> Chain<Self, U>
    where
        U: Buf,
        Self: Sized,
    {
        Chain::new(self, next)
    }

    #[cfg(feature = "std")]
    #[cfg_attr(docsrs, doc(cfg(feature = "std")))]
    fn reader(self) -> Reader<Self>
    where
        Self: Sized,
    {
        reader::new(self)
    }
}

macro_rules! deref_forward_buf {
    () => {
        #[inline]
        fn remaining(&self) -> usize {
            (**self).remaining()
        }

        #[inline]
        fn chunk(&self) -> &[u8] {
            (**self).chunk()
        }

        #[cfg(feature = "std")]
        #[inline]
        fn chunks_vectored<'b>(&'b self, dst: &mut [IoSlice<'b>]) -> usize {
            (**self).chunks_vectored(dst)
        }

        #[inline]
        fn advance(&mut self, cnt: usize) {
            (**self).advance(cnt)
        }

        #[inline]
        fn has_remaining(&self) -> bool {
            (**self).has_remaining()
        }

        #[inline]
        fn copy_to_slice(&mut self, dst: &mut [u8]) {
            (**self).copy_to_slice(dst)
        }

        #[inline]
        fn get_u8(&mut self) -> u8 {
            (**self).get_u8()
        }

        #[inline]
        fn get_i8(&mut self) -> i8 {
            (**self).get_i8()
        }

        #[inline]
        fn get_u16(&mut self) -> u16 {
            (**self).get_u16()
        }

        #[inline]
        fn get_u16_le(&mut self) -> u16 {
            (**self).get_u16_le()
        }

        #[inline]
        fn get_u16_ne(&mut self) -> u16 {
            (**self).get_u16_ne()
        }

        #[inline]
        fn get_i16(&mut self) -> i16 {
            (**self).get_i16()
        }

        #[inline]
        fn get_i16_le(&mut self) -> i16 {
            (**self).get_i16_le()
        }

        #[inline]
        fn get_i16_ne(&mut self) -> i16 {
            (**self).get_i16_ne()
        }

        #[inline]
        fn get_u32(&mut self) -> u32 {
            (**self).get_u32()
        }

        #[inline]
        fn get_u32_le(&mut self) -> u32 {
            (**self).get_u32_le()
        }

        #[inline]
        fn get_u32_ne(&mut self) -> u32 {
            (**self).get_u32_ne()
        }

        #[inline]
        fn get_i32(&mut self) -> i32 {
            (**self).get_i32()
        }

        #[inline]
        fn get_i32_le(&mut self) -> i32 {
            (**self).get_i32_le()
        }

        #[inline]
        fn get_i32_ne(&mut self) -> i32 {
            (**self).get_i32_ne()
        }

        #[inline]
        fn get_u64(&mut self) -> u64 {
            (**self).get_u64()
        }

        #[inline]
        fn get_u64_le(&mut self) -> u64 {
            (**self).get_u64_le()
        }

        #[inline]
        fn get_u64_ne(&mut self) -> u64 {
            (**self).get_u64_ne()
        }

        #[inline]
        fn get_i64(&mut self) -> i64 {
            (**self).get_i64()
        }

        #[inline]
        fn get_i64_le(&mut self) -> i64 {
            (**self).get_i64_le()
        }

        #[inline]
        fn get_i64_ne(&mut self) -> i64 {
            (**self).get_i64_ne()
        }

        #[inline]
        fn get_uint(&mut self, nbytes: usize) -> u64 {
            (**self).get_uint(nbytes)
        }

        #[inline]
        fn get_uint_le(&mut self, nbytes: usize) -> u64 {
            (**self).get_uint_le(nbytes)
        }

        #[inline]
        fn get_uint_ne(&mut self, nbytes: usize) -> u64 {
            (**self).get_uint_ne(nbytes)
        }

        #[inline]
        fn get_int(&mut self, nbytes: usize) -> i64 {
            (**self).get_int(nbytes)
        }

        #[inline]
        fn get_int_le(&mut self, nbytes: usize) -> i64 {
            (**self).get_int_le(nbytes)
        }

        #[inline]
        fn get_int_ne(&mut self, nbytes: usize) -> i64 {
            (**self).get_int_ne(nbytes)
        }

        #[inline]
        fn copy_to_bytes(&mut self, len: usize) -> crate::Bytes {
            (**self).copy_to_bytes(len)
        }
    };
}

impl<T> Buf for &mut T
where
    T: Buf + ?Sized,
{
    deref_forward_buf!();
}

impl<T> Buf for Box<T>
where
    T: Buf + ?Sized,
{
    deref_forward_buf!();
}

impl Buf for &[u8] {
    #[inline]
    fn remaining(&self) -> usize {
        self.len()
    }

    #[inline]
    fn chunk(&self) -> &[u8] {
        self
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        if self.len() < cnt {
            panic_advance(cnt, self.len());
        }

        *self = &self[cnt..];
    }

    #[inline]
    fn copy_to_slice(&mut self, mut dst: &mut [u8]) {
        if self.len() < dst.len() {
            panic_advance(dst.len(), self.len());
        }

        dst.copy_from_slice(&self[..dst.len()]);

        self.advance(dst.len())
    }
}

#[cfg(feature = "std")]
impl<T: AsRef<[u8]>> Buf for std::io::Cursor<T> {
    #[inline]
    fn remaining(&self) -> usize {
        saturating_sub_usize_u64(self.get_ref().as_ref().len(), self.position())
    }

    #[inline]
    fn chunk(&self) -> &[u8] {
        let slice = self.get_ref().as_ref();
        let pos = min_u64_usize(self.position(), slice.len());
        &slice[pos..]
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        let len = self.get_ref().as_ref().len();
        let pos = self.position();

        // We intentionally allow `cnt == 0` here even if `pos > len`.
        let max_cnt = saturating_sub_usize_u64(len, pos);
        if cnt > max_cnt {
            panic_advance(cnt, max_cnt);
        }

        self.set_position(pos + cnt as u64);
    }
}

// The existence of this function makes the compiler catch if the Buf
// trait is "object-safe" or not.
fn _assert_trait_object(_b: &dyn Buf) {}
