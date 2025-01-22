use std::{borrow::Cow, io, sync::Arc, thread::JoinHandle};

use crate::{
    Coroutine,
    done::Done,
    event::{EventSource, EventSubscriber},
};

/// Coroutine Builder, used to configure the coroutine
pub struct CoroutineBuilder {
    /// Name of the Coroutine
    name: Option<Cow<'static, str>>,
    /// The stack size of the coroutine to be spawned
    stack_size: Option<usize>,
    /// Identifier for the coroutine
    id: Option<usize>,
}

impl CoroutineBuilder {
    /// Generates a base configuration for coroutine
    pub fn new() -> Self {
        Self {
            name: None,
            stack_size: None,
            id: None,
        }
    }

    /// Set the name for coroutine
    pub fn name(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.name = Some(name.into());

        self
    }

    /// Set the size of the stack for the coroutine
    pub fn stack_size(mut self, stack_size: usize) -> Self {
        self.stack_size = Some(stack_size);

        self
    }

    /// Set the id of the coroutine
    pub fn id(mut self, id: usize) -> Self {
        self.id = Some(id);

        self
    }

    /// Spawns a new coroutine by taking ownership of the `CoroutineBuilder`, and returns an
    /// `io::Result` to it's `JoinHandle`
    /// Spawned coroutine may outlive the caller. The join handle method can be used to block on
    /// termination of the child thread, including recovering it's panics.
    pub unsafe fn spawn<F, T>(self, f: F) -> io::Result<JoinHandle<T>>
    where
        T: Send + 'static,
        F: FnOnce() -> T + Send + 'static,
    {
        unsafe {
            static DONE: Done = Done;

            let name = self.name;
            let id = self.id;
            let (coroutine, handle) = self.spawn_impl(f)?;
            let scheduler = get_scheduler();
            let stack_size = self.stack_size.unwrap_or_else(|| config().get_stack_size());

            // Create a join resource, shared by waited coroutine and *this* coroutine
            let panic = Arc::new(AtomicOption::none());
            let join = Arc::new(Join::new(panic.clone()));
            let packet = Arc::new(AtomicOption::none());
            let their_join = join.clone();
            let their_packet = packet.clone();

            let subscriber = EventSubscriber {
                resource: &DONE as &dyn EventSource as *const _ as *mut dyn EventSource,
            };

            let closure = move || {
                their_packet.store(f());
                their_join.trigger();

                subscriber
            };

            let mut coroutine = if stack_size == config().get_stack_size() {
                let mut coroutine = scheduler.pool.get();

                coroutine.init_code(closure);

                coroutine
            };

            let handle = Coroutine::new(name.into(), stack_size);

            // Create the local storage
            let local = CoroutineLocal::new(handle.clone(), join.clone());

            // Attach the local storage to the coroutine
            coroutine.set_local_data(Box::into_raw(local) as *mut u8);

            scheduler.schedule(coroutine, id);

            Ok((coroutine, make_join_handle(handle, join, packet, panic)))
        }
    }
}
