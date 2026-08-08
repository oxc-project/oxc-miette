/*!
This module defines the core of the miette protocol: a series of types and
traits that you can implement to get access to miette's (and related library's)
full reporting and such features.
*/
use std::{borrow::Cow, error::Error, ops::Range};

/// Adds rich metadata to your Error that can be used by
/// Rich metadata that renderers use to produce human-friendly error messages.
pub trait Diagnostic: Error {
    /// Unique diagnostic code that can be used to look up more information
    /// about this `Diagnostic`. Ideally also globally unique, and documented
    /// in the toplevel crate's documentation for easy searching. Rust path
    /// format (`foo::bar::baz`) is recommended, but more classic codes like
    /// `E0123` or enums will work just fine.
    fn code(&self) -> Option<Cow<'_, str>> {
        None
    }

    /// Diagnostic severity. This may be used by
    /// Renderers may use this to change the display format of this diagnostic.
    ///
    /// If `None`, reporters should treat this as [`Severity::Error`].
    fn severity(&self) -> Option<Severity> {
        None
    }

    /// Additional help text related to this `Diagnostic`. Do you have any
    /// advice for the poor soul who's just run into this issue?
    fn help(&self) -> Option<Cow<'_, str>> {
        None
    }

    /// Supplementary context for this `Diagnostic`, separate from help text.
    /// Notes mirror rustc-style `= note:` lines and offer additional
    /// information when guidance (help) is insufficient.
    fn note(&self) -> Option<Cow<'_, str>> {
        None
    }

    /// URL to visit for a more detailed explanation/help about this
    /// `Diagnostic`.
    fn url(&self) -> Option<Cow<'_, str>> {
        None
    }

    /// Source code to apply this `Diagnostic`'s [`Diagnostic::labels`] to.
    fn source_code(&self) -> Option<&dyn SourceCode> {
        None
    }

    /// Labels to apply to this `Diagnostic`'s [`Diagnostic::source_code`]
    ///
    /// The diagnostic retains ownership of the labels; renderers only borrow
    /// them for the duration of a report.
    fn labels(&self) -> &[LabeledSpan] {
        &[]
    }
}

/**
[`Diagnostic`] severity. Renderers use this to change the way diagnostics are
displayed. Defaults to [`Severity::Error`].
*/
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd, Default)]
pub enum Severity {
    /// Just some help. Here's how you could be doing it better.
    Advice,
    /// Warning. Please take note.
    Warning,
    /// Critical failure. The program cannot continue.
    /// This is the default severity, if you don't specify another one.
    #[default]
    Error,
}

/**
Represents readable source code of some sort.

This trait is able to support simple `SourceCode` types like [`String`]s, as
well as more involved types like indexes into centralized `SourceMap`-like
types, file handles, and even network streams.

If you can read it, you can source it, and it's not necessary to read the
whole thing--meaning you should be able to support `SourceCode`s which are
gigabytes or larger in size.
*/
pub trait SourceCode: Send + Sync {
    /// Read the bytes for a specific span from this `SourceCode`, keeping a
    /// certain number of lines before and after the span as context.
    ///
    /// Returns [`None`] when the requested span cannot be read.
    #[expect(
        clippy::trivially_copy_pass_by_ref,
        reason = "retained for public trait compatibility"
    )]
    fn read_span<'a>(
        &'a self,
        span: &SourceSpan,
        context_lines_before: usize,
        context_lines_after: usize,
    ) -> Option<SpanContents<'a>>;

    /// Returns the name of this source code, if any.
    fn name(&self) -> Option<&str> {
        None
    }

    /// Returns the entire source as one contiguous byte buffer, if it is
    /// backed by one.
    ///
    /// This is an optional fast path: renderers use it to locate several spans
    /// in a single scan of the source instead of issuing one [`read_span`]
    /// (a scan from byte 0) per span. Implementations that return `Some` must
    /// return the same bytes [`read_span`] reads — same content, same offsets
    /// — and report the source's name via [`name`], since renderers taking
    /// this path derive span contents from the buffer without calling
    /// [`read_span`].
    ///
    /// The default returns `None`, which keeps every read going through
    /// [`read_span`].
    ///
    /// [`read_span`]: SourceCode::read_span
    /// [`name`]: SourceCode::name
    fn contiguous_bytes(&self) -> Option<&[u8]> {
        None
    }
}

/// A labeled [`SourceSpan`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabeledSpan {
    label: Option<String>,
    span: SourceSpan,
    primary: bool,
}

impl LabeledSpan {
    /// Makes a new labeled span.
    #[must_use]
    pub const fn new(label: Option<String>, offset: u32, len: u32) -> Self {
        Self { label, span: SourceSpan { offset, length: len }, primary: false }
    }

    /// Makes a new labeled span using an existing span.
    #[must_use]
    pub fn new_with_span(label: Option<String>, span: impl Into<SourceSpan>) -> Self {
        Self { label, span: span.into(), primary: false }
    }

    /// Makes a new labeled primary span using an existing span.
    #[must_use]
    pub fn new_primary_with_span(label: Option<String>, span: impl Into<SourceSpan>) -> Self {
        Self { label, span: span.into(), primary: true }
    }

    /// Change the offset of the span.
    pub fn set_span_offset(&mut self, offset: u32) {
        self.span.offset = offset;
    }

    /// Makes a new label at specified span
    ///
    /// # Examples
    /// ```
    /// use miette::LabeledSpan;
    ///
    /// let source = "Cpp is the best";
    /// let label = LabeledSpan::at(0..3, "should be Rust");
    /// assert_eq!(
    ///     label,
    ///     LabeledSpan::new(Some("should be Rust".to_string()), 0, 3)
    /// )
    /// ```
    #[must_use]
    pub fn at(span: impl Into<SourceSpan>, label: impl Into<String>) -> Self {
        Self::new_with_span(Some(label.into()), span)
    }

    /// Makes a new label without text, that underlines a specific span.
    ///
    /// # Examples
    /// ```
    /// use miette::LabeledSpan;
    ///
    /// let source = "You have an error here";
    /// let label = LabeledSpan::underline(12..16);
    /// assert_eq!(label, LabeledSpan::new(None, 12, 4))
    /// ```
    #[must_use]
    pub fn underline(span: impl Into<SourceSpan>) -> Self {
        Self::new_with_span(None, span)
    }

    /// Gets the (optional) label string for this `LabeledSpan`.
    #[must_use]
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Returns a reference to the inner [`SourceSpan`].
    #[must_use]
    pub const fn inner(&self) -> &SourceSpan {
        &self.span
    }

    /// Returns the 0-based starting byte offset.
    #[must_use]
    pub const fn offset(&self) -> u32 {
        self.span.offset()
    }

    /// Returns the number of bytes this `LabeledSpan` spans.
    #[must_use]
    pub const fn len(&self) -> u32 {
        self.span.len()
    }

    /// True if this `LabeledSpan` is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.span.is_empty()
    }

    /// True if this `LabeledSpan` is a primary span.
    #[must_use]
    pub const fn primary(&self) -> bool {
        self.primary
    }
}

/// Contents of a [`SourceCode`] covered by a [`SourceSpan`].
///
/// Includes line and column information used by renderers.
#[derive(Clone, Debug)]
pub struct SpanContents<'a> {
    // Data from a [`SourceCode`], in bytes.
    data: &'a [u8],
    // span actually covered by this SpanContents.
    span: SourceSpan,
    // The 0-indexed line where the associated [`SourceSpan`] _starts_.
    line: usize,
    // The 0-indexed column where the associated [`SourceSpan`] _starts_.
    column: usize,
    // Number of line in this snippet.
    line_count: usize,
}

impl<'a> SpanContents<'a> {
    /// Make a new [`SpanContents`] object.
    #[must_use]
    pub const fn new(
        data: &'a [u8],
        span: SourceSpan,
        line: usize,
        column: usize,
        line_count: usize,
    ) -> Self {
        Self { data, span, line, column, line_count }
    }

    /// Reference to the covered source data, in bytes.
    pub const fn data(&self) -> &'a [u8] {
        self.data
    }

    /// The span covered by this payload.
    pub const fn span(&self) -> &SourceSpan {
        &self.span
    }

    /// The 0-indexed line where the payload begins.
    pub const fn line(&self) -> usize {
        self.line
    }

    /// The 0-indexed column where the payload begins.
    pub const fn column(&self) -> usize {
        self.column
    }

    /// Total number of lines covered by this payload.
    pub const fn line_count(&self) -> usize {
        self.line_count
    }
}

/// Span within a [`SourceCode`]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SourceSpan {
    /// The start of the span.
    offset: u32,
    /// The total length of the span, in bytes.
    length: u32,
}

impl SourceSpan {
    /// The absolute offset, in bytes, from the beginning of a [`SourceCode`].
    #[must_use]
    #[expect(clippy::trivially_copy_pass_by_ref, reason = "retained for public API compatibility")]
    pub const fn offset(&self) -> u32 {
        self.offset
    }

    /// Total length of the [`SourceSpan`], in bytes.
    #[must_use]
    #[expect(clippy::trivially_copy_pass_by_ref, reason = "retained for public API compatibility")]
    pub const fn len(&self) -> u32 {
        self.length
    }

    /// Whether this [`SourceSpan`] has a length of zero. It may still be useful
    /// to point to a specific point.
    #[must_use]
    #[expect(clippy::trivially_copy_pass_by_ref, reason = "retained for public API compatibility")]
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }
}

impl From<(u32, u32)> for SourceSpan {
    fn from((start, len): (u32, u32)) -> Self {
        Self { offset: start, length: len }
    }
}

impl From<Range<u32>> for SourceSpan {
    fn from(range: Range<u32>) -> Self {
        // `Range::len` returns `0` for empty/reversed ranges, matching the
        // previous behavior and avoiding underflow.
        let length = u32::try_from(range.len()).unwrap_or(u32::MAX);
        Self { offset: range.start, length }
    }
}
