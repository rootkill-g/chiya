use super::{uninit_slice::UninitSlice, Buf, BufMut, IntoIter};
use crate::{Bytes, BytesMut};

#[cfg(feature = "std")]
use std::io::IoSlice;

#[derive(Debug)]
pub struct Chain<T, U> {
    a: T,
    b: U,
}

impl<T, U> Chain<T, U> {
    pub(crate) fn new(a: T, b: U) -> Chain<T, U> {
        Chain { a, b }
    }

    pub fn first_ref(&self) -> &T {
        &self.a
    }

    pub fn first_mut(&mut self) -> &mut T {
        &mut self.a
    }

    pub fn last_ref(&self) -> &U {
        &self.b
    }

    pub fn last_mut(&mut self) -> &mut U {
        &mut self.b
    }

    pub fn into_inner(self) -> (T, U) {
        (self.a, self.b)
    }
}

impl<T, U> Buf for Chain<T, U>
where
    T: Buf,
    U: Buf,
{
    fn remaining(&self) -> usize {
        self.a.remaining().saturating_add(self.b.remaining())
    }

    fn chunk(&self) -> &[u8] {
        if self.a.has_remaining() {
            self.a.chunk()
        } else {
            self.b.chunk()
        }
    }

    fn advance(&mut self, mut cnt: usize) {
        let a_rem = self.a.remaining();

        if a_rem != 0 {
            if a_rem > cnt {
                self.a.advance(cnt);
                return;
            }

            self.a.advance(a_rem);

            cnt -= a_rem;
        }

        self.b.advance(cnt);
    }

    #[cfg(feature = "std")]
    fn chunks_vectored<'a>(&'a self, dst: &mut [IoSlice<'a>]) -> usize {
        let mut n = self.a.chunks_vectored(dst);

        n += self.b.chunks_vectored(&mut dst[..n]);

        n
    }

    fn copy_to_bytes(&mut self, len: usize) -> Bytes {
        let a_rem = self.a.remaining();

        if a_rem >= len {
            self.a.copy_to_bytes(len)
        } else if a_rem == 0 {
            self.b.copy_to_bytes(len)
        } else {
            assert!(
                len - a_rem <= self.b.remaining(),
                "`len` greater than remaining"
            );

            let mut ret = BytesMut::with_capacity(len);

            ret.put(&mut self.a);

            ret.put((&mut self.b).take(len - a_rem));

            ret.freeze()
        }
    }
}

unsafe impl<T, U> BufMut for Chain<T, U>
where
    T: BufMut,
    U: BufMut,
{
    fn remaining_mut(&self) -> usize {
        self.a
            .remaining_mut()
            .saturating_add(self.b.remaining_mut())
    }

    fn chunk_mut(&mut self) -> &mut UninitSlice {
        if self.a.has_remaining_mut() {
            self.a.chunk_mut()
        } else {
            self.b.chunk_mut()
        }
    }

    unsafe fn advance_mut(&mut self, mut cnt: usize) {
        let a_rem = self.a.remaining_mut();

        if a_rem != 0 {
            if a_rem >= cnt {
                self.a.advance_mut(cnt);
                return;
            }

            self.a.advance_mut(a_rem);

            cnt -= a_rem;
        }

        self.b.advance_mut(cnt);
    }
}

impl<T, U> IntoIterator for Chain<T, U>
where
    T: Buf,
    U: Buf,
{
    type Item = u8;
    type IntoIter = IntoIter<Chain<T, U>>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self)
    }
}
