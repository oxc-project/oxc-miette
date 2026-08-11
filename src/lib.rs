//! Diagnostic protocols and renderers used by Oxc.
//!
//! This crate defines the [`Diagnostic`] and [`SourceCode`] protocols together
//! with graphical and JSON renderers. It intentionally does not provide an
//! application error container: callers own diagnostics directly or through
//! boxed trait objects and choose a renderer explicitly.

pub use protocol::*;
pub use renderers::*;
pub use source::NamedSource;

mod protocol;
mod renderers;
mod source;
