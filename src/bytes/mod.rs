//! Module provides abstractions for working with bytes

extern crate alloc;

/// Using `std` crate for when the feature `std` is enabled
#[cfg(feature = "std")]
extern crate std;

/// Importing and using the `buf` module and it's adapters
pub mod buf;
pub use buf::{Buf, BufMut};

/// Importing and using the `fmt` module and it's adapters
pub mod fmt;
//pub use fmt::

mod bytes;
mod bytes_mut;
mod quick;

pub use bytes::Bytes;
pub use bytes_mut::BytesMut;

#[cold]
pub fn panic_advance(idx: usize, len: usize) -> ! {
    panic!(
        "advance out of bounds: the len is: {} but advancing by {}",
        len, idx
    );
}

#[inline]
fn panic_does_not_fit(size: usize, nbytes: usize) -> ! {
    panic!(
        "size too large: the integer type can fit {} bytes, but nbytes is {}",
        size, nbytes
    );
}

#[inline]
#[cfg(feature = "std")]
fn saturating_sub_usize_u64(a: usize, b: u64) -> usize {
    use core::convert::TryFrom;

    match usize::try_from(b) {
        Ok(b) => a.saturating_sub(b),
        Err(_) => 0,
    }
}

#[inline]
#[cfg(feature = "std")]
fn min_u64_usize(a: u64, b: usize) -> usize {
    use core::convert::TryFrom;

    match usize::try_from(a) {
        Ok(a) => usize::min(a, b),
        Err(_) => b,
    }
}

#[inline]
fn offset_from(dst: *const u8, original: *const u8) -> usize {
    dst as usize - original as usize
}

#[inline]
fn abort() -> ! {
    #[cfg(feature = "std")]
    {
        std::process::abort();
    }
    #[cfg(not(feature = "std"))]
    {
        struct Abort;

        impl Drop for Abort {
            fn drop(&mut self) {
                panic!()
            }
        }

        let _a = Abort;

        panic!("abort");
    }
}
