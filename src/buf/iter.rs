use super::Buf;

#[derive(Debug)]
pub struct IntoIter<T> {
    inner: T,
}

impl<T> IntoIter<T> {
    pub fn new(inner: T) -> IntoIter<T> {
        IntoIter { inner }
    }

    pub fn into_inner(self) -> T {
        self.inner
    }

    pub fn get_ref(&self) -> &T {
        &self.inner
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.inner
    }
}

impl<T: Buf> Iterator for IntoIter<T> {
    type Item = u8;

    fn next(&mut self) -> Option<u8> {
        if !self.inner.has_remaining() {
            return None;
        }

        let b = self.inner.chunk()[0];

        self.inner.advance(1);

        Some(b)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let rem = self.inner.remaining();

        (rem, Some(rem))
    }
}

impl<T: Buf> ExactSizeIterator for IntoIter<T> {}
