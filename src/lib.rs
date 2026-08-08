//! Diagnostic protocols and renderers used by Oxc.
//!
//! This crate defines the [`Diagnostic`] and [`SourceCode`] protocols together
//! with graphical and JSON renderers. It intentionally does not provide an
//! application error container: callers own diagnostics directly or through
//! boxed trait objects and choose a renderer explicitly.

pub use handlers::*;
pub use named_source::*;
pub use protocol::*;

mod handlers;
mod named_source;
mod protocol;
mod source_impls;
