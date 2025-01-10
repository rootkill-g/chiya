mod buf_impl;
pub use self::buf_impl::Buf;

mod iter;
pub use self::iter::IntoIter;

/// Module for trait BufMut
pub mod buf_mut;
pub use self::buf_mut::BufMut;

/// Module for chaining two slices
pub mod chain;
pub use self::chain::Chain;

/// Module for checking the limts of bounds for the bytes
pub mod limit;
pub use self::limit::Limit;

/// Module for UninitSlice type
pub mod uninit_slice;
pub use self::uninit_slice::UninitSlice;

/// Module for Writer
pub mod writer;
pub use self::writer::Writer;

/// Module for Take
pub mod take;
pub use self::take::Take;

/// Module for Reader
pub mod reader;
pub use self::reader::Reader;
