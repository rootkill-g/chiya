use super::{BufMut, UninitSlice};
use core::cmp;

#[derive(Debug)]
pub struct Limit<T> {
    inner: T,
    limit: usize,
}

pub(super) fn new<T>(inner: T, limit: usize) -> Limit<T> {
    Limit { inner, limit }
}

impl<T> Limit<T> {
    pub fn into_inner(self) -> T {
        self.inner
    }

    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }

    pub fn limit(&self) -> usize {
        self.limit
    }

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
