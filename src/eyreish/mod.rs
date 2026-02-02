#![allow(clippy::needless_doctest_main, clippy::new_ret_no_self, clippy::wrong_self_convention)]
use std::{error::Error as StdError, sync::OnceLock};

#[doc(hidden)]
#[allow(unreachable_pub)]
pub use Report as ErrReport;
/// Compatibility re-export of `Report` for interop with `anyhow`
#[allow(unreachable_pub)]
pub use Report as Error;
#[doc(hidden)]
#[allow(unreachable_pub)]
pub use ReportHandler as EyreContext;
use error::ErrorImpl;

use self::ptr::Own;
use crate::Diagnostic;
#[cfg(feature = "fancy-base")]
use crate::MietteHandler;

mod error;
mod fmt;
mod ptr;
mod wrapper;

/**
Core Diagnostic wrapper type.

## `eyre` Users

You can just replace `use`s of `eyre::Report` with `miette::Report`.
*/
pub struct Report {
    inner: Own<ErrorImpl<()>>,
}

unsafe impl Sync for Report {}
unsafe impl Send for Report {}

/// `ErrorHook`
pub type ErrorHook =
    Box<dyn Fn(&(dyn Diagnostic + 'static)) -> Box<dyn ReportHandler> + Sync + Send + 'static>;

static HOOK: OnceLock<ErrorHook> = OnceLock::new();

/// Error indicating that [`set_hook()`] was unable to install the provided
/// [`ErrorHook`].
#[derive(Debug)]
pub struct InstallError;

impl core::fmt::Display for InstallError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("cannot install provided ErrorHook, a hook has already been installed")
    }
}

impl StdError for InstallError {}
impl Diagnostic for InstallError {}

/**
Set the error hook.
*/
pub fn set_hook(hook: ErrorHook) -> Result<(), InstallError> {
    HOOK.set(hook).map_err(|_| InstallError)
}

#[track_caller]
fn capture_handler(error: &(dyn Diagnostic + 'static)) -> Box<dyn ReportHandler> {
    let hook = HOOK.get_or_init(|| Box::new(get_default_printer)).as_ref();
    hook(error)
}

#[cfg(feature = "fancy-base")]
fn get_default_printer(_err: &(dyn Diagnostic + 'static)) -> Box<dyn ReportHandler + 'static> {
    Box::new(MietteHandler::new())
}

#[cfg(not(feature = "fancy-base"))]
fn get_default_printer(_err: &(dyn Diagnostic + 'static)) -> Box<dyn ReportHandler + 'static> {
    Box::new(DefaultReportHandler)
}

/// Minimal report handler used when `fancy-base` feature is not enabled.
#[cfg(not(feature = "fancy-base"))]
struct DefaultReportHandler;

#[cfg(not(feature = "fancy-base"))]
impl ReportHandler for DefaultReportHandler {
    fn debug(
        &self,
        diagnostic: &dyn Diagnostic,
        f: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        if f.alternate() {
            return core::fmt::Debug::fmt(diagnostic, f);
        }
        core::fmt::Display::fmt(diagnostic, f)
    }
}

impl dyn ReportHandler {
    /// `is`
    pub fn is<T: ReportHandler>(&self) -> bool {
        // Get `TypeId` of the type this function is instantiated with.
        let t = core::any::TypeId::of::<T>();

        // Get `TypeId` of the type in the trait object (`self`).
        let concrete = self.type_id();

        // Compare both `TypeId`s on equality.
        t == concrete
    }

    /// `downcast_ref`
    pub fn downcast_ref<T: ReportHandler>(&self) -> Option<&T> {
        if self.is::<T>() {
            unsafe { Some(&*(self as *const dyn ReportHandler as *const T)) }
        } else {
            None
        }
    }

    /// `downcast_mut`
    pub fn downcast_mut<T: ReportHandler>(&mut self) -> Option<&mut T> {
        if self.is::<T>() {
            unsafe { Some(&mut *(self as *mut dyn ReportHandler as *mut T)) }
        } else {
            None
        }
    }
}

/// Error Report Handler trait for customizing `miette::Report`
pub trait ReportHandler: core::any::Any + Send + Sync {
    /// Define the report format
    ///
    /// Used to override the report format of `miette::Report`
    ///
    /// # Example
    ///
    /// ```rust
    /// use indenter::indented;
    /// use miette::{Diagnostic, ReportHandler};
    ///
    /// pub struct Handler;
    ///
    /// impl ReportHandler for Handler {
    ///     fn debug(
    ///         &self,
    ///         error: &dyn Diagnostic,
    ///         f: &mut core::fmt::Formatter<'_>,
    ///     ) -> core::fmt::Result {
    ///         use core::fmt::Write as _;
    ///
    ///         if f.alternate() {
    ///             return core::fmt::Debug::fmt(error, f);
    ///         }
    ///
    ///         write!(f, "{}", error)?;
    ///
    ///         Ok(())
    ///     }
    /// }
    /// ```
    fn debug(&self, error: &dyn Diagnostic, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result;

    /// Override for the `Display` format
    fn display(
        &self,
        error: &(dyn StdError + 'static),
        f: &mut core::fmt::Formatter<'_>,
    ) -> core::fmt::Result {
        write!(f, "{error}")?;

        if f.alternate() {
            for cause in crate::chain::Chain::new(error).skip(1) {
                write!(f, ": {cause}")?;
            }
        }

        Ok(())
    }

    /// Store the location of the caller who constructed this error report
    #[allow(unused_variables)]
    fn track_caller(&mut self, location: &'static std::panic::Location<'static>) {}
}

/// type alias for `Result<T, Report>`
///
/// This is a reasonable return type to use throughout your application but also
/// for `main()`. If you do, failures will be printed along with a backtrace if
/// one was captured.
///
/// `miette::Result` may be used with one *or* two type parameters.
///
/// ```rust
/// use miette::Result;
///
/// # const IGNORE: &str = stringify! {
/// fn demo1() -> Result<T> {...}
///            // ^ equivalent to std::result::Result<T, miette::Error>
///
/// fn demo2() -> Result<T, OtherError> {...}
///            // ^ equivalent to std::result::Result<T, OtherError>
/// # };
/// ```
///
/// ## `anyhow`/`eyre` Users
///
/// You can just replace `use`s of `anyhow::Result`/`eyre::Result` with
/// `miette::Result`.
pub type Result<T, E = Report> = core::result::Result<T, E>;
