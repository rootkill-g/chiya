use std::sync::Arc;

use super::{AtomicCell, blocker::Blocker};

pub struct AtomicOption<T> {
    inner: AtomicCell<Option<T>>,
}

const _: () = assert!(AtomicCell::<Option<CoroutineImpl>>::is_lock_free());
const _: () = assert!(AtomicCell::<Option<Arc<Blocker>>>::is_lock_free());

impl<T> AtomicOption<T> {
    pub const fn none() -> AtomicOption<T> {
        AtomicOption {
            inner: AtomicCell::new(None),
        }
    }
}
