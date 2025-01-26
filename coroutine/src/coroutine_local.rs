use std::{
    any::TypeId, cell::RefCell, collections::HashMap, hash::BuildHasherDefault, ptr::NonNull,
    sync::Arc,
};

use crate::{Coroutine, id_hasher::IdHasher, join::Join, runtime::get_local_data};

pub(crate) trait Opaque {}

impl<T> Opaque for T {}

pub(crate) type LocalMap = RefCell<HashMap<TypeId, Box<dyn Opaque>, BuildHasherDefault<IdHasher>>>;

thread_local! { static LOCALMAP: LocalMap = RefCell::new(HashMap::default()); }

/// Coroutine local storage
pub struct CoroutineLocal {
    // Current coroutine handle
    coroutine: Coroutine,

    // When panic happens, we need to trigger the join here
    join: Arc<Join>,

    // Real local data hashmap
    local_data: LocalMap,
}

impl CoroutineLocal {
    /// Create new coroutine local storage
    pub fn new(coroutine: Coroutine, join: Arc<Join>) -> Box<CoroutineLocal> {
        Box::new(CoroutineLocal {
            coroutine,
            join,
            local_data: RefCell::new(HashMap::default()),
        })
    }

    // Get the coroutine handle
    pub fn get_coroutine(&self) -> &Coroutine {
        &self.coroutine
    }

    // Get the join handle
    pub fn get_join(&self) -> Arc<Join> {
        self.join.clone()
    }
}

#[inline]
pub fn get_coroutine_local_data() -> Option<NonNull<CoroutineLocal>> {
    let ptr = get_local_data();

    #[allow(clippy::cast_ptr_alignment)]
    NonNull::new(ptr as *mut CoroutineLocal)
}

#[inline]
fn with<F, R>(f: F) -> R
where
    F: FnOnce(&LocalMap) -> R,
{
    match get_coroutine_local_data() {
        Some(c_local) => f(&unsafe { c_local.as_ref() }.local_data),
        None => LOCALMAP.with(|data| f(data)),
    }
}
