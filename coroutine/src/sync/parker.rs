use crate::park::Park;

use super::thread_park::ThreadPark;

#[derive(Debug)]
pub enum Parker {
    Coroutine(Park),
    Thread(ThreadPark),
}
