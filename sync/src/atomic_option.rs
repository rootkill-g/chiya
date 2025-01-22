use crate::AtomicCell;

pub struct AtomicOption<T> {
    inner: AtomicCell<Option<T>>,
}
