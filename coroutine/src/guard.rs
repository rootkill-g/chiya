use std::ops::Range;

pub type Guard = Range<usize>;

pub fn current() -> Guard {
    assert!(is_coroutine());
}
