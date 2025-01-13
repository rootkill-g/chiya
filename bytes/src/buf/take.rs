use crate::{Buf, Bytes};
use core::cmp;

/// A `Buf` adapter which limits the bytes read from an underlying buffer.
/// This struct is generally created by calling `take()` on `Buf`.
#[derive(Debug)]
pub struct Take<T> {
    inner: T,
    limit: usize,
}

/// Create a new Take object
pub fn new<T>(inner: T, limit: usize) -> Take<T> {
    Take { inner, limit }
}

impl<T> Take<T> {
    /// Consumes this `Take`, returning the underlying value
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Gets a reference to the underlying `Buf`
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Gets a mutable reference to the underlying `Buf`
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Returns the maximum number of bytes that can be read
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Sets the maximum number of bytes that can be read
    pub fn set_limit(&mut self, new_limit: usize) {
        self.limit = new_limit;
    }
}

impl<T> Buf for Take<T>
where
    T: Buf,
{
    fn remaining(&self) -> usize {
        cmp::min(self.inner.remaining(), self.limit)
    }

    fn chunk(&self) -> &[u8] {
        let bytes = self.inner.chunk();

        &bytes[..cmp::min(bytes.len(), self.limit)]
    }

    fn advance(&mut self, cnt: usize) {
        assert!(cnt <= self.limit);

        self.inner.advance(cnt);
        self.limit -= cnt;
    }

    fn copy_to_bytes(&mut self, len: usize) -> Bytes {
        assert!(len <= self.remaining(), "`len` greater than remaining");

        let r = self.inner.copy_to_bytes(len);

        self.limit -= len;

        r
    }
}
