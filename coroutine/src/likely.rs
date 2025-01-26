use crate::cold::cold;

#[inline]
pub const fn likely(b: bool) -> bool {
    if !b {
        cold()
    }

    b
}
