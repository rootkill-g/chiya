use super::Buf;
use crate::buf::{IntoIter, UninitSlice};
use crate::bytes::Vtable;
#[allow(unused)]
use crate::quick::sync::atomic::AtomicMut;
use crate::quick::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use crate::BufMut;
use crate::{offset_from, Bytes};
use alloc::{
    borrow::{Borrow, BorrowMut},
    boxed::Box,
    string::String,
    vec,
    vec::Vec,
};
use core::{
    cmp, fmt, hash, isize,
    iter::FromIterator,
    mem::{self, ManuallyDrop, MaybeUninit},
    ops::{Deref, DerefMut},
    ptr::{self, NonNull},
    slice, usize,
};

pub struct BytesMut {
    ptr: NonNull<u8>,
    len: usize,
    cap: usize,
    data: *mut Shared,
}

struct Shared {
    vec: Vec<u8>,
    original_capacity_repr: usize,
    ref_count: AtomicUsize,
}

const _: [(); 0 - mem::align_of::<Shared>() % 2] = [];

const KIND_ARC: usize = 0b0;
const KIND_VEC: usize = 0b1;
const KIND_MASK: usize = 0b1;

const MAX_ORIGINAL_CAPACITY_WIDTH: usize = 17;
const MIN_ORIGINAL_CAPACITY_WIDTH: usize = 10;

const ORIGINAL_CAPACITY_MASK: usize = 0b11100;
const ORIGINAL_CAPACITY_OFFSET: usize = 2;

const VEC_POS_OFFSET: usize = 5;
const MAX_VEC_POS: usize = usize::MAX >> VEC_POS_OFFSET;
const NOT_VEC_POS_MASK: usize = 0b11111;

#[cfg(target_pointer_width = "64")]
const PTR_WIDTH: usize = 64;
#[cfg(target_pointer_width = "32")]
const PTR_WIDTH: usize = 32;

// ---- impl BytesMut ----
impl BytesMut {
    #[inline]
    pub fn new() -> BytesMut {
        BytesMut::with_capacity(0)
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> BytesMut {
        BytesMut::from_vec(Vec::with_capacity(capacity))
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.len
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[inline]
    pub fn capacity(&self) -> usize {
        self.cap
    }

    #[inline]
    pub fn freeze(self) -> Bytes {
        let bytes = ManuallyDrop::new(self);
        if bytes.kind() == KIND_VEC {
            unsafe {
                let offset = bytes.get_vec_pos();
                let vec = rebuild_vec(bytes.ptr.as_ptr(), bytes.len, bytes.cap, offset);
                let mut b: Bytes = vec.into();

                b.advance(offset);

                b
            }
        } else {
            debug_assert_eq!(bytes.kind(), KIND_ARC);

            let ptr = bytes.ptr.as_ptr();
            let len = bytes.len;
            let data = AtomicPtr::new(bytes.data.cast());

            unsafe { Bytes::with_vtable(ptr, len, data, &SHARED_VTABLE) }
        }
    }

    pub fn zeroed(len: usize) -> BytesMut {
        BytesMut::from_vec(vec![0; len])
    }

    #[must_use = "consider BytesMut::truncate if you don't need the other half"]
    pub fn split_off(&mut self, at: usize) -> BytesMut {
        assert!(
            at <= self.capacity(),
            "split_off out of bounds: {:?} <= {:?}",
            at,
            self.capacity()
        );

        unsafe {
            let mut other = self.shallow_clone();

            // SAFETY: Since we already checked that `at <= self.capacity()`, so we can advace
            // without further checking
            other.advance_unchecked(at);

            self.cap = at;
            self.len = cmp::min(self.len, at);

            other
        }
    }

    #[must_use = "consider BytesMut::clear if you don't need the other half"]
    pub fn split(&mut self) -> BytesMut {
        let len = self.len();

        self.split_to(len)
    }

    #[must_use = "consider BytesMut::advance if you don't need the other half"]
    pub fn split_to(&mut self, at: usize) -> BytesMut {
        assert!(
            at <= self.len(),
            "split_to out of bounds: {:?} <= {:?}",
            at,
            self.len()
        );

        unsafe {
            let mut other = self.shallow_clone();

            // SAFETY: since we have already checked that `at <= self.len()`
            // and we know that `self.len()` <= `self.capacity()`
            self.advance_unchecked(at);

            other.cap = at;
            other.len = at;

            other
        }
    }

    pub fn truncate(&mut self, len: usize) {
        if len <= self.len() {
            // SAFETY: Shrinking the buffer cannot expose the uninitialized bytes
            unsafe { self.set_len(len) };
        }
    }

    pub fn clear(&mut self) {
        // SAFETY: Setting the length to zero cannot expose uninitialized bytes
        unsafe { self.set_len(0) };
    }

    pub fn resize(&mut self, new_len: usize, value: u8) {
        let additional = if let Some(additional) = new_len.checked_sub(self.len()) {
            additional
        } else {
            self.truncate(new_len);

            return;
        };

        if additional == 0 {
            return;
        }

        self.reserve(additional);

        let dst = self.spare_capacity_mut().as_mut_ptr();

        // SAFETY: `spare_capacity_mut` returns a valid, properly aligned pointer
        // and we've reserved enough space to write `additional` bytes
        unsafe { ptr::write_bytes(dst, value, additional) };

        // SAFETY: There are at least `new_len` initialized bytes in the buffer
        // so no uninitialized bytes are being exposed
        unsafe { self.set_len(new_len) };
    }

    #[inline]
    pub unsafe fn set_len(&mut self, len: usize) {
        debug_assert!(len <= self.cap, "set_len out of bounds");

        self.len = len
    }

    #[inline]
    pub fn reserve(&mut self, additional: usize) {
        let len = self.len();
        let rem = self.capacity() - len;

        if additional <= rem {
            // NOTE: It can already store at least `additional` more bytes,
            // so there is no further work required to be done
            return;
        }

        // Infallible
        let _ = self.reserve_inner(additional, true);
    }

    #[inline]
    pub fn reserve_inner(&mut self, additional: usize, allocate: bool) -> bool {
        let len = self.len();
        let kind = self.kind();

        if kind == KIND_VEC {
            // NOTE: If there's enough free space before the start of the buffer,
            // then just copy the data backwards and reuse the already-allocated space.
            // Otherwise, since backed by a vector, use `Vec::reserve`
            // We need to make sure that this optimization does not kill the amortized
            // runtimes of BytesMut's operations
            unsafe {
                let offset = self.get_vec_pos();

                if self.capacity() - self.len() + offset >= additional && offset >= self.len() {
                    let base_ptr = self.ptr.as_ptr().sub(offset);

                    ptr::copy_nonoverlapping(self.ptr.as_ptr(), base_ptr, self.len);

                    self.ptr = vptr(base_ptr);
                    self.set_vec_pos(0);

                    self.cap += offset;
                } else {
                    if !allocate {
                        return false;
                    }

                    let mut v = ManuallyDrop::new(rebuild_vec(
                        self.ptr.as_ptr(),
                        self.len,
                        self.cap,
                        offset,
                    ));

                    v.reserve(additional);

                    self.ptr = vptr(v.as_mut_ptr().add(offset));
                    self.cap = v.capacity() - offset;

                    debug_assert_eq!(self.len, v.len() - offset);
                }

                return true;
            }
        }

        debug_assert_eq!(kind, KIND_ARC);

        let shared: *mut Shared = self.data;

        let mut new_cap = match len.checked_add(additional) {
            Some(new_cap) => new_cap,
            None if !allocate => return false,
            None => panic!("Overflow"),
        };

        unsafe {
            if (*shared).is_unique() {
                let v = &mut (*shared).vec;
                let v_capacity = v.capacity();
                let ptr = v.as_mut_ptr();
                let offset = offset_from(self.ptr.as_ptr(), ptr);

                if v_capacity >= new_cap + offset {
                    self.cap = new_cap;
                } else if v_capacity >= new_cap && offset >= len {
                    ptr::copy_nonoverlapping(self.ptr.as_ptr(), ptr, len);

                    self.ptr = vptr(ptr);
                    self.cap = v.capacity();
                } else {
                    if !allocate {
                        return false;
                    }

                    let offset = (self.ptr.as_ptr() as usize) - (v.as_ptr() as usize);

                    new_cap = new_cap.checked_add(offset).expect("Overflow");

                    let double = v.capacity().checked_shl(1).unwrap_or(new_cap);

                    new_cap = cmp::max(double, new_cap);

                    debug_assert!(offset + len <= v.capacity());

                    v.set_len(offset + len);
                    v.reserve(new_cap - v.len());

                    self.ptr = vptr(v.as_mut_ptr().add(offset));
                    self.cap = v.capacity() - offset;
                }

                return true;
            }
        }

        if !allocate {
            return false;
        }

        let original_capacity_repr = unsafe { (*shared).original_capacity_repr };
        let original_capacity = original_capacity_from_repr(original_capacity_repr);

        new_cap = cmp::max(new_cap, original_capacity);

        let mut v = ManuallyDrop::new(Vec::with_capacity(new_cap));

        v.extend_from_slice(self.as_ref());

        unsafe { release_shared(shared) };

        let data = (original_capacity_repr << ORIGINAL_CAPACITY_OFFSET) | KIND_VEC;

        self.data = invalid_ptr(data);
        self.ptr = vptr(v.as_mut_ptr());
        self.cap = v.capacity();

        debug_assert_eq!(self.len, v.len());

        return true;
    }

    #[inline]
    #[must_use = "consider BytesMut::reserve if you need an infallible reservation"]
    pub fn try_reclaim(&mut self, additional: usize) -> bool {
        let len = self.len();
        let rem = self.capacity() - len;

        if additional <= rem {
            return true;
        }

        self.reserve_inner(additional, true)
    }

    #[inline]
    pub fn extend_from_slice(&mut self, extend: &[u8]) {
        let cnt = extend.len();

        self.reserve(cnt);

        unsafe {
            let dst = self.spare_capacity_mut();

            debug_assert!(dst.len() >= cnt);

            ptr::copy_nonoverlapping(extend.as_ptr(), dst.as_mut_ptr().cast(), cnt);
        }

        unsafe { self.advance_mut(cnt) };
    }

    pub fn unsplit(&mut self, other: BytesMut) {
        if self.is_empty() {
            *self = other;

            return;
        }

        if let Err(other) = self.try_unsplit(other) {
            self.extend_from_slice(other.as_ref());
        }
    }

    #[inline]
    pub(crate) fn from_vec(vec: Vec<u8>) -> BytesMut {
        let mut vec = ManuallyDrop::new(vec);
        let ptr = vptr(vec.as_mut_ptr());
        let len = vec.len();
        let cap = vec.capacity();

        let original_capacity_repr = original_capacity_to_repr(cap);
        let data = (original_capacity_repr << ORIGINAL_CAPACITY_OFFSET) | KIND_VEC;

        BytesMut {
            ptr,
            len,
            cap,
            data: invalid_ptr(data),
        }
    }

    #[inline]
    pub fn as_slice(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.ptr.as_ptr(), self.len) }
    }

    #[inline]
    fn as_slice_mut(&mut self) -> &mut [u8] {
        unsafe { slice::from_raw_parts_mut(self.ptr.as_ptr(), self.len) }
    }

    // NOTE:
    // Advance the buffer without checking bounds.
    //
    // SAFETY:
    // The caller must ensure that `count` <= `self.cap`
    pub(crate) unsafe fn advance_unchecked(&mut self, count: usize) {
        if count == 0 {
            return;
        }

        debug_assert!(count <= self.cap, "internal: set_start out of bounds");

        let kind = self.kind();

        if kind == KIND_VEC {
            let pos = self.get_vec_pos() + count;

            if pos <= MAX_VEC_POS {
                self.set_vec_pos(pos);
            } else {
                self.promote_to_shared(/* ref_count = */ 1);
            }
        }

        self.ptr = vptr(self.ptr.as_ptr().add(count));
        self.len = self.len.checked_sub(count).unwrap_or(0);
        self.cap -= count;
    }

    fn try_unsplit(&mut self, other: BytesMut) -> Result<(), BytesMut> {
        if other.capacity() == 0 {
            return Ok(());
        }

        let ptr = unsafe { self.ptr.as_ptr().add(self.len) };

        if ptr == other.ptr.as_ptr()
            && self.kind() == KIND_ARC
            && other.kind() == KIND_ARC
            && self.data == other.data
        {
            self.len += other.len;
            self.cap += other.cap;

            Ok(())
        } else {
            Err(other)
        }
    }

    #[inline]
    fn kind(&self) -> usize {
        self.data as usize & KIND_MASK
    }

    unsafe fn promote_to_shared(&mut self, ref_cnt: usize) {
        debug_assert_eq!(self.kind(), KIND_VEC);
        debug_assert!(ref_cnt == 1 || ref_cnt == 2);

        let original_capacity_repr =
            (self.data as usize & ORIGINAL_CAPACITY_MASK) >> ORIGINAL_CAPACITY_OFFSET;
        let offset = (self.data as usize) >> VEC_POS_OFFSET;
        let shared = Box::new(Shared {
            vec: rebuild_vec(self.ptr.as_ptr(), self.len, self.cap, offset),
            original_capacity_repr,
            ref_count: AtomicUsize::new(ref_cnt),
        });

        let shared = Box::into_raw(shared);

        debug_assert_eq!(shared as usize & KIND_MASK, KIND_ARC);

        self.data = shared;
    }

    #[inline]
    unsafe fn shallow_clone(&mut self) -> BytesMut {
        if self.kind() == KIND_ARC {
            increment_shared(self.data);

            ptr::read(self)
        } else {
            self.promote_to_shared(/* ref_count = */ 2);

            ptr::read(self)
        }
    }

    #[inline]
    unsafe fn get_vec_pos(&self) -> usize {
        debug_assert_eq!(self.kind(), KIND_VEC);

        self.data as usize >> VEC_POS_OFFSET
    }

    #[inline]
    unsafe fn set_vec_pos(&mut self, pos: usize) {
        debug_assert_eq!(self.kind(), KIND_VEC);
        debug_assert!(pos <= MAX_VEC_POS);

        self.data = invalid_ptr((pos << VEC_POS_OFFSET) | (self.data as usize & NOT_VEC_POS_MASK))
    }

    #[inline]
    pub fn spare_capacity_mut(&mut self) -> &mut [MaybeUninit<u8>] {
        unsafe {
            let ptr = self.ptr.as_ptr().add(self.len);
            let len = self.cap - self.len;

            slice::from_raw_parts_mut(ptr.cast(), len)
        }
    }
}

impl Drop for BytesMut {
    fn drop(&mut self) {
        let kind = self.kind();

        if kind == KIND_VEC {
            unsafe {
                let offset = self.get_vec_pos();

                // Vector storage, free the vector
                let _ = rebuild_vec(self.ptr.as_ptr(), self.len, self.cap, offset);
            }
        } else if kind == KIND_ARC {
            unsafe { release_shared(self.data) }
        }
    }
}

impl Buf for BytesMut {
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
            cnt <= self.remaining(),
            "cannot advance past `remaining`: {:?} <= {:?}",
            cnt,
            self.remaining()
        );

        unsafe {
            // SAFETY: We've checked that `cnt` <= `self.remaining()`
            // and we know that `self.remaining()` <= `self.cap`
            self.advance_unchecked(cnt)
        };
    }

    fn copy_to_bytes(&mut self, len: usize) -> crate::bytes::Bytes {
        self.split_to(len).freeze()
    }
}

unsafe impl BufMut for BytesMut {
    #[inline]
    fn remaining_mut(&self) -> usize {
        usize::MAX - self.len()
    }

    #[inline]
    unsafe fn advance_mut(&mut self, cnt: usize) {
        let rem = self.cap - self.len();

        if cnt > rem {
            super::panic_advance(cnt, rem);
        }

        // NOTE:
        // Addition won't overflow since it is at most `self.cap`
        self.len = self.len() + cnt;
    }

    #[inline]
    fn chunk_mut(&mut self) -> &mut UninitSlice {
        if self.capacity() == self.len() {
            self.reserve(64);
        }

        self.spare_capacity_mut().into()
    }

    // NOTE:
    // Specialize these methods so they can skip checking `remaining_mut` and `advance_mut`
    fn put<T>(&mut self, mut src: T)
    where
        T: Buf,
        Self: Sized,
    {
        while src.has_remaining() {
            let s = src.chunk();
            let l = s.len();

            self.extend_from_slice(s);

            src.advance(l);
        }
    }

    fn put_slice(&mut self, src: &[u8]) {
        self.extend_from_slice(src);
    }

    fn put_bytes(&mut self, val: u8, cnt: usize) {
        self.reserve(cnt);

        unsafe {
            let dst = self.spare_capacity_mut();

            // Reserved above
            debug_assert!(dst.len() >= cnt);

            ptr::write_bytes(dst.as_mut_ptr(), val, cnt);

            self.advance_mut(cnt);
        }
    }
}

impl AsRef<[u8]> for BytesMut {
    #[inline]
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

impl Deref for BytesMut {
    type Target = [u8];

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_ref()
    }
}

impl AsMut<[u8]> for BytesMut {
    #[inline]
    fn as_mut(&mut self) -> &mut [u8] {
        self.as_slice_mut()
    }
}

impl DerefMut for BytesMut {
    #[inline]
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut()
    }
}

impl<'a> From<&'a [u8]> for BytesMut {
    fn from(value: &'a [u8]) -> Self {
        BytesMut::from_vec(value.to_vec())
    }
}

impl<'a> From<&'a str> for BytesMut {
    fn from(value: &'a str) -> Self {
        BytesMut::from(value.as_bytes())
    }
}

impl From<BytesMut> for Bytes {
    fn from(value: BytesMut) -> Self {
        value.freeze()
    }
}

impl Eq for BytesMut {}

impl PartialEq for BytesMut {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl PartialOrd for BytesMut {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl Ord for BytesMut {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        self.as_slice().cmp(other.as_slice())
    }
}

impl Default for BytesMut {
    #[inline]
    fn default() -> Self {
        BytesMut::new()
    }
}

impl hash::Hash for BytesMut {
    fn hash<H>(&self, state: &mut H)
    where
        H: hash::Hasher,
    {
        let s: &[u8] = self.as_ref();

        s.hash(state);
    }
}

impl Borrow<[u8]> for BytesMut {
    fn borrow(&self) -> &[u8] {
        self.as_ref()
    }
}

impl BorrowMut<[u8]> for BytesMut {
    fn borrow_mut(&mut self) -> &mut [u8] {
        self.as_mut()
    }
}

impl fmt::Write for BytesMut {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        if self.remaining_mut() >= s.len() {
            self.put_slice(s.as_bytes());

            Ok(())
        } else {
            Err(fmt::Error)
        }
    }

    #[inline]
    fn write_fmt(&mut self, args: fmt::Arguments<'_>) -> fmt::Result {
        fmt::write(self, args)
    }
}

impl Clone for BytesMut {
    fn clone(&self) -> Self {
        BytesMut::from(&self[..])
    }
}

impl IntoIterator for BytesMut {
    type Item = u8;
    type IntoIter = IntoIter<BytesMut>;

    fn into_iter(self) -> Self::IntoIter {
        IntoIter::new(self)
    }
}

impl<'a> IntoIterator for &'a BytesMut {
    type Item = &'a u8;
    type IntoIter = core::slice::Iter<'a, u8>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_ref().iter()
    }
}

impl Extend<u8> for BytesMut {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = u8>,
    {
        let iter = iter.into_iter();
        let (lower, _) = iter.size_hint();

        self.reserve(lower);

        // TODO: optimize
        // 1. If self.kind() == KIND_VEC, use Vec::extend
        for b in iter {
            self.put_u8(b);
        }
    }
}

impl<'a> Extend<&'a u8> for BytesMut {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = &'a u8>,
    {
        self.extend(iter.into_iter().copied());
    }
}

impl Extend<Bytes> for BytesMut {
    fn extend<T>(&mut self, iter: T)
    where
        T: IntoIterator<Item = Bytes>,
    {
        for bytes in iter {
            self.extend_from_slice(&bytes);
        }
    }
}

impl FromIterator<u8> for BytesMut {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = u8>,
    {
        BytesMut::from_vec(Vec::from_iter(iter))
    }
}

impl<'a> FromIterator<&'a u8> for BytesMut {
    fn from_iter<T>(iter: T) -> Self
    where
        T: IntoIterator<Item = &'a u8>,
    {
        BytesMut::from_iter(iter.into_iter().copied())
    }
}

// ---- Inner ----
unsafe fn increment_shared(ptr: *mut Shared) {
    let old_size = (*ptr).ref_count.fetch_add(1, Ordering::Relaxed);

    if old_size > isize::MAX as usize {
        crate::abort();
    }
}

unsafe fn release_shared(ptr: *mut Shared) {
    // `Shared` storage... follow the drop steps from Arc.
    if (*ptr).ref_count.fetch_sub(1, Ordering::Release) != 1 {
        return;
    }

    // NOTE:
    // This fence is needed to prevent reordering of use of the data and deletion of the data.
    // Because it is marked `Released`, the decreasing of the reference count synchronizes with
    // this `Acquire` fence. This means that use of data happens before decreasing the reference
    // count, which happens before this fence, which happens before the deletion of the data.
    // Thread sanitizer does not support atomic fences. Use an atomic load instead.
    (*ptr).ref_count.load(Ordering::Acquire);

    // Drop the data
    drop(Box::from_raw(ptr));
}

impl Shared {
    fn is_unique(&self) -> bool {
        self.ref_count.load(Ordering::Acquire) == 1
    }
}

#[inline]
fn original_capacity_to_repr(cap: usize) -> usize {
    let width = PTR_WIDTH - ((cap >> MIN_ORIGINAL_CAPACITY_WIDTH).leading_zeros() as usize);

    cmp::min(
        width,
        MAX_ORIGINAL_CAPACITY_WIDTH - MIN_ORIGINAL_CAPACITY_WIDTH,
    )
}

#[inline]
fn original_capacity_from_repr(repr: usize) -> usize {
    if repr == 0 {
        return 0;
    }

    1 << (repr + (MIN_ORIGINAL_CAPACITY_WIDTH - 1))
}

unsafe impl Send for BytesMut {}
unsafe impl Sync for BytesMut {}

// ---- PartialEq + PartialOrd ----
impl PartialEq<[u8]> for BytesMut {
    fn eq(&self, other: &[u8]) -> bool {
        &**self == other
    }
}

impl PartialOrd<[u8]> for BytesMut {
    fn partial_cmp(&self, other: &[u8]) -> Option<cmp::Ordering> {
        (&**self).partial_cmp(other)
    }
}

impl PartialEq<BytesMut> for [u8] {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl PartialOrd<BytesMut> for [u8] {
    fn partial_cmp(&self, other: &BytesMut) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self, other)
    }
}

impl PartialEq<str> for BytesMut {
    fn eq(&self, other: &str) -> bool {
        &**self == other.as_bytes()
    }
}

impl PartialOrd<str> for BytesMut {
    fn partial_cmp(&self, other: &str) -> Option<cmp::Ordering> {
        (**self).partial_cmp(other.as_bytes())
    }
}

impl PartialEq<BytesMut> for str {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl PartialOrd<BytesMut> for str {
    fn partial_cmp(&self, other: &BytesMut) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self.as_bytes(), other)
    }
}

impl PartialEq<Vec<u8>> for BytesMut {
    fn eq(&self, other: &Vec<u8>) -> bool {
        *self == other[..]
    }
}

impl PartialOrd<Vec<u8>> for BytesMut {
    fn partial_cmp(&self, other: &Vec<u8>) -> Option<cmp::Ordering> {
        (**self).partial_cmp(&other[..])
    }
}

impl PartialEq<BytesMut> for Vec<u8> {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl PartialOrd<BytesMut> for Vec<u8> {
    fn partial_cmp(&self, other: &BytesMut) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl PartialEq<String> for BytesMut {
    fn eq(&self, other: &String) -> bool {
        *self == other[..]
    }
}

impl PartialOrd<String> for BytesMut {
    fn partial_cmp(&self, other: &String) -> Option<cmp::Ordering> {
        (**self).partial_cmp(other.as_bytes())
    }
}

impl PartialEq<BytesMut> for String {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl PartialOrd<BytesMut> for String {
    fn partial_cmp(&self, other: &BytesMut) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self.as_bytes(), other)
    }
}

impl<'a, T: ?Sized> PartialEq<&'a T> for BytesMut
where
    BytesMut: PartialEq<T>,
{
    fn eq(&self, other: &&'a T) -> bool {
        *self == **other
    }
}

impl<'a, T: ?Sized> PartialOrd<&'a T> for BytesMut
where
    BytesMut: PartialOrd<T>,
{
    fn partial_cmp(&self, other: &&'a T) -> Option<cmp::Ordering> {
        self.partial_cmp(*other)
    }
}

impl PartialEq<BytesMut> for &[u8] {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl PartialOrd<BytesMut> for &[u8] {
    fn partial_cmp(&self, other: &BytesMut) -> Option<cmp::Ordering> {
        <[u8] as PartialOrd<[u8]>>::partial_cmp(self, other)
    }
}

impl PartialEq<BytesMut> for &str {
    fn eq(&self, other: &BytesMut) -> bool {
        *other == *self
    }
}

impl PartialOrd<BytesMut> for &str {
    fn partial_cmp(&self, other: &BytesMut) -> Option<cmp::Ordering> {
        other.partial_cmp(self)
    }
}

impl PartialEq<BytesMut> for Bytes {
    fn eq(&self, other: &BytesMut) -> bool {
        other[..] == self[..]
    }
}

impl PartialEq<Bytes> for BytesMut {
    fn eq(&self, other: &Bytes) -> bool {
        other[..] == self[..]
    }
}

impl From<BytesMut> for Vec<u8> {
    fn from(bytes: BytesMut) -> Self {
        let kind = bytes.kind();
        let bytes = ManuallyDrop::new(bytes);
        let mut vec = if kind == KIND_VEC {
            unsafe {
                let offset = bytes.get_vec_pos();

                rebuild_vec(bytes.ptr.as_ptr(), bytes.len, bytes.cap, offset)
            }
        } else {
            let shared = bytes.data as *mut Shared;

            if unsafe { (*shared).is_unique() } {
                let vec = mem::replace(unsafe { &mut (*shared).vec }, Vec::new());

                unsafe { release_shared(shared) };

                vec
            } else {
                return ManuallyDrop::into_inner(bytes).deref().to_vec();
            }
        };

        let len = bytes.len;

        unsafe {
            ptr::copy(bytes.ptr.as_ptr(), vec.as_mut_ptr(), len);

            vec.set_len(len);
        }

        vec
    }
}

#[inline]
fn vptr(ptr: *mut u8) -> NonNull<u8> {
    if cfg!(debug_assertions) {
        NonNull::new(ptr).expect("Vec pointer should be non-null")
    } else {
        unsafe { NonNull::new_unchecked(ptr) }
    }
}

#[inline]
fn invalid_ptr<T>(addr: usize) -> *mut T {
    let ptr = core::ptr::null_mut::<u8>().wrapping_add(addr);

    debug_assert_eq!(ptr as usize, addr);

    ptr.cast::<T>()
}

unsafe fn rebuild_vec(ptr: *mut u8, mut len: usize, mut cap: usize, offset: usize) -> Vec<u8> {
    let ptr = ptr.sub(offset);

    len += offset;
    cap += offset;

    Vec::from_raw_parts(ptr, len, cap)
}

// ---- impl SharedVtable ----
static SHARED_VTABLE: Vtable = Vtable {
    clone: shared_v_clone,
    to_vec: shared_v_to_vec,
    to_mut: shared_v_to_mut,
    is_unique: shared_v_is_unique,
    drop: shared_v_drop,
};

unsafe fn shared_v_clone(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Bytes {
    let shared = data.load(Ordering::Relaxed) as *mut Shared;

    increment_shared(shared);

    let data = AtomicPtr::new(shared as *mut ());

    Bytes::with_vtable(ptr, len, data, &SHARED_VTABLE)
}

unsafe fn shared_v_to_vec(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> Vec<u8> {
    let shared: *mut Shared = data.load(Ordering::Relaxed).cast();

    if (*shared).is_unique() {
        let shared = &mut *shared;

        // Drop shared
        let mut vec = mem::replace(&mut shared.vec, Vec::new());

        release_shared(shared);

        // Copy back buffer
        ptr::copy(ptr, vec.as_mut_ptr(), len);

        vec.set_len(len);

        vec
    } else {
        let v = slice::from_raw_parts(ptr, len).to_vec();

        release_shared(shared);

        v
    }
}

unsafe fn shared_v_to_mut(data: &AtomicPtr<()>, ptr: *const u8, len: usize) -> BytesMut {
    let shared: *mut Shared = data.load(Ordering::Relaxed).cast();

    if (*shared).is_unique() {
        let shared = &mut *shared;

        // NOTE:
        // The capacity is always the original capacity of the buffer minus the offset from the
        // start of the buffer
        let v = &mut shared.vec;
        let v_capacity = v.capacity();
        let v_ptr = v.as_mut_ptr();
        let offset = offset_from(ptr as *mut u8, v_ptr);
        let cap = v_capacity - offset;
        let ptr = vptr(ptr as *mut u8);

        BytesMut {
            ptr,
            len,
            cap,
            data: shared,
        }
    } else {
        let v = slice::from_raw_parts(ptr, len).to_vec();

        release_shared(shared);

        BytesMut::from_vec(v)
    }
}

unsafe fn shared_v_is_unique(data: &AtomicPtr<()>) -> bool {
    let shared = data.load(Ordering::Acquire);
    let ref_count = (*shared.cast::<Shared>()).ref_count.load(Ordering::Relaxed);
    ref_count == 1
}

unsafe fn shared_v_drop(data: &mut AtomicPtr<()>, _ptr: *const u8, _len: usize) {
    data.with_mut(|shared| {
        release_shared(*shared as *mut Shared);
    });
}

fn _split_to_must_use() {}
fn _split_off_must_use() {}
fn _split_must_use() {}

// ---- Tests ----
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_original_capacity_to_repr() {
        assert_eq!(original_capacity_to_repr(0), 0);

        let max_width = 32;

        for width in 1..(max_width + 1) {
            let cap = 1 << width - 1;

            let expected = if width < MIN_ORIGINAL_CAPACITY_WIDTH {
                0
            } else if width < MAX_ORIGINAL_CAPACITY_WIDTH {
                width - MIN_ORIGINAL_CAPACITY_WIDTH
            } else {
                MAX_ORIGINAL_CAPACITY_WIDTH - MIN_ORIGINAL_CAPACITY_WIDTH
            };

            assert_eq!(original_capacity_to_repr(cap), expected);

            if width > 1 {
                assert_eq!(original_capacity_to_repr(cap + 1), expected);
            }

            //  MIN_ORIGINAL_CAPACITY_WIDTH must be bigger than 7 to pass tests below
            if width == MIN_ORIGINAL_CAPACITY_WIDTH + 1 {
                assert_eq!(original_capacity_to_repr(cap - 24), expected - 1);
                assert_eq!(original_capacity_to_repr(cap + 76), expected);
            } else if width == MIN_ORIGINAL_CAPACITY_WIDTH + 2 {
                assert_eq!(original_capacity_to_repr(cap - 1), expected - 1);
                assert_eq!(original_capacity_to_repr(cap - 48), expected - 1);
            }
        }
    }

    #[test]
    fn test_original_capacity_from_repr() {
        assert_eq!(0, original_capacity_from_repr(0));

        let min_cap = 1 << MIN_ORIGINAL_CAPACITY_WIDTH;

        assert_eq!(min_cap, original_capacity_from_repr(1));
        assert_eq!(min_cap * 2, original_capacity_from_repr(2));
        assert_eq!(min_cap * 4, original_capacity_from_repr(3));
        assert_eq!(min_cap * 8, original_capacity_from_repr(4));
        assert_eq!(min_cap * 16, original_capacity_from_repr(5));
        assert_eq!(min_cap * 32, original_capacity_from_repr(6));
        assert_eq!(min_cap * 64, original_capacity_from_repr(7));
    }
}
