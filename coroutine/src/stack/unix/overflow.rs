use core::ffi::c_int;
use std::{
    backtrace::Backtrace,
    mem::{self, MaybeUninit},
    process,
    ptr::null_mut,
    sync::{Mutex, Once},
    thread::{self, yield_now},
};

use crate::stack::unix::{
    sigaction, sigaddset, sigemptyset, sigprocmask,
    x86_64::{SIG_UNBLOCK, sighandler_t, siginfo_t, sigset_t, ucontext_t},
};

use super::x86_64::{SA_ONSTACK, SA_SIGINFO, SIGBUS, SIGSEGV};

static SIG_ACTION: Mutex<MaybeUninit<sigaction>> = Mutex::new(MaybeUninit::uninit());

unsafe extern "C" fn signal_handler(signum: c_int, info: *mut siginfo_t, ctx: *mut ucontext_t) {
    unsafe {
        let _ctx = &mut *ctx;
        let addr = (*info).si_addr() as usize;
        let stack_guard = crate::guard::current();

        if !stack_guard.contains(&addr) {
            println!("{}", Backtrace::force_capture());

            // SIG_ACTION is available after we registered our handler
            let old_action = SIG_ACTION.lock().unwrap();

            sigaction(signum, old_action.assume_init_ref(), null_mut());

            // We are unsable to handle this
            return;
        }

        eprintln!(
            "\nCoroutine in thread '{}' has overflowed it's stack\n",
            thread::current().name().unwrap_or("<unknown>")
        );

        crate::runtime::ContextStack::current().top().err = Some(Box::new(rt::Error::StackErr));

        let mut sigset: sigset_t = mem::zeroed();

        sigemptyset(&mut sigset);
        sigaddset(&mut sigset, signum);
        sigprocmask(SIG_UNBLOCK, &sigset, null_mut());

        yield_now();

        process::abort();
    }
}

#[cold]
unsafe fn init() {
    unsafe {
        let mut action: sigaction = mem::zeroed();

        action.sa_flags = SA_SIGINFO | SA_ONSTACK;
        action.sa_sigaction = signal_handler as sighandler_t;

        let mut old_action = SIG_ACTION.lock().unwrap();

        for signal in [SIGSEGV, SIGBUS] {
            sigaction(signal, &action, old_action.assume_init_mut());
        }
    }
}

pub fn init_once() {
    static INIT_ONCE: Once = Once::new();

    INIT_ONCE.call_once(|| unsafe { init() });
}
