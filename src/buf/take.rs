#[derive(Debug)]
pub struct Take<T> {
    inner: T,
    limit: usize,
}
