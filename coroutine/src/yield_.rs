/// Yield
/// Generator yield implementation
use std::any::Any;
use std::sync::atomic;

use crate::runtime::{Context, ContextStack, is_generator};

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
