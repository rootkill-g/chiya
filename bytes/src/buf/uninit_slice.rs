use core::{
    fmt,
    mem::MaybeUninit,
    ops::{
        Index, IndexMut, Range, RangeFrom, RangeFull, RangeInclusive, RangeTo, RangeToInclusive,
    },
    slice,
};

/// Uninitialized byte slice:
///
/// Returned by `BufMut::chunk_mut()`, the referenced byte slice may be uninitialized.
/// The wrapper provides safe access without introducing undefined behavior.
///
/// The safety invariants of this wrapper are:
///
///  1. Reading from an `UninitSlice` is undefined behavior.
///  2. Writing uninitialized bytes to an `UninitSlice` is undefined behavior.
///
/// The difference between `&mut UninitSlice` and `&mut [MaybeUninit<u8>]` is that it is possible in
/// safe code to write uninitialized bytes to an `&mut [MaybeUninit<u8>]`, which this type prohibits
#[repr(transparent)]
pub struct UninitSlice([MaybeUninit<u8>]);

impl UninitSlice {
    /// Creates a `&mut UninitSlice` wrapping a slice of initiatialized memory
    #[inline]
    pub fn new(slice: &mut [u8]) -> &mut UninitSlice {
        unsafe { &mut *(slice as *mut [u8] as *mut [MaybeUninit<u8>] as *mut UninitSlice) }
    }

    /// Creates a `&mut UninitiSlice` wrapping a slice of uninitialized memory
    #[inline]
    pub fn uninit(slice: &mut [MaybeUninit<u8>]) -> &mut UninitSlice {
        unsafe { &mut *(slice as *mut [MaybeUninit<u8>] as *mut UninitSlice) }
    }

    fn uninit_ref(slice: &[MaybeUninit<u8>]) -> &UninitSlice {
        unsafe { &*(slice as *const [MaybeUninit<u8>] as *const UninitSlice) }
    }

    /// Creates a `&mut UninitSlice` from a pointer and a length
    #[inline]
    pub unsafe fn from_raw_parts_mut<'a>(ptr: *mut u8, len: usize) -> &'a mut UninitSlice {
        unsafe {
            let maybe_init: &mut [MaybeUninit<u8>] = slice::from_raw_parts_mut(ptr as *mut _, len);

            Self::uninit(maybe_init)
        }
    }

    /// Write a single byte at the specified offset
    #[inline]
    pub fn write_byte(&mut self, index: usize, byte: u8) {
        assert!(index < self.len());

        unsafe { self[index..].as_mut_ptr().write(byte) }
    }

    /// Copies bytes from `src` into `self`
    #[inline]
    pub fn copy_from_slice(&mut self, slice: &[u8]) {
        use core::ptr;

        assert_eq!(self.len(), slice.len());

        unsafe {
            ptr::copy(slice.as_ptr(), self.as_mut_ptr(), self.len());
        }
    }

    /// Returns a raw pointer to the slice's buffer
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut u8 {
        self.0.as_mut_ptr() as *mut _
    }

    /// Returns a `&mut [MaybeUninit]` to the slice's buffer
    #[inline]
    pub unsafe fn as_uninit_slice_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        &mut self.0
    }

    /// Returns the number of bytes in the slice
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl fmt::Debug for UninitSlice {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("UninitSlice[...]").finish()
    }
}

impl<'a> From<&'a mut [u8]> for &'a mut UninitSlice {
    fn from(slice: &'a mut [u8]) -> Self {
        UninitSlice::new(slice)
    }
}

impl<'a> From<&'a mut [MaybeUninit<u8>]> for &'a mut UninitSlice {
    fn from(slice: &'a mut [MaybeUninit<u8>]) -> Self {
        UninitSlice::uninit(slice)
    }
}

// Macro Rules
macro_rules! impl_index {
    ($($t:ty),*) => {
        $(
            impl Index<$t> for UninitSlice {
                type Output = UninitSlice;

                #[inline]
                fn index(&self, index: $t) -> &UninitSlice {
                    UninitSlice::uninit_ref(&self.0[index])
                }
            }

            impl IndexMut<$t> for UninitSlice {
                #[inline]
                fn index_mut(&mut self, index: $t) -> &mut UninitSlice {
                    UninitSlice::uninit(&mut self.0[index])
                }
            }
        )*
    };
}

impl_index!(
    Range<usize>,
    RangeFrom<usize>,
    RangeFull,
    RangeInclusive<usize>,
    RangeTo<usize>,
    RangeToInclusive<usize>
);
