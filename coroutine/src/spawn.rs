use std::thread::JoinHandle;

use crate::builder::CoroutineBuilder;

pub unsafe fn spawn<F, T>(f: F) -> JoinHandle<()>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    spawn_coroutine_builder(f, CoroutineBuilder::new())
}
