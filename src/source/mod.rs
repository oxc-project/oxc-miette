//! Source-code adapters and span reading.

use std::sync::Arc;

use crate::SourceCode;

pub use named::NamedSource;

mod named;
pub mod reader;

impl SourceCode for str {
    fn data(&self) -> &[u8] {
        self.as_bytes()
    }
}

/// Makes `src: &'static str` or `struct S<'a> { src: &'a str }` usable.
impl SourceCode for &str {
    fn data(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl SourceCode for String {
    fn data(&self) -> &[u8] {
        self.as_bytes()
    }
}

impl<T: ?Sized + SourceCode> SourceCode for Arc<T> {
    fn data(&self) -> &[u8] {
        self.as_ref().data()
    }

    fn name(&self) -> Option<&str> {
        self.as_ref().name()
    }
}
