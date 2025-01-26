/// Register contexts used in various architectures
#[inline]
pub(crate) fn align_down(sp: *mut usize) -> *mut usize {
    let sp = (sp as usize) & !(16 - 1);

    sp as *mut usize
}

#[inline]
pub(crate) fn mut_offset<T>(ptr: *mut T, count: isize) -> *mut T {
    unsafe { ptr.offset(count) }
}

pub(crate) type InitFn = extern "sysv64" fn(usize, *mut usize) -> !;
