//! Yield
//! Generator yield implementation
use std::any::Any;
use std::sync::atomic;

use crate::{
    event::EventResult,
    runtime::{Context, ContextStack, is_generator},
};

/// This is a special return instruction that yield nothing but terminates the generator safely
#[macro_export]
macro_rules! done {
    () => {{
        return $crate::done();
    }};
}

// WARN: Don't use this directly, use done!() macro instead
// Would panic if used in none generator context
#[inline]
pub fn done<T>() -> T {
    assert!(is_generator(), "done is only possible in a generator");

    std::panic::panic_any(crate::error::Error::Done);
}

/// Switch back to parent context
#[inline]
pub fn yield_now() {
    let env = ContextStack::current();
    let cur = env.top();
    raw_yield_now(&env, cur);
}

#[inline]
pub fn raw_yield_now(env: &ContextStack, cur: &mut Context) {
    let parent = env.pop_context(cur as *mut _);

    RegContext::swap(&mut cur.regs, &parent.regs);
}

#[inline]
pub fn get_coroutine_para() -> Option<EventResult> {
    coroutine_get_yield::<EventResult>()
}

/// Coroutine get passed in yield para
fn coroutine_get_yield<T: Any>() -> Option<T> {
    ContextStack::current()
        .coroutine_ctx()
        .and_then(|ctx| ctx.coroutine_get_para())
}
