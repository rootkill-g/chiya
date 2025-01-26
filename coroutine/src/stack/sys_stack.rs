use std::os::raw::c_void;

use crate::stack::{stack_error::StackError, unix};

/// Represents any kind of stack memory
#[derive(Debug)]
pub struct SysStack {
    pub(crate) top: *mut c_void,
    pub(crate) bottom: *mut c_void,
}

impl SysStack {
    /// Creates a (non-owning) representation of stack memory
    #[inline]
    pub unsafe fn new(top: *mut c_void, bottom: *mut c_void) -> SysStack {
        debug_assert!(top >= bottom);

        SysStack { top, bottom }
    }

    /// Returns the top of the stack from which it grows downwards
    #[inline]
    pub fn top(&self) -> *mut c_void {
        self.top
    }

    /// Returns the bottom of the stack representing the end of stack
    #[inline]
    pub fn bottom(&self) -> *mut c_void {
        self.bottom
    }

    /// Returns the length of the stack
    #[inline]
    pub fn len(&self) -> usize {
        self.top as usize - self.bottom as usize
    }

    /// Returns the minimum stack size allowed by the current platform
    #[inline]
    pub fn min_size() -> usize {
        unix::min_stack_size()
    }

    /// Allocates a new stack of size: `size`
    pub(crate) fn allocate(mut size: usize, protected: bool) -> Result<SysStack, StackError> {
        let page_size = unix::page_size();
        let min_stack_size = unix::min_stack_size();
        let max_stack_size = unix::max_stack_size();
        let add_shift = i32::from(protected);
        let add = page_size << add_shift;

        if size < min_stack_size {
            size = min_stack_size;
        }

        size = (size - 1) & !(page_size.overflowing_sub(1).0);

        if let Some(size) = size.checked_add(add) {
            if size <= max_stack_size {
                let mut ret = unsafe { unix::allocate_stack(size) };

                if protected {
                    if let Ok(stack) = ret {
                        ret = unsafe { unix::protect_stack(&stack) };
                    }
                }

                return ret.map_err(StackError::IoError);
            }
        }

        Err(StackError::ExceedsMaximumSize(max_stack_size - add))
    }
}

unsafe impl Send for SysStack {}
