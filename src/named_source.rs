use std::{borrow::Cow, fmt};

use crate::{MietteError, MietteSpanContents, SourceCode, SpanContents};

/// Utility struct for when you have a regular [`SourceCode`] type that doesn't
/// implement `name`. For example [`String`]. Or if you want to override the
/// `name` returned by the `SourceCode`.
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NamedSource<S: SourceCode + 'static> {
    source: S,
    name: String,
    language: Option<String>,
}

impl<S: SourceCode> fmt::Debug for NamedSource<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("NamedSource")
            .field("name", &self.name)
            .field("source", &"<redacted>")
            .field("language", &self.language)
            .finish()
    }
}

impl<S: SourceCode + 'static> NamedSource<S> {
    /// Create a new `NamedSource` using a regular [`SourceCode`] and giving
    /// its returned [`SpanContents`] a name.
    pub fn new(name: impl AsRef<str>, source: S) -> Self
    where
        S: Send + Sync,
    {
        Self { source, name: name.as_ref().to_string(), language: None }
    }

    /// Gets the name of this `NamedSource`.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a reference the inner [`SourceCode`] type for this
    /// `NamedSource`.
    pub fn inner(&self) -> &S {
        &self.source
    }

    /// Sets the [`language`](SpanContents::language) for this source code.
    #[must_use]
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }
}

impl<S: SourceCode + 'static> SourceCode for NamedSource<S> {
    fn read_span<'a>(
        &'a self,
        span: &crate::SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Result<MietteSpanContents<'a>, MietteError> {
        let inner_contents =
            self.inner().read_span(span, context_lines_before, context_lines_after)?;
        let mut contents = MietteSpanContents::new_named(
            Cow::Borrowed(self.name.as_str()),
            inner_contents.data(),
            *inner_contents.span(),
            inner_contents.line(),
            inner_contents.column(),
            inner_contents.line_count(),
        );
        if let Some(language) = &self.language {
            contents = contents.with_language(language);
        }
        Ok(contents)
    }

    fn name(&self) -> Option<&str> {
        Some(&self.name)
    }

    fn contiguous_bytes(&self) -> Option<&[u8]> {
        self.inner().contiguous_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::NamedSource;

    #[test]
    fn debug_redacts_source_and_finishes_the_struct() {
        let source = NamedSource::new("input.rs", "secret").with_language("Rust");

        assert_eq!(
            format!("{source:?}"),
            r#"NamedSource { name: "input.rs", source: "<redacted>", language: Some("Rust") }"#
        );
    }
}
