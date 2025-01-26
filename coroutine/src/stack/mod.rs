use std::{os::raw::c_void, ptr};

//pub(crate) use asm::{InitFn, align_down, mut_offset};
pub use sys_stack::SysStack;
pub use unix::{page_size, x86_64::Registers};

mod asm;
mod stack_error;
mod sys_stack;
mod unix;

/// Generator stack
/// This struct will not deallocate the memory
/// `StackBox` will track and deallocate it
pub struct Stack {
    buf: SysStack,
}

impl Stack {
    /// Allocate a new stack of size: `size`. If size == 0, it is a `dummy_stack`
    pub fn new(size: usize) -> Stack {
        let track = (size & 1) != 0;
        let bytes = usize::max(size * std::mem::size_of::<usize>(), SysStack::min_size());
        let buf = SysStack::allocate(bytes, true).expect("Failed to allocate sys stack");
        let stack = Stack { buf };

        // If size is not `even` we do the full footprint test
        let count = if track {
            stack.size()
        } else {
            // We only check the last few words
            8
        };

        unsafe {
            let buf = stack.buf.bottom as *mut usize;

            ptr::write_bytes(buf, 0xEE, count);
        }

        // Initialize the box usage
        let offset = stack.get_offset();

        unsafe { *offset = 1 };

        stack
    }

    /// Get used stack size
    pub fn get_used_size(&self) -> usize {
        let mut offset = 0usize;

        unsafe {
            let mut magic = 0xEEusize;

            ptr::write_bytes(&mut magic, 0xEE, 1);

            let mut ptr = self.buf.bottom as *mut usize;

            while *ptr == magic {
                offset += 1;

                ptr = ptr.offset(1);
            }
        }

        let cap = self.size();

        cap - offset
    }

    /// Get the stack capacity
    #[inline]
    pub fn size(&self) -> usize {
        self.buf.len() / std::mem::size_of::<usize>()
    }

    /// Point to the high end of the allocated stack
    pub fn end(&self) -> *mut usize {
        let offset = self.get_offset();

        unsafe { (self.buf.top as *mut usize).offset(0 - *offset as isize) }
    }

    /// Point to the low end of the allocated stack
    pub fn begin(&self) -> *mut usize {
        self.buf.bottom as *mut _
    }

    /// Get offset
    fn get_offset(&self) -> *mut usize {
        unsafe { (self.buf.top as *mut usize).offset(1) }
    }

    /// Deallocate the stack
    fn drop_stack(&self) {
        if self.buf.len() == 0 {
            return;
        }

        let page_size = unix::page_size();
        let guard = (self.buf.bottom as usize - page_size) as *mut c_void;
        let size_with_guard = self.buf.len() + page_size;

        unsafe { unix::deallocate_stack(guard, size_with_guard) };
    }
}
