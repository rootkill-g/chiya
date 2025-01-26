use std::ops::Range;

use crate::{
    runtime::{ContextStack, is_generator},
    stack::page_size,
};

pub type Guard = Range<usize>;

pub fn current() -> Guard {
    assert!(is_generator());

    let guard = unsafe { (*(*ContextStack::current().root).child).stack_guard };

    guard.0 - page_size()..guard.1
}
