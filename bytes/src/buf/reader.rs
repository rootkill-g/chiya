use crate::Buf;
#[cfg(feature = "std")]
use std::{cmp, io};

/// A `Buf` adapter which implements `io::Read` for the inner value
#[derive(Debug)]
pub struct Reader<B> {
    buf: B,
}

/// Creates a new `Reader` adapter with the underlying `Buf`
pub fn new<B>(buf: B) -> Reader<B> {
    Reader { buf }
}

impl<B> Reader<B>
where
    B: Buf,
{
    /// Gets a reference to the underlying `Buf`
    pub fn get_ref(&self) -> &B {
        &self.buf
    }

    /// Gets a mutable reference to the underlying `Buf`
    pub fn get_mut(&mut self) -> &mut B {
        &mut self.buf
    }

    /// Consumes this `Reader`, returns the underlying value
    pub fn into_inner(self) -> B {
        self.buf
    }
}

impl<B> io::Read for Reader<B>
where
    B: Buf + Sized,
{
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let len = cmp::min(self.buf.remaining(), buf.len());

        Buf::copy_to_slice(&mut self.buf, &mut buf[0..len]);

        Ok(len)
    }
}

impl<B> io::BufRead for Reader<B>
where
    B: Buf + Sized,
{
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        Ok(self.buf.chunk())
    }

    fn consume(&mut self, amt: usize) {
        self.buf.advance(amt);
    }
}
