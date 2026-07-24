#![allow(
    missing_debug_implementations,
    missing_docs,
    clippy::new_ret_no_self,
    clippy::wrong_self_convention
)]
//! Autoderef-specialization dispatch for `miette!(expr)`: picks up the argument's
//! [`Diagnostic`] metadata when available, then an [`std::error::Error`]
//! source chain, and otherwise falls back to `Display + Debug`. Vendored from
//! [eyre](https://docs.rs/eyre) / anyhow; the comment below explains the
//! mechanism in detail.

// Tagged dispatch mechanism for resolving the behavior of `miette!($expr)`.
//
// When miette! is given a single expr argument to turn into miette::Report, we
// want the resulting Report to pick up the input's implementation of source()
// and backtrace() if it has a std::error::Error impl, otherwise require nothing
// more than Display and Debug.
//
// Expressed in terms of specialization, we want something like:
//
//     trait EyreNew {
//         fn new(self) -> Report;
//     }
//
//     impl<T> EyreNew for T
//     where
//         T: Display + Debug + Send + Sync + 'static,
//     {
//         default fn new(self) -> Report {
//             /* no std error impl */
//         }
//     }
//
//     impl<T> EyreNew for T
//     where
//         T: std::error::Error + Send + Sync + 'static,
//     {
//         fn new(self) -> Report {
//             /* use std error's source() and backtrace() */
//         }
//     }
//
// Since specialization is not stable yet, instead we rely on autoderef behavior
// of method resolution to perform tagged dispatch. TraitKind handles values
// that already convert into Report, StdErrorKind handles other standard errors,
// and AdhocKind is the fallback. The dispatch wrapper dereferences through these
// cases in priority order, so method resolution picks the first applicable one.
//
// The miette! macro will set up the call in this form:
//
//     #[allow(unused_imports)]
//     use $crate::private::kind::*;
//     let error = $msg;
//     dispatch(&error).miette_kind().new(error)

use core::{
    error::Error as StdError,
    fmt::{Debug, Display},
    ops::Deref,
};

use crate::Diagnostic;
use crate::Report;

pub struct Adhoc;

pub struct Dispatch<T>(StdErrorDispatch<T>);

pub struct StdErrorDispatch<T>(AdhocDispatch<T>);

pub struct AdhocDispatch<T>(T);

#[inline]
pub fn dispatch<T>(value: &T) -> Dispatch<&T> {
    Dispatch(StdErrorDispatch(AdhocDispatch(value)))
}

impl<T> Deref for Dispatch<T> {
    type Target = StdErrorDispatch<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> Deref for StdErrorDispatch<T> {
    type Target = AdhocDispatch<T>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub trait AdhocKind: Sized {
    #[inline]
    fn miette_kind(&self) -> Adhoc {
        Adhoc
    }
}

impl<T> AdhocKind for AdhocDispatch<&T> where T: Display + Debug + Send + Sync + 'static {}

impl Adhoc {
    #[track_caller]
    pub fn new<M>(self, message: M) -> Report
    where
        M: Display + Debug + Send + Sync + 'static,
    {
        Report::from_adhoc(message)
    }
}

pub struct Trait;

pub trait TraitKind: Sized {
    #[inline]
    fn miette_kind(&self) -> Trait {
        Trait
    }
}

impl<E> TraitKind for Dispatch<&E> where E: Into<Report> {}

impl Trait {
    #[track_caller]
    pub fn new<E>(self, error: E) -> Report
    where
        E: Into<Report>,
    {
        error.into()
    }
}

pub struct StdErrorTag;

pub trait StdErrorKind: Sized {
    #[inline]
    fn miette_kind(&self) -> StdErrorTag {
        StdErrorTag
    }
}

impl<E> StdErrorKind for StdErrorDispatch<&E> where E: StdError + Send + Sync + 'static {}

impl StdErrorTag {
    #[track_caller]
    pub fn new<E>(self, error: E) -> Report
    where
        E: StdError + Send + Sync + 'static,
    {
        Report::from_std_error(error)
    }
}

pub struct Boxed;

pub trait BoxedKind: Sized {
    #[inline]
    fn miette_kind(&self) -> Boxed {
        Boxed
    }
}

impl BoxedKind for Dispatch<&Box<dyn Diagnostic + Send + Sync>> {}

impl Boxed {
    #[track_caller]
    pub fn new(self, error: Box<dyn Diagnostic + Send + Sync>) -> Report {
        Report::from_boxed(error)
    }
}
