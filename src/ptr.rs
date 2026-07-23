//! Thin owning/borrowing pointer wrappers (`Own`, `Ref`, `Mut`) that
//! [`Report`](crate::Report)'s type-erased storage is built on. Vendored from
//! [anyhow](https://docs.rs/anyhow)'s `ptr.rs`.

use std::{marker::PhantomData, ptr::NonNull};

#[repr(transparent)]
/// A raw pointer that owns its pointee
pub(crate) struct Own<T>
where
    T: ?Sized,
{
    ptr: NonNull<T>,
}

// SAFETY: `Own<T>` has the same thread-safety requirements as the owned `T`.
unsafe impl<T> Send for Own<T> where T: ?Sized + Send {}
// SAFETY: shared access through `Own<T>` is safe exactly when `T` is `Sync`.
unsafe impl<T> Sync for Own<T> where T: ?Sized + Sync {}

impl<T> Own<T>
where
    T: ?Sized,
{
    pub(crate) fn new(ptr: Box<T>) -> Self {
        Own { ptr: NonNull::from(Box::leak(ptr)) }
    }

    /// # Safety
    ///
    /// `ptr` must uniquely own a live allocation for `T`. After this call, no
    /// other owner may use or free that allocation.
    pub(crate) const unsafe fn from_raw(ptr: NonNull<T>) -> Self {
        Own { ptr }
    }

    /// # Safety
    ///
    /// The allocation owned by this pointer must have a layout compatible with
    /// `U`, and the returned pointer must remain responsible for that same
    /// allocation.
    pub(crate) unsafe fn cast<U>(self) -> Own<U> {
        Own { ptr: self.ptr.cast() }
    }

    /// # Safety
    ///
    /// This pointer must still uniquely own an allocation created for `T`, and
    /// no other owner may use or free it after this call.
    pub(crate) unsafe fn boxed(self) -> Box<T> {
        // SAFETY: upheld by the caller as documented above.
        unsafe { Box::from_raw(self.ptr.as_ptr()) }
    }

    pub(crate) const fn by_ref(&self) -> Ref<'_, T> {
        Ref { ptr: self.ptr, lifetime: PhantomData }
    }

    pub(crate) const fn by_mut(&mut self) -> Mut<'_, T> {
        Mut { ptr: self.ptr, lifetime: PhantomData }
    }

    pub(crate) const fn as_non_null(&self) -> NonNull<T> {
        self.ptr
    }
}

#[allow(explicit_outlives_requirements)]
#[repr(transparent)]
/// A raw pointer that represents a shared borrow of its pointee
pub(crate) struct Ref<'a, T>
where
    T: ?Sized,
{
    ptr: NonNull<T>,
    lifetime: PhantomData<&'a T>,
}

impl<T> Copy for Ref<'_, T> where T: ?Sized {}

impl<T> Clone for Ref<'_, T>
where
    T: ?Sized,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<'a, T> Ref<'a, T>
where
    T: ?Sized,
{
    pub(crate) fn new(ptr: &'a T) -> Self {
        Ref { ptr: NonNull::from(ptr), lifetime: PhantomData }
    }

    /// # Safety
    ///
    /// `ptr` must be aligned, initialized, and valid for shared access for the
    /// inferred lifetime `'a`.
    pub(crate) const unsafe fn from_raw(ptr: NonNull<T>) -> Self {
        Ref { ptr, lifetime: PhantomData }
    }

    /// # Safety
    ///
    /// `ptr` must point to a valid `U` within the same live allocation for the
    /// whole lifetime `'a`.
    pub(crate) unsafe fn cast<U>(self) -> Ref<'a, U> {
        Ref { ptr: self.ptr.cast(), lifetime: PhantomData }
    }

    /// # Safety
    ///
    /// The pointee must be uniquely accessible for `'a`; no other `Ref` or
    /// reference may be used while the returned `Mut` is live.
    pub(crate) const unsafe fn by_mut(self) -> Mut<'a, T> {
        Mut { ptr: self.ptr, lifetime: PhantomData }
    }

    pub(crate) const fn as_ptr(self) -> *const T {
        self.ptr.as_ptr() as *const T
    }

    pub(crate) fn deref(self) -> &'a T {
        // SAFETY: every constructor establishes validity for `'a`.
        unsafe { &*self.ptr.as_ptr() }
    }

    pub(crate) const fn as_non_null(self) -> NonNull<T> {
        self.ptr
    }
}

#[allow(explicit_outlives_requirements)]
#[repr(transparent)]
/// A raw pointer that represents a unique borrow of its pointee
pub(crate) struct Mut<'a, T>
where
    T: ?Sized,
{
    ptr: NonNull<T>,
    lifetime: PhantomData<&'a mut T>,
}

impl<'a, T> Mut<'a, T>
where
    T: ?Sized,
{
    /// # Safety
    ///
    /// `ptr` must point to a valid, uniquely accessible `U` within the same
    /// live allocation for the whole lifetime `'a`.
    pub(crate) unsafe fn cast<U>(self) -> Mut<'a, U> {
        Mut { ptr: self.ptr.cast(), lifetime: PhantomData }
    }

    pub(crate) const fn into_ref(self) -> Ref<'a, T> {
        Ref { ptr: self.ptr, lifetime: PhantomData }
    }

    /// # Safety
    ///
    /// The pointee must remain valid and uniquely accessible for the new
    /// lifetime `'b`.
    pub(crate) unsafe fn extend<'b>(self) -> Mut<'b, T> {
        Mut { ptr: self.ptr, lifetime: PhantomData }
    }

    pub(crate) fn deref_mut(self) -> &'a mut T {
        // SAFETY: every constructor establishes validity and uniqueness for `'a`.
        unsafe { &mut *self.ptr.as_ptr() }
    }

    pub(crate) const fn as_non_null(&self) -> NonNull<T> {
        self.ptr
    }
}

impl<T> Mut<'_, T> {
    /// # Safety
    ///
    /// The pointee must be initialized, and the caller must ensure it is not
    /// subsequently read or dropped through another owner.
    pub(crate) unsafe fn read(self) -> T {
        // SAFETY: upheld by the caller as documented above.
        unsafe { self.ptr.as_ptr().read() }
    }
}
