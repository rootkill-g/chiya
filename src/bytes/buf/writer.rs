use crate::bytes::BufMut;
#[cfg(feature = "std")]
use std::{cmp, io};

/// A `ButMut` adapter which implements `io::Write` for the inner value
#[derive(Debug)]
pub struct Writer<B> {
    buf: B,
}

/// Creates a new `Writer` adapter with the underlying `Buf`
pub fn new<B>(buf: B) -> Writer<B> {
    Writer { buf }
}

impl<B> Writer<B>
where
    B: BufMut,
{
    /// Gets a reference to the underlying `BufMut`
    pub fn get_ref(&self) -> &B {
        &self.buf
    }

    /// Gets a mutable reference to the underlying `BufMut`
    pub fn get_mut(&mut self) -> &mut B {
        &mut self.buf
    }

    /// Consumes this `Writer`, returning the underlying value
    pub fn into_inner(self) -> B {
        self.buf
    }
}

impl<B> io::Write for Writer<B>
where
    B: BufMut + Sized,
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = cmp::min(self.buf.remaining_mut(), buf.len());

        self.buf.put_slice(&buf[..n]);

        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
