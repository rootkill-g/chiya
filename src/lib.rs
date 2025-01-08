#![allow(unknown_lints, unexpected_cfgs)]
#![warn(missing_docs, missing_debug_implementations, rust_2021_idioms)]
#![doc(test(
    no_crate_inject,
    attr(deny(warnings, rust_2018_idioms), allow(dead_code, unused_variables))
))]
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]

extern crate alloc;

#[cfg(feature = "std")]
extern crate std;

pub mod buf;
pub use crate::buf::{Buf, BufMut};

mod bytes;
mod bytes_mut;
pub use bytes::Bytes;
pub use bytes_mut::BytesMut;

pub mod quick;

#[cold]
fn panic_advance(idx: usize, len: usize) -> ! {
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
