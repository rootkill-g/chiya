use std::sync::atomic::Ordering;

/// An Atomic `()`
/// All Operations are noops
pub struct AtomicUnit;

impl AtomicUnit {
    #[inline]
    pub(crate) fn load(&self, _order: Ordering) {}

    #[inline]
    pub(crate) fn store(&self, _val: (), _order: Ordering) {}

    #[inline]
    pub(crate) fn swap(&self, _val: (), _order: Ordering) {}

    #[inline]
    pub(crate) fn compare_exchange_weak(
        &self,
        _current: (),
        _new: (),
        _success: Ordering,
        _failure: Ordering,
    ) -> Result<(), ()> {
        Ok(())
    }
}
