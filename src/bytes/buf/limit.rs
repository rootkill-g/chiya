use super::{BufMut, UninitSlice};
use core::cmp;

/// A `ButMut` adapter which limits the amount of bytes that can be written to an underlying buffer
#[derive(Debug)]
pub struct Limit<T> {
    inner: T,
    limit: usize,
}

pub(super) fn new<T>(inner: T, limit: usize) -> Limit<T> {
    Limit { inner, limit }
}

impl<T> Limit<T> {
    /// Consumes the `Limit`, returning the underlying value
    pub fn into_inner(self) -> T {
        self.inner
    }

    /// Gets a reference to the underlying `BufMut`.
    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    /// Gets a mutable reference to the underlying `BufMut`
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    /// Returns the maximum number of bytes that can be written
    pub fn limit(&self) -> usize {
        self.limit
    }

    /// Sets the maximum number of bytes that can be written
    pub fn set_limit(&mut self, limit: usize) {
        self.limit = limit
    }
}

unsafe impl<T> BufMut for Limit<T>
where
    T: BufMut,
{
    fn remaining_mut(&self) -> usize {
        cmp::min(self.inner.remaining_mut(), self.limit)
    }

    fn chunk_mut(&mut self) -> &mut UninitSlice {
        let bytes = self.inner.chunk_mut();
        let end = cmp::min(bytes.len(), self.limit);

        &mut bytes[..end]
    }

    unsafe fn advance_mut(&mut self, cnt: usize) {
        assert!(cnt <= self.limit);

        self.inner.advance_mut(cnt);
        self.limit -= cnt
    }
}
