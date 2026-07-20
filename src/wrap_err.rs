#![allow(clippy::needless_doctest_main)]
//! `WrapErr`/`Context` (attach a message to an error) plus the wrapper types
//! `Report` builds on: `MessageError`, `BoxedError`, and `WithSourceCode`.
//! Merged from the former `eyreish/context.rs` and `eyreish/wrapper.rs`.

use core::fmt::{self, Debug, Display, Write};
use std::{borrow::Cow, error::Error as StdError};

use crate as miette;
use crate::{
    Diagnostic, Report, SourceCode,
    report_impl::{ContextError, ErrorImpl},
};

/// Provides the [`wrap_err()`](WrapErr::wrap_err) method for [`Result`].
///
/// This trait is sealed and cannot be implemented for types outside of
/// `miette`.
///
/// # Example
///
/// ```
/// use miette::{WrapErr, IntoDiagnostic, Result};
/// use std::{fs, path::PathBuf};
///
/// pub struct ImportantThing {
///     path: PathBuf,
/// }
///
/// impl ImportantThing {
///     # const IGNORE: &'static str = stringify! {
///     pub fn detach(&mut self) -> Result<()> {...}
///     # };
///     # fn detach(&mut self) -> Result<()> {
///     #     unimplemented!()
///     # }
/// }
///
/// pub fn do_it(mut it: ImportantThing) -> Result<Vec<u8>> {
///     it.detach().wrap_err("Failed to detach the important thing")?;
///
///     let path = &it.path;
///     let content = fs::read(path)
///         .into_diagnostic()
///         .wrap_err_with(|| format!(
///             "Failed to read instrs from {}",
///             path.display())
///         )?;
///
///     Ok(content)
/// }
/// ```
///
/// When printed, the outermost error would be printed first and the lower
/// level underlying causes would be enumerated below.
///
/// ```console
/// Error: Failed to read instrs from ./path/to/instrs.json
///
/// Caused by:
///     No such file or directory (os error 2)
/// ```
///
/// # Wrapping Types That Do Not Implement `Error`
///
/// For example `&str` and `Box<dyn Error>`.
///
/// Due to restrictions for coherence `Report` cannot implement `From` for types
/// that don't implement `Error`. Attempts to do so will give `"this type might
/// implement Error in the future"` as an error. As such, `wrap_err()`, which
/// uses `From` under the hood, cannot be used to wrap these types. Instead we
/// encourage you to use the combinators provided for `Result` in `std`/`core`.
///
/// For example, instead of this:
///
/// ```rust,compile_fail
/// use std::error::Error;
/// use miette::{WrapErr, Report};
///
/// fn wrap_example(err: Result<(), Box<dyn Error + Send + Sync + 'static>>)
///     -> Result<(), Report>
/// {
///     err.wrap_err("saw a downstream error")
/// }
/// ```
///
/// We encourage you to write this:
///
/// ```rust
/// use miette::{miette, Report, WrapErr};
/// use std::error::Error;
///
/// fn wrap_example(err: Result<(), Box<dyn Error + Send + Sync + 'static>>) -> Result<(), Report> {
///     err.map_err(|e| miette!(e))
///         .wrap_err("saw a downstream error")
/// }
/// ```
///
/// # Effect on Downcasting
///
/// After attaching a message of type `D` onto an error of type `E`, the
/// resulting `miette::Error` may be downcast to `D` **or** to `E`.
///
/// That is, in codebases that rely on downcasting, `miette`'s `wrap_err()`
/// supports both of the following use cases:
///
///   - **Attaching messages whose type is insignificant onto errors whose type
///     is used in downcasts.**
///
///     In other error libraries whose `wrap_err()` is not designed this way, it
///     can be risky to introduce messages to existing code because new message
///     might break existing working downcasts. In miette, any downcast that
///     worked before adding the message will continue to work after you add a
///     message, so you should freely wrap errors wherever it would be helpful.
///
///     ```
///     # use miette::bail;
///     # use thiserror::Error;
///     #
///     # #[derive(Error, Debug)]
///     # #[error("???")]
///     # struct SuspiciousError;
///     #
///     # fn helper() -> Result<()> {
///     #     bail!(SuspiciousError);
///     # }
///     #
///     use miette::{WrapErr, Result};
///
///     fn do_it() -> Result<()> {
///         helper().wrap_err("Failed to complete the work")?;
///         # const IGNORE: &str = stringify! {
///         ...
///         # };
///         # unreachable!()
///     }
///
///     fn main() {
///         let err = do_it().unwrap_err();
///         if let Some(e) = err.downcast_ref::<SuspiciousError>() {
///             // If helper() returned SuspiciousError, this downcast will
///             // correctly succeed even with the message in between.
///             # return;
///         }
///         # panic!("expected downcast to succeed");
///     }
///     ```
///
///   - **Attaching message whose type is used in downcasts onto errors whose
///     type is insignificant.**
///
///     Some codebases prefer to use machine-readable messages to categorize
///     lower level errors in a way that will be actionable to higher levels of
///     the application.
///
///     ```
///     # use miette::bail;
///     # use thiserror::Error;
///     #
///     # #[derive(Error, Debug)]
///     # #[error("???")]
///     # struct HelperFailed;
///     #
///     # fn helper() -> Result<()> {
///     #     bail!("no such file or directory");
///     # }
///     #
///     use miette::{WrapErr, Result};
///
///     fn do_it() -> Result<()> {
///         helper().wrap_err(HelperFailed)?;
///         # const IGNORE: &str = stringify! {
///         ...
///         # };
///         # unreachable!()
///     }
///
///     fn main() {
///         let err = do_it().unwrap_err();
///         if let Some(e) = err.downcast_ref::<HelperFailed>() {
///             // If helper failed, this downcast will succeed because
///             // HelperFailed is the message that has been attached to
///             // that error.
///             # return;
///         }
///         # panic!("expected downcast to succeed");
///     }
///     ```
pub trait WrapErr<T, E>: private::Sealed {
    /// Wrap the error value with a new adhoc error
    #[track_caller]
    fn wrap_err<D>(self, msg: D) -> Result<T, Report>
    where
        D: Display + Send + Sync + 'static;

    /// Wrap the error value with a new adhoc error that is evaluated lazily
    /// only once an error does occur.
    #[track_caller]
    fn wrap_err_with<D, F>(self, f: F) -> Result<T, Report>
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D;

    /// Compatibility re-export of `wrap_err()` for interop with `anyhow`
    #[track_caller]
    fn context<D>(self, msg: D) -> Result<T, Report>
    where
        D: Display + Send + Sync + 'static;

    /// Compatibility re-export of `wrap_err_with()` for interop with `anyhow`
    #[track_caller]
    fn with_context<D, F>(self, f: F) -> Result<T, Report>
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D;
}

/// Compatibility re-export of `WrapErr` for interop with `anyhow`
#[allow(unreachable_pub)]
pub use WrapErr as Context;

mod ext {
    use super::*;

    pub trait Diag {
        #[track_caller]
        fn ext_report<D>(self, msg: D) -> Report
        where
            D: Display + Send + Sync + 'static;
    }

    impl<E> Diag for E
    where
        E: Diagnostic + Send + Sync + 'static,
    {
        fn ext_report<D>(self, msg: D) -> Report
        where
            D: Display + Send + Sync + 'static,
        {
            Report::from_msg(msg, self)
        }
    }

    impl Diag for Report {
        fn ext_report<D>(self, msg: D) -> Report
        where
            D: Display + Send + Sync + 'static,
        {
            self.wrap_err(msg)
        }
    }
}

impl<T, E> WrapErr<T, E> for Result<T, E>
where
    E: ext::Diag + Send + Sync + 'static,
{
    fn wrap_err<D>(self, msg: D) -> Result<T, Report>
    where
        D: Display + Send + Sync + 'static,
    {
        match self {
            Ok(t) => Ok(t),
            Err(e) => Err(e.ext_report(msg)),
        }
    }

    fn wrap_err_with<D, F>(self, msg: F) -> Result<T, Report>
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        match self {
            Ok(t) => Ok(t),
            Err(e) => Err(e.ext_report(msg())),
        }
    }

    fn context<D>(self, msg: D) -> Result<T, Report>
    where
        D: Display + Send + Sync + 'static,
    {
        self.wrap_err(msg)
    }

    fn with_context<D, F>(self, msg: F) -> Result<T, Report>
    where
        D: Display + Send + Sync + 'static,
        F: FnOnce() -> D,
    {
        self.wrap_err_with(msg)
    }
}

impl<D, E> Debug for ContextError<D, E>
where
    D: Display,
    E: Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Error")
            .field("msg", &Quoted(&self.msg))
            .field("source", &self.error)
            .finish()
    }
}

impl<D, E> Display for ContextError<D, E>
where
    D: Display,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.msg, f)
    }
}

impl<D, E> StdError for ContextError<D, E>
where
    D: Display,
    E: StdError + 'static,
{
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        Some(&self.error)
    }
}

impl<D> StdError for ContextError<D, Report>
where
    D: Display,
{
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        unsafe { Some(ErrorImpl::error(self.error.inner.by_ref())) }
    }
}

impl<D, E> Diagnostic for ContextError<D, E>
where
    D: Display,
    E: Diagnostic + 'static,
{
    fn code(&self) -> Option<Cow<'_, str>> {
        self.error.code()
    }

    fn severity(&self) -> Option<crate::Severity> {
        self.error.severity()
    }

    fn help(&self) -> Option<Cow<'_, str>> {
        self.error.help()
    }

    fn url(&self) -> Option<Cow<'_, str>> {
        self.error.url()
    }

    fn labels(&self) -> crate::Labels {
        self.error.labels()
    }

    fn source_code(&self) -> Option<&dyn crate::SourceCode> {
        self.error.source_code()
    }

    fn related(&self) -> crate::Related<'_> {
        self.error.related()
    }
}

impl<D> Diagnostic for ContextError<D, Report>
where
    D: Display,
{
    fn code(&self) -> Option<Cow<'_, str>> {
        unsafe { ErrorImpl::diagnostic(self.error.inner.by_ref()).code() }
    }

    fn severity(&self) -> Option<crate::Severity> {
        unsafe { ErrorImpl::diagnostic(self.error.inner.by_ref()).severity() }
    }

    fn help(&self) -> Option<Cow<'_, str>> {
        unsafe { ErrorImpl::diagnostic(self.error.inner.by_ref()).help() }
    }

    fn url(&self) -> Option<Cow<'_, str>> {
        unsafe { ErrorImpl::diagnostic(self.error.inner.by_ref()).url() }
    }

    fn labels(&self) -> crate::Labels {
        unsafe { ErrorImpl::diagnostic(self.error.inner.by_ref()).labels() }
    }

    fn source_code(&self) -> Option<&dyn crate::SourceCode> {
        self.error.source_code()
    }

    fn related(&self) -> crate::Related<'_> {
        self.error.related()
    }
}

struct Quoted<D>(D);

impl<D> Debug for Quoted<D>
where
    D: Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_char('"')?;
        Quoted(&mut *formatter).write_fmt(format_args!("{}", self.0))?;
        formatter.write_char('"')?;
        Ok(())
    }
}

impl Write for Quoted<&mut fmt::Formatter<'_>> {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        Display::fmt(&s.escape_debug(), self.0)
    }
}

pub(crate) mod private {
    use super::*;

    pub trait Sealed {}

    impl<T, E> Sealed for Result<T, E> where E: ext::Diag {}
    impl<T> Sealed for Option<T> {}
}

#[repr(transparent)]
pub(crate) struct MessageError<M>(pub(crate) M);

impl<M> Debug for MessageError<M>
where
    M: Display + Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl<M> Display for MessageError<M>
where
    M: Display + Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<M> StdError for MessageError<M> where M: Display + Debug + 'static {}
impl<M> Diagnostic for MessageError<M> where M: Display + Debug + 'static {}

#[repr(transparent)]
pub(crate) struct BoxedError(pub(crate) Box<dyn Diagnostic + Send + Sync>);

impl Diagnostic for BoxedError {
    fn code(&self) -> Option<Cow<'_, str>> {
        self.0.code()
    }

    fn severity(&self) -> Option<miette::Severity> {
        self.0.severity()
    }

    fn help(&self) -> Option<Cow<'_, str>> {
        self.0.help()
    }

    fn note(&self) -> Option<Cow<'_, str>> {
        self.0.note()
    }

    fn url(&self) -> Option<Cow<'_, str>> {
        self.0.url()
    }

    fn labels(&self) -> crate::Labels {
        self.0.labels()
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.0.source_code()
    }

    fn related(&self) -> crate::Related<'_> {
        self.0.related()
    }

    fn diagnostic_source(&self) -> Option<&dyn Diagnostic> {
        self.0.diagnostic_source()
    }
}

impl Debug for BoxedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Display for BoxedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl StdError for BoxedError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.0.source()
    }

    fn description(&self) -> &str {
        #[allow(deprecated)]
        self.0.description()
    }

    fn cause(&self) -> Option<&dyn StdError> {
        #[allow(deprecated)]
        self.0.cause()
    }
}

pub(crate) struct WithSourceCode<E, C> {
    pub(crate) error: E,
    pub(crate) source_code: C,
}

impl<E: Diagnostic, C: SourceCode> Diagnostic for WithSourceCode<E, C> {
    fn code(&self) -> Option<Cow<'_, str>> {
        self.error.code()
    }

    fn severity(&self) -> Option<miette::Severity> {
        self.error.severity()
    }

    fn help(&self) -> Option<Cow<'_, str>> {
        self.error.help()
    }

    fn note(&self) -> Option<Cow<'_, str>> {
        self.error.note()
    }

    fn url(&self) -> Option<Cow<'_, str>> {
        self.error.url()
    }

    fn labels(&self) -> crate::Labels {
        self.error.labels()
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.error.source_code().or(Some(&self.source_code))
    }

    fn related(&self) -> crate::Related<'_> {
        self.error.related()
    }

    fn diagnostic_source(&self) -> Option<&dyn Diagnostic> {
        self.error.diagnostic_source()
    }
}

impl<C: SourceCode> Diagnostic for WithSourceCode<Report, C> {
    fn code(&self) -> Option<Cow<'_, str>> {
        self.error.code()
    }

    fn severity(&self) -> Option<miette::Severity> {
        self.error.severity()
    }

    fn help(&self) -> Option<Cow<'_, str>> {
        self.error.help()
    }

    fn note(&self) -> Option<Cow<'_, str>> {
        self.error.note()
    }

    fn url(&self) -> Option<Cow<'_, str>> {
        self.error.url()
    }

    fn labels(&self) -> crate::Labels {
        self.error.labels()
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        self.error.source_code().or(Some(&self.source_code))
    }

    fn related(&self) -> crate::Related<'_> {
        self.error.related()
    }

    fn diagnostic_source(&self) -> Option<&dyn Diagnostic> {
        self.error.diagnostic_source()
    }
}

impl<E: Debug, C> Debug for WithSourceCode<E, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Debug::fmt(&self.error, f)
    }
}

impl<E: Display, C> Display for WithSourceCode<E, C> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        Display::fmt(&self.error, f)
    }
}

impl<E: StdError, C> StdError for WithSourceCode<E, C> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.error.source()
    }
}

impl<C> StdError for WithSourceCode<Report, C> {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.error.source()
    }
}
