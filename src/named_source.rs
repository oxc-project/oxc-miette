use std::fmt;

use crate::{SourceCode, SpanContents};

/// Utility struct for when you have a regular [`SourceCode`] type that doesn't
/// implement `name`. For example [`String`]. Or if you want to override the
/// `name` returned by the `SourceCode`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NamedSource<S: SourceCode + 'static> {
    source: S,
    name: String,
}

impl<S: SourceCode> fmt::Debug for NamedSource<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NamedSource").field("name", &self.name).field("source", &"<redacted>");
        Ok(())
    }
}

impl<S: SourceCode + 'static> NamedSource<S> {
    /// Create a new `NamedSource` using a regular [`SourceCode`] and giving
    /// its returned [`SpanContents`] a name.
    #[must_use]
    pub fn new(name: impl AsRef<str>, source: S) -> Self
    where
        S: Send + Sync,
    {
        Self { source, name: name.as_ref().to_string() }
    }
}

impl<S: SourceCode + 'static> SourceCode for NamedSource<S> {
    fn read_span<'a>(
        &'a self,
        span: &crate::SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Option<SpanContents<'a>> {
        self.source.read_span(span, context_lines_before, context_lines_after)
    }

    fn name(&self) -> Option<&str> {
        Some(&self.name)
    }

    fn contiguous_bytes(&self) -> Option<&[u8]> {
        self.source.contiguous_bytes()
    }
}
