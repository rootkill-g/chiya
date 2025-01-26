use crate::{is_coroutine, park::Park};

use super::{parker::Parker, thread_park::ThreadPark};

#[derive(Debug)]
pub struct Blocker {
    parker: Parker,
}

impl Blocker {
    /// Create a new Blocker
    pub fn new(ignore_cancel: bool) -> Blocker {
        let parker = if is_coroutine() {
            let park = Park::new();

            park.ignore_cancel(ignore_cancel);

            Parker::Coroutine(park)
        } else {
            let thread_park = ThreadPark::new();

            Parker::Thread(thread_park)
        };

        Blocker { parker }
    }
}
