use core::ffi::{c_int, c_uint, c_ulong, c_void};

use crate::stack::{
    Stack,
    asm::{InitFn, align_down, mut_offset},
};

#[allow(non_camel_case_types)]
pub type off_t = i64;
#[allow(non_camel_case_types)]
pub type rlim_t = u64;
#[allow(non_camel_case_types)]
pub type __rlimit_resource_t = c_uint;
#[allow(non_camel_case_types)]
pub type sighandler_t = size_t;
#[allow(non_camel_case_types)]
pub type size_t = usize;
#[allow(non_camel_case_types)]
pub type greg_t = i64;

pub const _SC_PAGESIZE: c_int = 30;

pub const NULL: *mut c_void = 0 as *mut c_void;

pub const MAP_STACK: c_int = 0x020000;
pub const MAP_PRIVATE: c_int = 0x0002;
pub const MAP_ANON: c_int = 0x0020;
pub const MAP_FAILED: *mut c_void = 0xffffffffffffffff as *mut c_void;

pub const PROT_READ: c_int = 1;
pub const PROT_WRITE: c_int = 2;
pub const PROT_NONE: c_int = 0;

pub const RLIMIT_STACK: __rlimit_resource_t = 3;
pub const RLIM_INFINITY: rlim_t = 18_446_744_073_709_551_615u64; // u64::MAX

pub const SA_SIGINFO: c_int = 0x00000004;
pub const SA_ONSTACK: c_int = 0x08000000;
pub const SIGBUS: c_int = 7;
pub const SIGSEGV: c_int = 11;
pub const SIG_UNBLOCK: c_int = 0x01;

unsafe extern "sysv64" {
    pub fn bootstrap_green_task();
    pub fn prefetch(data: *const usize);
    pub fn swap_registers(out_regs: *mut Register, in_regs: *const Register);
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct rlimit {
    pub rlim_cur: rlim_t,
    pub rlim_max: rlim_t,
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct sigset_t {
    #[cfg(target_pointer_width = "32")]
    __val: [u32; 32],
    #[cfg(target_pointer_width = "64")]
    __val: [u64; 16],
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct sigaction {
    pub sa_sigaction: sighandler_t,
    pub sa_mask: sigset_t,
    pub sa_flags: c_int,
    pub sa_restorer: Option<unsafe extern "C" fn()>,
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct siginfo_t {
    pub si_signo: c_int,
    pub si_errno: c_int,
    pub si_code: c_int,
    _align: [u64; 0],
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct ucontext_t {
    pub uc_flags: c_ulong,
    pub uc_link: *mut ucontext_t,
    pub uc_stack: stack_t,
    pub uc_mcontext: mcontext_t,
    pub uc_sigmask: sigset_t,
    __private: [u8; 512],
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct stack_t {
    pub ss_sp: *mut c_void,
    pub ss_flags: c_int,
    pub ss_size: size_t,
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct mcontext_t {
    pub gregs: [greg_t; 23],
    pub fpregs: *mut _libc_fpstate,
    __private: [u64; 8],
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct _libc_fpstate {
    pub cwd: u16,
    pub swd: u16,
    pub ftw: u16,
    pub fop: u16,
    pub rip: u64,
    pub rdp: u64,
    pub mxcsr: u32,
    pub mxcr_mask: u32,
    pub _st: [_libc_fpxreg; 8],
    pub _xmm: [_libc_xmmreg; 16],
    __private: [u64; 12],
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct _libc_xmmreg {
    pub element: [u32; 4],
}

#[repr(C)]
#[allow(non_camel_case_types)]
pub struct _libc_fpxreg {
    pub sigificand: [u16; 4],
    pub exponent: u16,
    __private: [u16; 3],
}

impl siginfo_t {
    pub unsafe fn si_addr(&self) -> *mut c_void {
        unsafe {
            #[repr(C)]
            #[allow(non_camel_case_types)]
            struct siginfo_sigfault {
                _si_signo: c_int,
                _si_errno: c_int,
                _si_code: c_int,
                _si_addr: *mut c_void,
            }

            (*(self as *const siginfo_t as *const siginfo_sigfault))._si_addr
        }
    }
}

#[repr(C)]
#[derive(Debug)]
pub struct Register {
    gpr: [usize; 8],
}

impl Register {
    pub fn new() -> Register {
        Register { gpr: [0; 8] }
    }

    #[inline]
    pub fn prefetch(&self) {
        let ptr = self.gpr[1] as *const usize;

        unsafe {
            prefetch(ptr); // RSP
            prefetch(ptr.add(8)); // RSP + 8
        }
    }
}

pub fn initialize_call_frame(
    regs: &mut Register,
    fptr: InitFn,
    arg: usize,
    arg2: *mut usize,
    stack: &Stack,
) {
    // Redefinitions from runtime/arch/x86_64/regs.h
    const RUSTRT_RSP: usize = 1;
    const RUSTRT_RBP: usize = 2;
    const RUSTRT_R12: usize = 4;
    const RUSTRT_R13: usize = 5;
    const RUSTRT_R14: usize = 6;

    let sp = align_down(stack.end());

    // These registers are frobbed by bootstrap_green_task into the right location so we can
    // invoke the "real init function", `fptr`
    regs.gpr[RUSTRT_R12] = arg;
    regs.gpr[RUSTRT_R13] = arg2 as usize;
    regs.gpr[RUSTRT_R14] = fptr as usize;

    // Last base pointer on the stack should be 0
    regs.gpr[RUSTRT_RBP] = 0;

    // Setup the init stack
    // This is prepared for the swap context
    regs.gpr[RUSTRT_RSP] = mut_offset(sp, -2) as usize;

    unsafe {
        // Leave enough space for RET
        *mut_offset(sp, -2) = bootstrap_green_task as usize;
        *mut_offset(sp, -1) = 0;
    }
}
