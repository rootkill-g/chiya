use core::ffi::{c_int, c_long, c_void};
use std::{
    io,
    sync::atomic::{AtomicUsize, Ordering},
};

use crate::sys_stack::SysStack;
use x86_64::{
    __rlimit_resource_t, _SC_PAGESIZE, MAP_ANON, MAP_FAILED, MAP_PRIVATE, MAP_STACK, NULL,
    PROT_NONE, PROT_READ, PROT_WRITE, off_t, rlimit, sigaction, sigset_t, size_t,
};

pub mod overflow;
mod x86_64;

unsafe extern "C" {
    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "mmap$UNIX2003"
    )]
    fn mmap(
        addr: *mut c_void,
        len: size_t,
        prot: c_int,
        flags: c_int,
        fd: c_int,
        offset: off_t,
    ) -> *mut c_void;

    fn mprotect(addr: *mut c_void, len: size_t, prot: c_int) -> c_int;

    #[cfg_attr(
        all(target_os = "macos", target_arch = "x86"),
        link_name = "munmap$UNIX2003"
    )]
    fn munmap(addr: *mut c_void, len: size_t) -> c_int;

    #[cfg_attr(target_os = "solaris", link_name = "__sysconf_xpg7")]
    fn sysconf(name: c_int) -> c_long;

    fn getrlimit(resource: __rlimit_resource_t, rlim: *mut rlimit) -> c_int;

    #[cfg_attr(target_os = "netbsd", link_name = "__sigaction14")]
    pub fn sigaction(signum: c_int, act: *const sigaction, oldact: *mut sigaction) -> c_int;

    #[cfg_attr(target_os = "netbsd", link_name = "__sigaddset14")]
    fn sigaddset(set: *mut sigset_t, signum: c_int) -> c_int;

    #[cfg_attr(target_os = "netbsd", link_name = "__sigemptyset14")]
    fn sigemptyset(set: *mut sigset_t) -> c_int;

    #[cfg_attr(target_os = "netbsd", link_name = "__sigprocmask14")]
    fn sigprocmask(how: c_int, set: *const sigset_t, oldset: *mut sigset_t) -> c_int;
}

pub unsafe fn allocate_stack(size: usize) -> io::Result<SysStack> {
    unsafe {
        const PROT: c_int = PROT_READ | PROT_WRITE;
        const TYPE: c_int = MAP_PRIVATE | MAP_ANON | MAP_STACK;

        let ptr = mmap(NULL, size, PROT, TYPE, -1, 0);

        if ptr == MAP_FAILED {
            Err(io::Error::last_os_error())
        } else {
            Ok(SysStack::new(
                (ptr as usize + size) as *mut c_void,
                ptr as *mut c_void,
            ))
        }
    }
}

pub unsafe fn protect_stack(stack: &SysStack) -> io::Result<SysStack> {
    unsafe {
        let page_size = page_size();

        debug_assert!(stack.len() % page_size == 0 && stack.len() != 0);

        let ret = {
            let bottom = stack.bottom();

            mprotect(bottom, page_size, PROT_NONE)
        };

        if ret != 0 {
            Err(io::Error::last_os_error())
        } else {
            let bottom = (stack.bottom() as usize + page_size) as *mut c_void;

            Ok(SysStack::new(stack.top(), bottom))
        }
    }
}

pub unsafe fn deallocate_stack(ptr: *mut c_void, size: usize) {
    unsafe {
        munmap(ptr, size);
    }
}

pub fn page_size() -> usize {
    static PAGE_SIZE: AtomicUsize = AtomicUsize::new(0);

    let mut ret = PAGE_SIZE.load(Ordering::Relaxed);

    if ret == 0 {
        unsafe {
            ret = sysconf(_SC_PAGESIZE) as usize;
        }

        PAGE_SIZE.store(ret, Ordering::Relaxed);
    }

    ret
}

pub fn min_stack_size() -> usize {
    page_size()
}

#[cfg(not(target_os = "fuchsia"))]
pub fn max_stack_size() -> usize {
    use std::mem::MaybeUninit;

    use x86_64::{RLIM_INFINITY, RLIMIT_STACK, rlim_t};

    static PAGE_SIZE: AtomicUsize = AtomicUsize::new(0);

    let mut ret = PAGE_SIZE.load(Ordering::Relaxed);

    if ret == 0 {
        let mut limit = MaybeUninit::uninit();
        let limit_ret = unsafe { getrlimit(RLIMIT_STACK, limit.as_mut_ptr()) };
        let limit = unsafe { limit.assume_init() };

        if limit_ret == 0 {
            ret = if limit.rlim_max == RLIM_INFINITY || limit.rlim_max > (usize::MAX as rlim_t) {
                usize::MAX
            } else {
                limit.rlim_max as usize
            };

            PAGE_SIZE.store(ret, Ordering::Relaxed);
        } else {
            ret = 1024 * 1024 * 1024;
        }
    }

    ret
}
