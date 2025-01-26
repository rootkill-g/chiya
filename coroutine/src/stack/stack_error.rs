use std::{error::Error, fmt, io};

/// Error type returned by stack allocation methods
#[derive(Debug)]
pub enum StackError {
    /// Contains the maximum amount of memory allowed to be allocated as stack space
    ExceedsMaximumSize(usize),

    /// Returned if some kind of I/O error happens during allocation
    IoError(io::Error),
}

impl fmt::Display for StackError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match *self {
            StackError::ExceedsMaximumSize(size) => {
                write!(
                    f,
                    "Requested more than max size of {} bytes for a stack",
                    size
                )
            }
            StackError::IoError(ref e) => e.fmt(f),
        }
    }
}

impl Error for StackError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match *self {
            StackError::ExceedsMaximumSize(_) => None,
            StackError::IoError(ref e) => Some(e),
        }
    }
}
