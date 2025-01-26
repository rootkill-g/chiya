use crate::cold::cold;

#[inline]
pub const fn unlikely(b: bool) -> bool {
    if b {
        cold()
    }

    b
}
