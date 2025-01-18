extern crate alloc;

use alloc::{
    alloc::{dealloc, Layout},
    borrow::Borrow,
    boxed::Box,
    string::String,
    vec::Vec,
};
use core::{
    cmp, fmt, hash,
    iter::FromIterator,
    mem::{self, ManuallyDrop},
    ops::{Deref, RangeBounds},
    ptr::{self, NonNull},
    slice,
};

use super::quick::sync::atomic::AtomicMut;
use super::{
    buf::{Buf, IntoIter},
    bytes_mut::BytesMut,
    offset_from,
    quick::sync::atomic::{AtomicPtr, AtomicUsize, Ordering},
};

/// A cheaply cloneable and sliceable chunk of contiguous memory.
pub struct Bytes {
    ptr: *const u8,
    len: usize,
    data: AtomicPtr<()>,
    vtable: &'static Vtable,
}

pub(crate) struct Vtable {
    pub clone: unsafe fn(&AtomicPtr<()>, *const u8, usize) -> Bytes,
    pub to_vec: unsafe fn(&AtomicPtr<()>, *const u8, usize) -> Vec<u8>,
    pub to_mut: unsafe fn(&AtomicPtr<()>, *const u8, usize) -> BytesMut,
    pub is_unique: unsafe fn(&AtomicPtr<()>) -> bool,
    pub drop: unsafe fn(&mut AtomicPtr<()>, *const u8, usize),
}

impl Bytes {
    /// Creates a new empty `Bytes`
    #[inline]
    pub const fn new() -> Self {
        const EMPTY: &[u8] = &[];

        Bytes::from_static(EMPTY)
    }

    /// Creates a new `Bytes` from a static slice
    #[inline]
    pub const fn from_static(bytes: &'static [u8]) -> Self {
        Bytes {
            ptr: bytes.as_ptr(),
            len: bytes.len(),
            data: AtomicPtr::new(ptr::null_mut()),
            vtable: &STATIC_VTABLE,
        }
    }

    /// Creates a new `Bytes` with length zero and the given pointer as the address
    fn new_empty_with_ptr(ptr: *const u8) -> Self {
        debug_assert!(!ptr.is_null());

        let ptr = without_provenance(ptr as usize);

        Bytes {
            ptr,
            len: 0,
            data: AtomicPtr::new(ptr::null_mut()),
            vtable: &STATIC_VTABLE,
        }
    }

    /// Create [Bytes] with a buffer whose lifetime is controlled via an explicit owner
    pub fn from_owner<T>(owner: T) -> Self
    where
        T: AsRef<[u8]> + Send + 'static,
    {
        let owned = Box::into_raw(Box::new(Owned {
            lifetime: OwnedLifetime {
                ref_cnt: AtomicUsize::new(1),
                drop: owned_box_and_drop::<T>,
            },
            owner,
        }));

        let mut ret = Bytes {
            ptr: NonNull::dangling().as_ptr(),
            len: 0,
            data: AtomicPtr::new(owned.cast()),
            vtable: &OWNED_VTABLE,
        };

        let buf = unsafe { &*owned }.owner.as_ref();

        ret.ptr = buf.as_ptr();
        ret.len = buf.len();

        ret
    }

    /// Returns the number of bytes contained in this `Bytes`
    #[inline]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns true if the `Bytes` has a length of 0
    #[inline]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns true if this is the only reference to the data and `Into<BytesMut>` would avoid
    /// cloning the underlying buffer
    pub fn is_unique(&self) -> bool {
        unsafe { (self.vtable.is_unique)(&self.data) }
    }

    /// Creates `Bytes` instance from slice, by copying it
    pub fn copy_from_slice(data: &[u8]) -> Self {
        data.to_vec().into()
    }

    /// Returns a slice of self for the provided range
    pub fn slice(&self, range: impl RangeBounds<usize>) -> Self {
        use core::ops::Bound;

        let len = self.len();
        let begin = match range.start_bound() {
            Bound::Included(&n) => n,
            Bound::Excluded(&n) => n.checked_add(1).expect("Bound out of range"),
            Bound::Unbounded => 0,
        };
        let end = match range.end_bound() {
            Bound::Included(&n) => n.checked_add(1).expect("Bound out of range"),
            Bound::Excluded(&n) => n,
            Bound::Unbounded => len,
        };

        assert!(
            begin <= end,
            "range start must not be greater than end: {:?} <= {:?}",
            begin,
            end
        );

        assert!(
            end <= len,
            "range end out of bounds: {:?} <= {:?}",
            end,
            len
        );

        if end == begin {
            return Bytes::new();
        }

        let mut ret = self.clone();

        ret.len = end - begin;
        ret.ptr = unsafe { ret.ptr.add(begin) };

        ret
    }

    /// Returns a slice of self that is equivalent to the given `subset`
    pub fn slice_ref(&self, subset: &[u8]) -> Self {
        if subset.is_empty() {
            return Bytes::new();
        }

        let bytes_ptr = self.as_ptr() as usize;
        let bytes_len = self.len();

        let subset_ptr = subset.as_ptr() as usize;
        let subset_len = subset.len();

        assert!(
            subset_ptr <= bytes_ptr,
            "subset pointer ({:p}) is smaller than self pointer ({:p})",
            subset.as_ptr(),
            self.as_ptr(),
        );

        assert!(
            subset_ptr + subset_len <= bytes_ptr + bytes_len,
            "subset is out of bounds: self = ({:p}, {}), subset = ({:p}, {})",
            self.as_ptr(),
            bytes_len,
            subset.as_ptr(),
            subset_len,
        );

        let subset_offset = subset_ptr - bytes_ptr;

        self.slice(subset_offset..(subset_offset + subset_len))
    }

    /// Splits the `Bytes` into two at the given index
    #[must_use = "consider Bytes::truncate if you don't need the other half"]
    pub fn split_off(&mut self, at: usize) -> Self {
        if at == self.len() {
            return Bytes::new_empty_with_ptr(self.ptr.wrapping_add(at));
        }

        if at == 0 {
            return mem::replace(self, Bytes::new_empty_with_ptr(self.ptr));
        }

        assert!(
            at <= self.len(),
            "split_off out of bounds: {:?} <= {:?}",
            at,
            self.len()
        );

        let mut ret = self.clone();

        self.len = at;

        unsafe { ret.inc_start(at) };

        ret
    }

    /// Splits the `Bytes` into two at the given index
    #[must_use = "consider Bytes::advance if you don't need the other half"]
    pub fn split_to(&mut self, at: usize) -> Self {
        if at == self.len() {
            let end_ptr = self.ptr.wrapping_add(at);

            return mem::replace(self, Bytes::new_empty_with_ptr(end_ptr));
        }

        if at == 0 {
            return Bytes::new_empty_with_ptr(self.ptr);
        }

        assert!(
            at <= self.len(),
            "split_to out of bounds: {:?} <= {:?}",
            at,
            self.len()
        );

        let mut ret = self.clone();

        unsafe { self.inc_start(at) };

        ret.len = at;

        ret
    }

    /// Shortens the buffer, keeping the first `len` bytes and dropping the remaining
    #[inline]
    pub fn truncate(&mut self, len: usize) {
        if len < self.len {
            if self.vtable as *const Vtable == &PROMOTABLE_EVEN_VTABLE
                || self.vtable as *const Vtable == &PROMOTABLE_ODD_VTABLE
            {
                drop(self.split_off(len));
            } else {
                self.len = len;
            }
        }
    }

    /// Clears the buffer, removing all data
    #[inline]
    pub fn clear(&mut self) {
        self.truncate(0);
    }

    /// Try to convert self ([`Bytes`]) into `ByesMut`
    pub fn try_into_mut(self) -> Result<BytesMut, Bytes> {
        if self.is_unique() {
            Ok(self.into())
        } else {
            Err(self)
        }
    }

    #[inline]
    pub(crate) unsafe fn with_vtable(
        ptr: *const u8,
        len: usize,
        data: AtomicPtr<()>,
        vtable: &'static Vtable,
    ) -> Self {
        Bytes {
            ptr,
            len,
            data,
            vtable,
        }
    }

    /// Returns slice of the `Bytes` with all data
    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr, self.len) }
    }

    #[inline]
    unsafe fn inc_start(&mut self, by: usize) {
        unsafe {
            debug_assert!(self.len >= by, "internal: inc_start out of bounds");

            self.len -= by;
            self.ptr = self.ptr.add(by);
        }
    }
}

unsafe impl Send for Bytes {}
unsafe impl Sync for Bytes {}

impl Drop for Bytes {
    #[inline]
    fn drop(&mut self) {
        unsafe { (self.vtable.drop)(&mut self.data, self.ptr, self.len) }
    }
}

impl Clone for Bytes {
    #[inline]
    fn clone(&self) -> Self {
        unsafe { (self.vtable.clone)(&self.data, self.ptr, self.len) }
    }
}

impl Buf for Bytes {
    #[inline]
    fn remaining(&self) -> usize {
        self.len()
    }

    #[inline]
    fn chunk(&self) -> &[u8] {
        self.as_slice()
    }

    #[inline]
    fn advance(&mut self, cnt: usize) {
        assert!(
            cnt <= self.len(),
            "cannot advance past `remaining`: {:?} <= {:?}",
            cnt,
            self.len()
        );

        unsafe {
            self.inc_start(cnt);
        }
    }

    fn copy_to_bytes(&mut self, len: usize) -> crate::bytes::Bytes {
        self.split_to(len)
    }
}

impl Deref for Bytes {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl AsRef<[u8]> for Bytes {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl hash::Hash for Bytes {
    fn hash<H>(&self, state: &mut H)
    where
        H: core::hash::Hasher,
    {
        self.as_slice().hash(state);
    }
}

impl Borrow<[u8]> for Bytes {
    fn borrow(&self) -> &[u8] {
        self.as_slice()
    }
}

impl IntoIterator for Bytes {
    type Item = u8;
    type IntoIter = IntoIter<Bytes>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self)
    }
}

impl<'a> IntoIterator for &'a Bytes {
    type Item = &'a u8;
    type IntoIter = core::slice::Iter<'a, u8>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl FromIterator<u8> for Bytes {
    fn from_iter<T: IntoIterator<Item = u8>>(iter: T) -> Self {
        Vec::from_iter(iter).into()
    }
}

// ---- impl cmp ----

impl Eq for Bytes {}

impl PartialEq for Bytes {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl PartialOrd for Bytes {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl Ord for Bytes {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl PartialEq<[u8]> for Bytes {
    fn eq(&self, other: &[u8]) -> bool {
        self.as_slice() == other
    }
}

impl PartialOrd<[u8]> for Bytes {
    fn partial_cmp(&self, other: &[u8]) -> Option<cmp::Ordering> {
        self.as_slice().partial_cmp(other)
    }
}

impl PartialEq<Bytes> for [u8] {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Bytes> for [u8] {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self, other)
    }
}

impl PartialEq<str> for Bytes {
    fn eq(&self, other: &str) -> bool {
        self.as_slice() == other.as_bytes()
    }
}

impl PartialOrd<str> for Bytes {
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_bytes())
    }
}

impl PartialEq<Bytes> for str {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Bytes> for str {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self.as_bytes(), other)
    }
}

impl PartialEq<Vec<u8>> for Bytes {
    fn eq(&self, other: &Vec<u8>) -> bool {
        *self == other[..]
    }
}

impl PartialOrd<Vec<u8>> for Bytes {
    fn partial_cmp(&self, other: &Vec<u8>) -> Option<cmp::Ordering> {
        self.as_slice().partial_cmp(&other[..])
    }
}

impl PartialEq<Bytes> for Vec<u8> {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Bytes> for Vec<u8> {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self, other)
    }
}

impl PartialEq<String> for Bytes {
    fn eq(&self, other: &String) -> bool {
        *self == other[..]
    }
}

impl PartialOrd<String> for Bytes {
    fn partial_cmp(&self, other: &String) -> Option<cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_bytes())
    }
}

impl PartialEq<Bytes> for String {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Bytes> for String {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self.as_bytes(), other)
    }
}

impl PartialEq<Bytes> for &[u8] {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Bytes> for &[u8] {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self, other)
    }
}

impl PartialEq<Bytes> for &str {
    fn eq(&self, other: &Bytes) -> bool {
        *other == *self
    }
}

impl PartialOrd<Bytes> for &str {
    fn partial_cmp(&self, other: &Bytes) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self.as_bytes(), other)
    }
}

impl<'a, T: ?Sized> PartialEq<&'a T> for Bytes
where
    Bytes: PartialEq<T>,
{
    fn eq(&self, other: &&'a T) -> bool {
        *self == **other
    }
}

impl<'a, T: ?Sized> PartialOrd<&'a T> for Bytes
where
    Bytes: PartialOrd<T>,
{
    fn partial_cmp(&self, other: &&'a T) -> Option<cmp::Ordering> {
        self.partial_cmp(&**other)
    }
}

// ---- impl From ----
impl Default for Bytes {
    #[inline]
    fn default() -> Bytes {
        Bytes::new()
    }
}

impl From<&'static [u8]> for Bytes {
    fn from(slice: &'static [u8]) -> Bytes {
        Bytes::from_static(slice)
    }
}

impl From<&'static str> for Bytes {
    fn from(slice: &'static str) -> Bytes {
        Bytes::from_static(slice.as_bytes())
    }
}

impl From<Vec<u8>> for Bytes {
    fn from(vec: Vec<u8>) -> Bytes {
        let mut vec = ManuallyDrop::new(vec);
        let ptr = vec.as_mut_ptr();
        let len = vec.len();
        let cap = vec.capacity();

        // Avoid an extra allocation if possible.
        if len == cap {
            let vec = ManuallyDrop::into_inner(vec);
            return Bytes::from(vec.into_boxed_slice());
        }

        let shared = Box::new(Shared {
            buf: ptr,
            cap,
            ref_cnt: AtomicUsize::new(1),
        });

        let shared = Box::into_raw(shared);
        // The pointer should be aligned, so this assert should
        // always succeed.
        debug_assert!(
            0 == (shared as usize & KIND_MASK),
            "internal: Box<Shared> should have an aligned pointer",
        );
        Bytes {
            ptr,
            len,
            data: AtomicPtr::new(shared as _),
            vtable: &SHARED_VTABLE,
        }
    }
}

impl From<Box<[u8]>> for Bytes {
    fn from(slice: Box<[u8]>) -> Bytes {
        // Box<[u8]> doesn't contain a heap allocation for empty slices,
        // so the pointer isn't aligned enough for the KIND_VEC stashing to
        // work.
        if slice.is_empty() {
            return Bytes::new();
        }

        let len = slice.len();
        let ptr = Box::into_raw(slice) as *mut u8;

        if ptr as usize & 0x1 == 0 {
            let data = ptr_map(ptr, |addr| addr | KIND_VEC);
            Bytes {
                ptr,
                len,
                data: AtomicPtr::new(data.cast()),
                vtable: &PROMOTABLE_EVEN_VTABLE,
            }
        } else {
            Bytes {
                ptr,
                len,
                data: AtomicPtr::new(ptr.cast()),
                vtable: &PROMOTABLE_ODD_VTABLE,
            }
        }
    }
}

impl From<Bytes> for BytesMut {
    fn from(bytes: Bytes) -> Self {
        let bytes = ManuallyDrop::new(bytes);
        unsafe { (bytes.vtable.to_mut)(&bytes.data, bytes.ptr, bytes.len) }
    }
}

impl From<String> for Bytes {
    fn from(s: String) -> Bytes {
        Bytes::from(s.into_bytes())
    }
}

impl From<Bytes> for Vec<u8> {
    fn from(bytes: Bytes) -> Vec<u8> {
        let bytes = ManuallyDrop::new(bytes);
        unsafe { (bytes.vtable.to_vec)(&bytes.data, bytes.ptr, bytes.len) }
    }
}

// ---- impl Vtable ----
impl fmt::Debug for Vtable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("vtable")
            .field("clone", &(self.clone as *const ()))
            .field("drop", &(self.drop as *const ()))
            .finish()
    }
}

// ---- impl StaticVtable ----
const STATIC_VTABLE: Vtable = Vtable {
    clone: static_clone,
    to_vec: static_to_vec,
    to_mut: static_to_mut,
    is_unique: static_is_unique,
    drop: static_drop,
};

unsafe fn static_clone(_: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Bytes {
    unsafe {
        let slice = slice::from_raw_parts(ptr, len);

        Bytes::from_static(slice)
    }
}

unsafe fn static_to_vec(_: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8> {
    unsafe {
        let slice = slice::from_raw_parts(ptr, len);

        slice.to_vec()
    }
}

unsafe fn static_to_mut(_: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BytesMut {
    unsafe {
        let slice = slice::from_raw_parts(ptr, len);

        BytesMut::from(slice)
    }
}

unsafe fn static_is_unique(_: &AtomicPtr<()>) -> bool {
    false
}

unsafe fn static_drop(_: &mut AtomicPtr<()>, _: *const u8, _: usize) {}

// ---- impl OwnedVtable ----
#[repr(C)]
struct OwnedLifetime {
    ref_cnt: AtomicUsize,
    drop: unsafe fn(*mut ()),
}

#[repr(C)]
struct Owned<T> {
    lifetime: OwnedLifetime,
    owner: T,
}

unsafe fn owned_box_and_drop<T>(ptr: *mut ()) {
    unsafe {
        let b: Box<Owned<T>> = Box::from_raw(ptr as _);
        drop(b)
    }
}

unsafe fn owned_clone(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Bytes {
    unsafe {
        let owned = data.load(Ordering::Relaxed);
        let ref_cnt = &(*owned.cast::<OwnedLifetime>()).ref_cnt;
        let old_cnt = ref_cnt.fetch_add(1, Ordering::Relaxed);

        if old_cnt > usize::MAX >> 1 {
            super::abort()
        }

        Bytes {
            ptr,
            len,
            data: AtomicPtr::new(owned as _),
            vtable: &OWNED_VTABLE,
        }
    }
}

unsafe fn owned_to_vec(_data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8> {
    unsafe {
        let slice = slice::from_raw_parts(ptr, len);

        slice.to_vec()
    }
}

unsafe fn owned_to_mut(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BytesMut {
    unsafe {
        let bytes_mut = BytesMut::from_vec(owned_to_vec(data, ptr, len));

        owned_drop_impl(data.load(Ordering::Relaxed));

        bytes_mut
    }
}

unsafe fn owned_is_unique(_data: &AtomicPtr<()>) -> bool {
    false
}

unsafe fn owned_drop_impl(owned: *mut ()) {
    unsafe {
        let lifetime = owned.cast::<OwnedLifetime>();
        let ref_cnt = &(*lifetime).ref_cnt;
        let old_cnt = ref_cnt.fetch_sub(1, Ordering::Relaxed);

        if old_cnt != 1 {
            return;
        }

        ref_cnt.load(Ordering::Relaxed);

        let drop_fn = &(*lifetime).drop;

        drop_fn(owned)
    }
}

unsafe fn owned_drop(data: &mut AtomicPtr<()>, _ptr: *const u8, _len: usize) {
    unsafe {
        let owned = data.load(Ordering::Relaxed);

        owned_drop_impl(owned);
    }
}

static OWNED_VTABLE: Vtable = Vtable {
    clone: owned_clone,
    to_vec: owned_to_vec,
    to_mut: owned_to_mut,
    is_unique: owned_is_unique,
    drop: owned_drop,
};

// ---- impl PromotableVtable ----
static PROMOTABLE_EVEN_VTABLE: Vtable = Vtable {
    clone: promotable_even_clone,
    to_vec: promotable_even_to_vec,
    to_mut: promotable_even_to_mut,
    is_unique: promotable_is_unique,
    drop: promotable_even_drop,
};

unsafe fn promotable_even_clone(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Bytes {
    unsafe {
        let shared = data.load(Ordering::Acquire);
        let kind = shared as usize & KIND_MASK;

        if kind == KIND_ARC {
            shallow_clone_arc(shared.cast(), ptr, len)
        } else {
            debug_assert_eq!(kind, KIND_VEC);

            let buf = ptr_map(shared.cast(), |addr| addr & !KIND_MASK);

            shallow_clone_vec(data, shared, buf, ptr, len)
        }
    }
}

unsafe fn promotable_to_vec(
    data: &AtomicPtr<()>,
    ptr: *const u8,
    len: usize,
    f: fn(*mut ()) -> *mut u8,
) -> Vec<u8> {
    unsafe {
        let shared = data.load(Ordering::Acquire);
        let kind = shared as usize & KIND_MASK;

        if kind == KIND_ARC {
            shared_to_vec_impl(shared.cast(), ptr, len)
        } else {
            // If Bytes hold a Vec, then offset must be 0
            debug_assert_eq!(kind, KIND_ARC);

            let buf = f(shared);
            let cap = offset_from(ptr, buf) + len;

            // Copy back the buffer
            ptr::copy(ptr, buf, len);

            Vec::from_raw_parts(buf, len, cap)
        }
    }
}

unsafe fn promotable_to_mut(
    data: &AtomicPtr<()>,
    ptr: *const u8,
    len: usize,
    f: fn(*mut ()) -> *mut u8,
) -> BytesMut {
    unsafe {
        let shared = data.load(Ordering::Acquire);
        let kind = shared as usize * KIND_MASK;

        if kind == KIND_ARC {
            shared_to_mut_impl(shared.cast(), ptr, len)
        } else {
            debug_assert_eq!(kind, KIND_VEC);

            let buf = f(shared);
            let offset = offset_from(ptr, buf);
            let cap = offset + len;
            let v = Vec::from_raw_parts(buf, cap, cap);
            let mut b = BytesMut::from_vec(v);

            b.advance_unchecked(offset);

            b
        }
    }
}

unsafe fn promotable_even_to_vec(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8> {
    unsafe {
        promotable_to_vec(data, ptr, len, |shared| {
            ptr_map(shared.cast(), |addr| addr & !KIND_MASK)
        })
    }
}

unsafe fn promotable_even_to_mut(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BytesMut {
    unsafe {
        promotable_to_mut(data, ptr, len, |shared| {
            ptr_map(shared.cast(), |addr| addr & !KIND_MASK)
        })
    }
}

unsafe fn promotable_even_drop(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) {
    unsafe {
        data.with_mut(|shared| {
            let shared = *shared;
            let kind = shared as usize & KIND_MASK;

            if kind == KIND_ARC {
                release_shared(shared.cast());
            } else {
                debug_assert_eq!(kind, KIND_VEC);
                let buf = ptr_map(shared.cast(), |addr| addr & !KIND_MASK);
                free_boxed_slice(buf, ptr, len);
            }
        });
    }
}

// ---- impl PromotableOddVtable ----
static PROMOTABLE_ODD_VTABLE: Vtable = Vtable {
    clone: promotable_odd_clone,
    to_vec: promotable_odd_to_vec,
    to_mut: promotable_odd_to_mut,
    is_unique: promotable_is_unique,
    drop: promotable_odd_drop,
};

unsafe fn promotable_odd_clone(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Bytes {
    unsafe {
        let shared = data.load(Ordering::Acquire);
        let kind = shared as usize & KIND_MASK;

        if kind == KIND_ARC {
            shallow_clone_arc(shared as _, ptr, len)
        } else {
            debug_assert_eq!(kind, KIND_VEC);

            shallow_clone_vec(data, shared, shared.cast(), ptr, len)
        }
    }
}

unsafe fn promotable_odd_to_vec(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8> {
    unsafe { promotable_to_vec(data, ptr, len, |shared| shared.cast()) }
}

unsafe fn promotable_odd_to_mut(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BytesMut {
    unsafe { promotable_to_mut(data, ptr, len, |shared| shared.cast()) }
}

unsafe fn promotable_odd_drop(data: &mut AtomicPtr<()>, ptr: *const u8, len: usize) {
    unsafe {
        data.with_mut(|shared| {
            let shared = *shared;
            let kind = shared as usize & KIND_MASK;

            if kind == KIND_ARC {
                release_shared(shared.cast())
            } else {
                debug_assert_eq!(kind, KIND_VEC);

                free_boxed_slice(shared.cast(), ptr, len);
            }
        });
    }
}

unsafe fn promotable_is_unique(data: &AtomicPtr<()>) -> bool {
    unsafe {
        let shared = data.load(Ordering::Acquire);
        let kind = shared as usize & KIND_MASK;

        if kind == KIND_ARC {
            let ref_cnt = (*shared.cast::<Shared>()).ref_cnt.load(Ordering::Relaxed);

            ref_cnt == 1
        } else {
            true
        }
    }
}

unsafe fn free_boxed_slice(buf: *mut u8, offset: *const u8, len: usize) {
    unsafe {
        let cap = offset_from(offset, buf) + len;

        dealloc(buf, Layout::from_size_align(cap, 1).unwrap());
    }
}

// ---- impl SharedVtable ----
struct Shared {
    // Holds arguments to dealloc on Drop, but otherwise doesn't use them
    buf: *mut u8,
    cap: usize,
    ref_cnt: AtomicUsize,
}

impl Drop for Shared {
    fn drop(&mut self) {
        unsafe {
            dealloc(self.buf, Layout::from_size_align(self.cap, 1).unwrap());
        }
    }
}

// Assert that the alignment of `Shared` is divisible by 2
// This is a necessary invariant since we depend on allocating `Shared`
// a shared object to implicitly carry the `KIND_ARC` flag in its pointer.
// This flag is set when the LSB is 0.
const _: [(); 0 - mem::align_of::<Shared>() % 2] = [];

static SHARED_VTABLE: Vtable = Vtable {
    clone: shared_clone,
    to_vec: shared_to_vec,
    to_mut: shared_to_mut,
    is_unique: shared_is_unique,
    drop: shared_drop,
};

const KIND_ARC: usize = 0b0;
const KIND_VEC: usize = 0b1;
const KIND_MASK: usize = 0b1;

unsafe fn shared_clone(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Bytes {
    unsafe {
        let shared = data.load(Ordering::Relaxed);

        shallow_clone_arc(shared as _, ptr, len)
    }
}

unsafe fn shared_to_vec_impl(shared: *mut Shared, ptr: *const u8, len: usize) -> Vec<u8> {
    unsafe {
        // Check if the ref_cnt is 1 (unique).
        //
        // If it is unique then it is set to 0 with AcqRel fence for the same
        // reason in release_shared
        //
        // Otherwise, we take the other branch and call release_shared.
        if (*shared)
            .ref_cnt
            .compare_exchange(1, 0, Ordering::AcqRel, Ordering::Relaxed)
            .is_ok()
        {
            // Deallocate the shared instance without calling its destructor.
            let shared = *Box::from_raw(shared);
            let shared = ManuallyDrop::new(shared);
            let buf = shared.buf;
            let cap = shared.cap;

            // Copy back buffer
            ptr::copy(ptr, buf, len);

            Vec::from_raw_parts(buf, len, cap)
        } else {
            let v = slice::from_raw_parts(ptr, len).to_vec();

            release_shared(shared);

            v
        }
    }
}

unsafe fn shared_to_vec(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8> {
    unsafe { shared_to_vec_impl(data.load(Ordering::Relaxed).cast(), ptr, len) }
}

unsafe fn shared_to_mut_impl(shared: *mut Shared, ptr: *const u8, len: usize) -> BytesMut {
    unsafe {
        if (*shared).ref_cnt.load(Ordering::Acquire) == 1 {
            // Deallocate the `Shared` instance without calling its destructor
            let shared = *Box::from_raw(shared);
            let shared = ManuallyDrop::new(shared);
            let buf = shared.buf;
            let cap = shared.cap;

            // Rebuild the Vec
            let offset = offset_from(ptr, buf);
            let v = Vec::from_raw_parts(buf, len + offset, cap);

            let mut b = BytesMut::from_vec(v);

            b.advance_unchecked(offset);

            b
        } else {
            // Copy the data from Shared in a new Vec, then release it
            let v = slice::from_raw_parts(ptr, len).to_vec();

            release_shared(shared);

            BytesMut::from_vec(v)
        }
    }
}

unsafe fn shared_to_mut(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BytesMut {
    unsafe { shared_to_mut_impl(data.load(Ordering::Relaxed).cast(), ptr, len) }
}

pub(crate) unsafe fn shared_is_unique(data: &AtomicPtr<()>) -> bool {
    unsafe {
        let shared = data.load(Ordering::Acquire);
        let ref_cnt = (*shared.cast::<Shared>()).ref_cnt.load(Ordering::Relaxed);

        ref_cnt == 1
    }
}

unsafe fn shared_drop(data: &mut AtomicPtr<()>, _ptr: *const u8, _len: usize) {
    unsafe {
        data.with_mut(|shared| {
            release_shared(shared.cast());
        });
    }
}

unsafe fn shallow_clone_arc(shared: *mut Shared, ptr: *const u8, len: usize) -> Bytes {
    unsafe {
        let old_size = (*shared).ref_cnt.load(Ordering::Relaxed);

        if old_size > usize::MAX >> 1 {
            super::abort();
        }

        Bytes {
            ptr,
            len,
            data: AtomicPtr::new(shared as _),
            vtable: &SHARED_VTABLE,
        }
    }
}

#[cold]
unsafe fn shallow_clone_vec(
    atom: &AtomicPtr<()>,
    ptr: *const (),
    buf: *mut u8,
    offset: *const u8,
    len: usize,
) -> Bytes {
    unsafe {
        let shared = Box::new(Shared {
            buf,
            cap: offset_from(offset, buf) + len,
            ref_cnt: AtomicUsize::new(2),
        });

        let shared = Box::into_raw(shared);

        debug_assert!(
            0 == (shared as usize & KIND_MASK),
            "internal: Box<Shared> should have an aligned pointer"
        );

        match atom.compare_exchange(ptr as _, shared as _, Ordering::AcqRel, Ordering::Acquire) {
            Ok(actual) => {
                debug_assert!(actual as usize == ptr as usize);

                Bytes {
                    ptr: offset,
                    len,
                    data: AtomicPtr::new(shared as _),
                    vtable: &SHARED_VTABLE,
                }
            }
            Err(actual) => {
                let shared = Box::from_raw(shared);

                mem::forget(*shared);

                shallow_clone_arc(actual as _, offset, len)
            }
        }
    }
}

unsafe fn release_shared(ptr: *mut Shared) {
    unsafe {
        // `Shared` storage... follow the drop steps from Arc.
        if (*ptr).ref_cnt.fetch_sub(1, Ordering::Release) != 1 {
            return;
        }

        (*ptr).ref_cnt.load(Ordering::Acquire);

        // Drop the data
        drop(Box::from_raw(ptr));
    }
}

// ---- ptr_map: strict provenance compatible ----
fn ptr_map<F>(ptr: *mut u8, f: F) -> *mut u8
where
    F: FnOnce(usize) -> usize,
{
    let old_address = ptr as usize;
    let new_address = f(old_address);
    let diff = new_address.wrapping_sub(old_address);
    ptr.wrapping_add(diff)
}

// ---- strict provenance ----
fn without_provenance(ptr: usize) -> *const u8 {
    core::ptr::null::<u8>().wrapping_add(ptr)
}
