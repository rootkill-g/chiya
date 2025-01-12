#![allow(unused)]
#![allow(unknown_lints, unexpected_cfgs)]
#![allow(unconditional_recursion)]
#![warn(missing_docs, missing_debug_implementations, rust_2021_idioms)]
#![doc(test(
    no_crate_inject,
    attr(deny(warnings, rust_2018_idioms), allow(dead_code, unused_variables))
))]
#![no_std]
#![cfg_attr(docsrs, feature(doc_cfg))]

//! Quickie is a Web Framework designed to set you up quickly to get on with development instead of
//! worrying about adding bazzillion crates with specific features

extern crate alloc;
extern crate std;

mod bytes;
