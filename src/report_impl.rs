//! The guts of [`Report`](crate::Report): `ErrorImpl` and the hand-built
//! `ErrorVTable` that let `Report` be a single-word pointer instead of a
//! `Box<dyn Error>` fat pointer.
//!
//! Vendored from [eyre](https://docs.rs/eyre) / [anyhow](https://docs.rs/anyhow).
//! An `ErrorImpl<E>` sits behind a thin pointer next to a vtable of function
//! pointers (`object_drop`, `object_ref`, `object_downcast`, …) that stand in
//! for the erased type `E`. The `unsafe` upholds subtle layout and
//! type-erasure invariants — prefer re-syncing from upstream over editing it.

use core::{
    any::TypeId,
    fmt::{self, Debug, Display},
    mem::ManuallyDrop,
    ops::{Deref, DerefMut},
    ptr::{self, NonNull},
};
use std::error::Error as StdError;

use crate::{
    Diagnostic, Report, ReportHandler, SourceCode,
    chain::Chain,
    ptr::{Mut, Own, Ref},
    wrapper::WithSourceCode,
};

impl Report {
    /// Create a new error object from any error type.
    ///
    /// The error type must be thread safe and `'static`, so that the `Report`
    /// will be as well.
    ///
    /// If the error type does not provide a backtrace, a backtrace will be
    /// created here to ensure that a backtrace exists.
    #[track_caller]
    #[must_use]
    pub fn new<E>(error: E) -> Self
    where
        E: Diagnostic + Send + Sync + 'static,
    {
        Report::from_std(error)
    }

    /// Create a new error object from a printable error message.
    ///
    /// If the argument implements [`std::error::Error`], prefer `Report::new`
    /// instead which preserves the underlying error's cause chain and
    /// backtrace. If the argument may or may not implement [`std::error::Error`]
    /// now or in the future, use `miette!(err)` which handles either way
    /// correctly.
    ///
    /// `Report::msg("...")` is equivalent to `miette!("...")` but occasionally
    /// convenient in places where a function is preferable over a macro, such
    /// as iterator or stream combinators:
    ///
    /// ```
    /// # mod ffi {
    /// #     pub struct Input;
    /// #     pub struct Output;
    /// #     pub async fn do_some_work(_: Input) -> Result<Output, &'static str> {
    /// #         unimplemented!()
    /// #     }
    /// # }
    /// #
    /// # use ffi::{Input, Output};
    /// #
    /// use futures::stream::{Stream, StreamExt, TryStreamExt};
    /// use miette::{Report, Result};
    ///
    /// async fn demo<S>(stream: S) -> Result<Vec<Output>>
    /// where
    ///     S: Stream<Item = Input>,
    /// {
    ///     stream
    ///         .then(ffi::do_some_work) // returns Result<Output, &str>
    ///         .map_err(Report::msg)
    ///         .try_collect()
    ///         .await
    /// }
    /// ```
    #[track_caller]
    #[must_use]
    pub fn msg<M>(message: M) -> Self
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        Report::from_adhoc(message)
    }

    /// Create a new error object from a boxed [`Diagnostic`].
    ///
    /// The boxed type must be thread safe and 'static, so that the `Report`
    /// will be as well.
    ///
    /// Boxed `Diagnostic`s don't implement `Diagnostic` themselves due to trait coherence issues.
    /// This method allows you to create a `Report` from a boxed `Diagnostic`.
    #[track_caller]
    #[must_use]
    pub fn new_boxed(error: Box<dyn Diagnostic + Send + Sync + 'static>) -> Self {
        Report::from_boxed(error)
    }

    #[track_caller]
    pub(crate) fn from_std<E>(error: E) -> Self
    where
        E: Diagnostic + Send + Sync + 'static,
    {
        let vtable = &ErrorVTable {
            drop: object_drop::<E>,
            as_diagnostic: object_ref::<E>,
            as_error: object_ref_stderr::<E>,
            into_diagnostic: object_boxed::<E>,
            into_error: object_boxed_stderr::<E>,
            downcast: object_downcast::<E>,
            drop_rest: object_drop_front::<E>,
        };

        let handler = Some(crate::report::capture_handler(&error));

        // SAFETY: Every vtable entry above is monomorphized for `E`.
        unsafe { Report::construct(error, vtable, handler) }
    }

    #[track_caller]
    pub(crate) fn from_adhoc<M>(message: M) -> Self
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        use crate::wrapper::MessageError;
        let error: MessageError<M> = MessageError(message);
        let vtable = &ErrorVTable {
            drop: object_drop::<MessageError<M>>,
            as_diagnostic: object_ref::<MessageError<M>>,
            as_error: object_ref_stderr::<MessageError<M>>,
            into_diagnostic: object_boxed::<MessageError<M>>,
            into_error: object_boxed_stderr::<MessageError<M>>,
            downcast: object_downcast::<M>,
            drop_rest: object_drop_front::<M>,
        };

        let handler = Some(crate::report::capture_handler(&error));

        // SAFETY: `MessageError` is transparent, and every vtable entry is
        // monomorphized for either `MessageError<M>` or its inner `M`.
        unsafe { Report::construct(error, vtable, handler) }
    }

    #[track_caller]
    pub(crate) fn from_boxed(error: Box<dyn Diagnostic + Send + Sync>) -> Self {
        use crate::wrapper::BoxedError;
        let error = BoxedError(error);
        let handler = Some(crate::report::capture_handler(&error));

        let vtable = &ErrorVTable {
            drop: object_drop::<BoxedError>,
            as_diagnostic: object_ref::<BoxedError>,
            as_error: object_ref_stderr::<BoxedError>,
            into_diagnostic: object_boxed::<BoxedError>,
            into_error: object_boxed_stderr::<BoxedError>,
            downcast: object_downcast::<Box<dyn Diagnostic + Send + Sync>>,
            drop_rest: object_drop_front::<Box<dyn Diagnostic + Send + Sync>>,
        };

        // SAFETY: BoxedError is repr(transparent) so it is okay for the vtable
        // to allow casting to Box<dyn StdError + Send + Sync>.
        unsafe { Report::construct(error, vtable, handler) }
    }

    // Takes backtrace as argument rather than capturing it here so that the
    // user sees one fewer layer of wrapping noise in the backtrace.
    //
    // Unsafe because the given vtable must have sensible behavior on the error
    // value of type E.
    unsafe fn construct<E>(
        error: E,
        vtable: &'static ErrorVTable,
        handler: Option<Box<dyn ReportHandler>>,
    ) -> Self
    where
        E: Diagnostic + Send + Sync + 'static,
    {
        let inner = Box::new(ErrorImpl { vtable, handler, object: error });
        // Erase the concrete type of E from the compile-time type system. This
        // is equivalent to the safe unsize coercion from Box<ErrorImpl<E>> to
        // Box<ErrorImpl<dyn StdError + Send + Sync + 'static>> except that the
        // result is a thin pointer. The necessary behavior for manipulating the
        // underlying ErrorImpl<E> is preserved in the vtable provided by the
        // caller rather than a builtin fat pointer vtable.
        let inner = Own::new(inner).cast::<ErasedErrorImpl>();
        Report { inner }
    }

    /// An iterator of the chain of source errors contained by this Report.
    ///
    /// This iterator will visit every error in the cause chain of this error
    /// object, beginning with the error that this error object was created
    /// from.
    ///
    /// # Example
    ///
    /// ```
    /// use miette::Report;
    /// use std::io;
    ///
    /// pub fn underlying_io_error_kind(error: &Report) -> Option<io::ErrorKind> {
    ///     for cause in error.chain() {
    ///         if let Some(io_error) = cause.downcast_ref::<io::Error>() {
    ///             return Some(io_error.kind());
    ///         }
    ///     }
    ///     None
    /// }
    /// ```
    #[must_use]
    pub fn chain(&self) -> Chain<'_> {
        // SAFETY: `self.inner` points to an `ErrorImpl` paired with its original
        // vtable for the lifetime of `self`.
        unsafe { ErrorImpl::chain(self.inner.by_ref()) }
    }

    /// The lowest level cause of this error &mdash; this error's cause's
    /// cause's cause etc.
    ///
    /// The root cause is the last error in the iterator produced by
    /// [`chain()`](Report::chain).
    ///
    /// # Panics
    ///
    /// This would panic only if the report's cause chain were empty, which a
    /// valid [`Report`] never permits.
    #[must_use]
    pub fn root_cause(&self) -> &(dyn StdError + 'static) {
        self.chain().next_back().unwrap()
    }

    /// Returns true if `E` is the type held by this error object.
    ///
    /// For errors constructed from messages, this method returns true if `E`
    /// matches the type of the message `D` **or** the type of the error on
    /// which the message has been attached. For details about the
    /// interaction between message and downcasting, [see here].
    ///
    /// [see here]: trait.WrapErr.html#effect-on-downcasting
    #[must_use]
    pub fn is<E>(&self) -> bool
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        self.downcast_ref::<E>().is_some()
    }

    /// Attempt to downcast the error object to a concrete type.
    ///
    /// # Errors
    ///
    /// Returns the original report when its stored value is not an `E`.
    pub fn downcast<E>(self) -> Result<E, Self>
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        let target = TypeId::of::<E>();
        let inner = self.inner.by_mut();
        // SAFETY: The vtable belongs to this allocation. Its downcast entry
        // returns an `E` pointer only after a matching `TypeId`.
        unsafe {
            // Use vtable to find NonNull<()> which points to a value of type E
            // somewhere inside the data structure.
            let addr = match (vtable(inner.ptr).downcast)(inner.by_ref(), target) {
                Some(addr) => addr.by_mut().extend(),
                None => return Err(self),
            };

            // Prepare to read E out of the data structure. We'll drop the rest
            // of the data structure separately so that E is not dropped.
            let outer = ManuallyDrop::new(self);

            // Read E from where the vtable found it.
            let error = addr.cast::<E>().read();

            // Drop rest of the data structure outside of E.
            (vtable(outer.inner.ptr).drop_rest)(outer.inner, target);

            Ok(error)
        }
    }

    /// Downcast this error object by reference.
    ///
    /// # Example
    ///
    /// ```
    /// # use miette::{Report, miette};
    /// # use std::fmt::{self, Display};
    /// # use std::task::Poll;
    /// #
    /// # #[derive(Debug)]
    /// # enum DataStoreError {
    /// #     Censored(()),
    /// # }
    /// #
    /// # impl Display for DataStoreError {
    /// #     fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
    /// #         unimplemented!()
    /// #     }
    /// # }
    /// #
    /// # impl std::error::Error for DataStoreError {}
    /// #
    /// # const REDACTED_CONTENT: () = ();
    /// #
    /// # let error: Report = miette!("...");
    /// # let root_cause = &error;
    /// #
    /// # let ret =
    /// // If the error was caused by redaction, then return a tombstone instead
    /// // of the content.
    /// match root_cause.downcast_ref::<DataStoreError>() {
    ///     Some(DataStoreError::Censored(_)) => Ok(Poll::Ready(REDACTED_CONTENT)),
    ///     None => Err(error),
    /// }
    /// # ;
    /// ```
    #[must_use]
    pub fn downcast_ref<E>(&self) -> Option<&E>
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        let target = TypeId::of::<E>();
        // SAFETY: The vtable belongs to this allocation and validates `target`
        // before returning the field pointer.
        unsafe {
            // Use vtable to find NonNull<()> which points to a value of type E
            // somewhere inside the data structure.
            let addr = (vtable(self.inner.ptr).downcast)(self.inner.by_ref(), target)?;
            Some(addr.cast::<E>().deref())
        }
    }

    /// Downcast this error object by mutable reference.
    pub fn downcast_mut<E>(&mut self) -> Option<&mut E>
    where
        E: Display + Debug + Send + Sync + 'static,
    {
        let target = TypeId::of::<E>();
        // SAFETY: As above, with exclusive access guaranteed by `&mut self`.
        unsafe {
            // Use vtable to find NonNull<()> which points to a value of type E
            // somewhere inside the data structure.
            let addr = (vtable(self.inner.ptr).downcast)(self.inner.by_ref(), target)?.by_mut();
            Some(addr.cast::<E>().deref_mut())
        }
    }

    /// Get a reference to the Handler for this Report.
    ///
    /// # Panics
    ///
    /// Panics only if the report's internal handler invariant is violated.
    #[must_use]
    pub fn handler(&self) -> &dyn ReportHandler {
        // SAFETY: `inner` is a live allocation owned by `self`.
        unsafe { self.inner.by_ref().deref().handler.as_ref().unwrap().as_ref() }
    }

    /// Get a mutable reference to the Handler for this Report.
    ///
    /// # Panics
    ///
    /// Panics only if the report's internal handler invariant is violated.
    pub fn handler_mut(&mut self) -> &mut dyn ReportHandler {
        // SAFETY: `&mut self` guarantees exclusive access to the live allocation.
        unsafe { self.inner.by_mut().deref_mut().handler.as_mut().unwrap().as_mut() }
    }

    /// Provide source code for this error
    #[must_use]
    pub fn with_source_code(self, source_code: impl SourceCode + 'static) -> Report {
        WithSourceCode { source_code, error: self }.into()
    }
}

impl<E> From<E> for Report
where
    E: Diagnostic + Send + Sync + 'static,
{
    #[track_caller]
    fn from(error: E) -> Self {
        Report::from_std(error)
    }
}

impl Deref for Report {
    type Target = dyn Diagnostic + Send + Sync + 'static;

    fn deref(&self) -> &Self::Target {
        // SAFETY: `inner` and its vtable remain valid for the lifetime of `self`.
        unsafe { ErrorImpl::diagnostic(self.inner.by_ref()) }
    }
}

impl DerefMut for Report {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: `&mut self` guarantees exclusive access to the erased value.
        unsafe { ErrorImpl::diagnostic_mut(self.inner.by_mut()) }
    }
}

impl Display for Report {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: `inner` and its vtable remain valid while formatting.
        unsafe { ErrorImpl::display(self.inner.by_ref(), formatter) }
    }
}

impl Debug for Report {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: `inner` and its vtable remain valid while formatting.
        unsafe { ErrorImpl::debug(self.inner.by_ref(), formatter) }
    }
}

impl Drop for Report {
    fn drop(&mut self) {
        // SAFETY: The allocation is still owned here, and its matching vtable
        // provides the correct concrete drop implementation.
        unsafe {
            // Invoke the vtable's drop behavior.
            (vtable(self.inner.ptr).drop)(self.inner);
        }
    }
}

struct ErrorVTable {
    drop: unsafe fn(Own<ErasedErrorImpl>),
    as_diagnostic:
        unsafe fn(Ref<'_, ErasedErrorImpl>) -> Ref<'_, dyn Diagnostic + Send + Sync + 'static>,
    as_error: unsafe fn(Ref<'_, ErasedErrorImpl>) -> Ref<'_, dyn StdError + Send + Sync + 'static>,
    into_diagnostic: unsafe fn(Own<ErasedErrorImpl>) -> Box<dyn Diagnostic + Send + Sync + 'static>,
    into_error: unsafe fn(Own<ErasedErrorImpl>) -> Box<dyn StdError + Send + Sync + 'static>,
    downcast: unsafe fn(Ref<'_, ErasedErrorImpl>, TypeId) -> Option<Ref<'_, ()>>,
    drop_rest: unsafe fn(Own<ErasedErrorImpl>, TypeId),
}

/// # Safety
///
/// `e` must point to an `ErrorImpl<E>` allocation.
unsafe fn object_drop<E>(e: Own<ErasedErrorImpl>) {
    // SAFETY: Required by this function's contract.
    unsafe {
        // Cast back to ErrorImpl<E> so that the allocator receives the correct
        // Layout to deallocate the Box's memory.
        let unerased = e.cast::<ErrorImpl<E>>().boxed();
        drop(unerased);
    }
}

/// # Safety
///
/// `e` must point to an `ErrorImpl<E>` allocation, and `target` must describe
/// the value already moved out by the caller.
unsafe fn object_drop_front<E>(e: Own<ErasedErrorImpl>, target: TypeId) {
    // SAFETY: Required by this function's contract; `ManuallyDrop<E>` prevents
    // a second drop of the extracted value.
    unsafe {
        // Drop the fields of ErrorImpl other than E as well as the Box allocation,
        // without dropping E itself. This is used by downcast after doing a
        // ptr::read to take ownership of the E.
        let _ = target;
        let unerased = e.cast::<ErrorImpl<ManuallyDrop<E>>>().boxed();
        drop(unerased);
    }
}

/// # Safety
///
/// `e` must point to an `ErrorImpl<E>`.
unsafe fn object_ref<E>(
    e: Ref<'_, ErasedErrorImpl>,
) -> Ref<'_, dyn Diagnostic + Send + Sync + 'static>
where
    E: Diagnostic + Send + Sync + 'static,
{
    // SAFETY: Required by this function's contract. `addr_of!` yields the
    // non-null address of the live `object` field.
    unsafe {
        // Attach E's native StdError vtable onto a pointer to `self.object`.
        let unerased = e.cast::<ErrorImpl<E>>();

        Ref::from_raw(NonNull::new_unchecked(ptr::addr_of!((*unerased.as_ptr()).object).cast_mut()))
    }
}

/// # Safety
///
/// `e` must point to an `ErrorImpl<E>`.
unsafe fn object_ref_stderr<E>(
    e: Ref<'_, ErasedErrorImpl>,
) -> Ref<'_, dyn StdError + Send + Sync + 'static>
where
    E: StdError + Send + Sync + 'static,
{
    // SAFETY: Required by this function's contract. `addr_of!` yields the
    // non-null address of the live `object` field.
    unsafe {
        // Attach E's native StdError vtable onto a pointer to `self.object`.
        let unerased = e.cast::<ErrorImpl<E>>();

        Ref::from_raw(NonNull::new_unchecked(ptr::addr_of!((*unerased.as_ptr()).object).cast_mut()))
    }
}

/// # Safety
///
/// `e` must own an `ErrorImpl<E>` allocation.
unsafe fn object_boxed<E>(e: Own<ErasedErrorImpl>) -> Box<dyn Diagnostic + Send + Sync + 'static>
where
    E: Diagnostic + Send + Sync + 'static,
{
    // SAFETY: Required by this function's contract.
    unsafe {
        // Attach ErrorImpl<E>'s native StdError vtable. The StdError impl is below.
        e.cast::<ErrorImpl<E>>().boxed()
    }
}

/// # Safety
///
/// `e` must own an `ErrorImpl<E>` allocation.
unsafe fn object_boxed_stderr<E>(
    e: Own<ErasedErrorImpl>,
) -> Box<dyn StdError + Send + Sync + 'static>
where
    E: StdError + Send + Sync + 'static,
{
    // SAFETY: Required by this function's contract.
    unsafe {
        // Attach ErrorImpl<E>'s native StdError vtable. The StdError impl is below.
        e.cast::<ErrorImpl<E>>().boxed()
    }
}

/// # Safety
///
/// `e` must point to an `ErrorImpl<E>`.
unsafe fn object_downcast<E>(e: Ref<'_, ErasedErrorImpl>, target: TypeId) -> Option<Ref<'_, ()>>
where
    E: 'static,
{
    // SAFETY: Required by this function's contract. A pointer is returned only
    // after confirming that the requested type is `E`.
    unsafe {
        if TypeId::of::<E>() == target {
            // Caller is looking for an E pointer and e is ErrorImpl<E>, take a
            // pointer to its E field.
            let unerased = e.cast::<ErrorImpl<E>>();

            Some(
                Ref::from_raw(NonNull::new_unchecked(
                    ptr::addr_of!((*unerased.as_ptr()).object).cast_mut(),
                ))
                .cast::<()>(),
            )
        } else {
            None
        }
    }
}

// repr C to ensure that E remains in the final position.
#[repr(C)]
#[expect(clippy::redundant_pub_crate, reason = "keeps erased storage crate-private")]
pub(crate) struct ErrorImpl<E> {
    vtable: &'static ErrorVTable,
    pub(crate) handler: Option<Box<dyn ReportHandler>>,
    // NOTE: Don't use directly. Use only through vtable. Erased type may have
    // different alignment.
    object: E,
}

type ErasedErrorImpl = ErrorImpl<()>;

/// # Safety
///
/// `p` must point to an `ErrorImpl`; `ErrorVTable` is its first field.
unsafe fn vtable(p: NonNull<ErasedErrorImpl>) -> &'static ErrorVTable {
    // SAFETY: Required by this function's contract and guaranteed by
    // `ErrorImpl`'s `repr(C)` layout.
    unsafe { (p.as_ptr() as *const &'static ErrorVTable).read() }
}

impl<E> ErrorImpl<E> {
    fn erase(&self) -> Ref<'_, ErasedErrorImpl> {
        // Erase the concrete type of E but preserve the vtable in self.vtable
        // for manipulating the resulting thin pointer. This is analogous to an
        // unsize coercion.
        Ref::new(self).cast::<ErasedErrorImpl>()
    }
}

impl ErasedErrorImpl {
    pub(crate) unsafe fn error<'a>(
        this: Ref<'a, Self>,
    ) -> &'a (dyn StdError + Send + Sync + 'static) {
        // SAFETY: The caller guarantees `this` is paired with its original
        // vtable, whose `as_error` entry reconstructs the correct trait object.
        unsafe {
            // Use vtable to attach E's native StdError vtable for the right
            // original type E.
            (vtable(this.ptr).as_error)(this).deref()
        }
    }

    pub(crate) unsafe fn diagnostic<'a>(
        this: Ref<'a, Self>,
    ) -> &'a (dyn Diagnostic + Send + Sync + 'static) {
        // SAFETY: The caller guarantees `this` is paired with its original
        // vtable, whose `as_diagnostic` entry reconstructs the trait object.
        unsafe {
            // Use vtable to attach E's native StdError vtable for the right
            // original type E.
            (vtable(this.ptr).as_diagnostic)(this).deref()
        }
    }

    pub(crate) unsafe fn diagnostic_mut<'a>(
        this: Mut<'a, Self>,
    ) -> &'a mut (dyn Diagnostic + Send + Sync + 'static) {
        // SAFETY: The caller additionally guarantees exclusive access through
        // `this`.
        unsafe {
            // Use vtable to attach E's native StdError vtable for the right
            // original type E.
            (vtable(this.ptr).as_diagnostic)(this.by_ref()).by_mut().deref_mut()
        }
    }

    pub(crate) unsafe fn chain(this: Ref<'_, Self>) -> Chain<'_> {
        // SAFETY: The caller guarantees `this` is a valid erased error.
        unsafe { Chain::new(Self::error(this)) }
    }
}

impl<E> StdError for ErrorImpl<E>
where
    E: StdError,
{
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // SAFETY: `erase` preserves this allocation's vtable and lifetime.
        unsafe { ErrorImpl::diagnostic(self.erase()).source() }
    }
}

impl<E> Diagnostic for ErrorImpl<E> where E: Diagnostic {}

impl<E> Debug for ErrorImpl<E>
where
    E: Debug,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: `erase` preserves this allocation's vtable and lifetime.
        unsafe { ErrorImpl::debug(self.erase(), formatter) }
    }
}

impl<E> Display for ErrorImpl<E>
where
    E: Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // SAFETY: `erase` preserves this allocation's vtable and lifetime.
        unsafe { Display::fmt(ErrorImpl::diagnostic(self.erase()), formatter) }
    }
}

impl From<Report> for Box<dyn Diagnostic + Send + Sync + 'static> {
    fn from(error: Report) -> Self {
        let outer = ManuallyDrop::new(error);
        // SAFETY: `outer` retains its allocation and matching vtable; the
        // vtable conversion takes ownership exactly once.
        unsafe {
            // Use vtable to attach ErrorImpl<E>'s native StdError vtable for
            // the right original type E.
            (vtable(outer.inner.ptr).into_diagnostic)(outer.inner)
        }
    }
}

impl From<Report> for Box<dyn StdError + Send + Sync + 'static> {
    fn from(error: Report) -> Self {
        let outer = ManuallyDrop::new(error);
        // SAFETY: `outer` retains its allocation and matching vtable; the
        // vtable conversion takes ownership exactly once.
        unsafe {
            // Use vtable to attach ErrorImpl<E>'s native StdError vtable for
            // the right original type E.
            (vtable(outer.inner.ptr).into_error)(outer.inner)
        }
    }
}

impl From<Report> for Box<dyn Diagnostic + 'static> {
    fn from(error: Report) -> Self {
        Box::<dyn Diagnostic + Send + Sync>::from(error)
    }
}

impl From<Report> for Box<dyn StdError + 'static> {
    fn from(error: Report) -> Self {
        Box::<dyn StdError + Send + Sync>::from(error)
    }
}

impl AsRef<dyn Diagnostic + Send + Sync> for Report {
    fn as_ref(&self) -> &(dyn Diagnostic + Send + Sync + 'static) {
        &**self
    }
}

impl AsRef<dyn Diagnostic> for Report {
    fn as_ref(&self) -> &(dyn Diagnostic + 'static) {
        &**self
    }
}

impl AsRef<dyn StdError + Send + Sync> for Report {
    fn as_ref(&self) -> &(dyn StdError + Send + Sync + 'static) {
        // SAFETY: `inner` and its matching vtable live as long as `self`.
        unsafe { ErrorImpl::error(self.inner.by_ref()) }
    }
}

impl AsRef<dyn StdError> for Report {
    fn as_ref(&self) -> &(dyn StdError + 'static) {
        // SAFETY: `inner` and its matching vtable live as long as `self`.
        unsafe { ErrorImpl::error(self.inner.by_ref()) }
    }
}

impl std::borrow::Borrow<dyn Diagnostic> for Report {
    fn borrow(&self) -> &(dyn Diagnostic + 'static) {
        self.as_ref()
    }
}
