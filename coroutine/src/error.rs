#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Error {
    /// Done panic
    Done,
    /// Cancel panic
    Cancel,
    /// Type mismatch panic
    TypeErr,
    /// Stack overflow panic
    StackErr,
    /// Wrong context panic
    ContextErr,
}
