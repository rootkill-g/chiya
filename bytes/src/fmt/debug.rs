use super::BytesRef;
use crate::{Bytes, BytesMut};
use core::fmt::{Debug, Formatter, Result};

/// Alternative implementation of `std::fmt::Debug` for byte slice.
///
/// Standard `Debug` implementation for `[u8]` is comma separated list of numbers. Since large
/// amount of byte strings are in fact ASCII strings or contain a lot of ASCII strins (e.g. HTTP),
/// it is convenient to print strings to ASCII when possible
impl Debug for BytesRef<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result {
        for &b in self.0 {
            if b == b'\n' {
                write!(f, "\\n")?;
            } else if b == b'\r' {
                write!(f, "\\r")?;
            } else if b == b'\t' {
                write!(f, "\\t")?;
            } else if b == b'\\' || b == b'"' {
                write!(f, "\\{}", b as char)?;
            } else if b == b'\0' {
                write!(f, "\\0")?;
            }
            // ASCII printable
            else if (0x20..0x7f).contains(&b) {
                write!(f, "{}", b as char)?;
            } else {
                write!(f, "\\x{:02x}", b)?;
            }
        }

        Ok(())
    }
}

fmt_impl!(Debug, Bytes);
fmt_impl!(Debug, BytesMut);
