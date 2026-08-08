/*!
This module defines the core of the miette protocol: a series of types and
traits that you can implement to get access to miette's (and related library's)
full reporting and such features.
*/
use std::{
    borrow::Cow,
    error::Error,
    mem,
    ops::{Deref, DerefMut, Range},
    slice::{Iter, IterMut},
};

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
    /// Returns the owned [`Labels`] container. For the common one/two-label
    /// case this is allocation-free (the labels are stored inline), and it
    /// avoids the boxed-iterator allocation the previous signature required.
    fn labels(&self) -> crate::Labels {
        crate::Labels::None
    }
}

impl Error for Box<dyn Diagnostic + Send + Sync> {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        (**self).source()
    }

    fn cause(&self) -> Option<&dyn Error> {
        self.source()
    }
}

impl<T: Diagnostic + Send + Sync + 'static> From<T>
    for Box<dyn Diagnostic + Send + Sync + 'static>
{
    fn from(diagnostic: T) -> Self {
        Box::new(diagnostic)
    }
}

/// Owned labels attached to a [`Diagnostic`].
///
/// The common one- and two-label cases are stored inline.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Labels {
    /// No labels.
    #[default]
    None,
    /// A single label.
    One([LabeledSpan; 1]),
    /// Two labels.
    Two([LabeledSpan; 2]),
    /// Three or more labels.
    Many(Vec<LabeledSpan>),
}

impl Labels {
    /// Returns the labels as a slice.
    pub fn as_slice(&self) -> &[LabeledSpan] {
        match self {
            Self::None => &[],
            Self::One(labels) => labels,
            Self::Two(labels) => labels,
            Self::Many(labels) => labels,
        }
    }

    /// Returns the labels as a mutable slice.
    pub fn as_mut_slice(&mut self) -> &mut [LabeledSpan] {
        match self {
            Self::None => &mut [],
            Self::One(labels) => labels,
            Self::Two(labels) => labels,
            Self::Many(labels) => labels,
        }
    }

    /// Returns whether there are no labels.
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::None)
    }

    /// Returns the number of labels.
    pub fn len(&self) -> usize {
        self.as_slice().len()
    }

    /// Appends a label.
    pub fn push(&mut self, label: LabeledSpan) {
        if let Self::Many(labels) = self {
            labels.push(label);
            return;
        }
        *self = match mem::take(self) {
            Self::None => Self::One([label]),
            Self::One([a]) => Self::Two([a, label]),
            Self::Two([a, b]) => Self::Many(vec![a, b, label]),
            Self::Many(_) => unreachable!("handled above"),
        };
    }
}

impl Deref for Labels {
    type Target = [LabeledSpan];

    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl DerefMut for Labels {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.as_mut_slice()
    }
}

impl<'a> IntoIterator for &'a Labels {
    type Item = &'a LabeledSpan;
    type IntoIter = Iter<'a, LabeledSpan>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

impl<'a> IntoIterator for &'a mut Labels {
    type Item = &'a mut LabeledSpan;
    type IntoIter = IterMut<'a, LabeledSpan>;

    fn into_iter(self) -> Self::IntoIter {
        self.as_mut_slice().iter_mut()
    }
}

impl Extend<LabeledSpan> for Labels {
    fn extend<I: IntoIterator<Item = LabeledSpan>>(&mut self, iter: I) {
        let mut iter = iter.into_iter();
        while !matches!(self, Self::Many(_)) {
            let Some(label) = iter.next() else { return };
            self.push(label);
        }
        if let Self::Many(labels) = self {
            labels.reserve(iter.size_hint().0);
            labels.extend(iter);
        }
    }
}

impl FromIterator<LabeledSpan> for Labels {
    fn from_iter<I: IntoIterator<Item = LabeledSpan>>(iter: I) -> Self {
        let mut iter = iter.into_iter();
        if iter.size_hint().0 > 2 {
            return Self::Many(iter.collect());
        }
        let Some(a) = iter.next() else { return Self::None };
        let Some(b) = iter.next() else { return Self::One([a]) };
        let Some(c) = iter.next() else { return Self::Two([a, b]) };
        let mut labels = Vec::with_capacity(3 + iter.size_hint().0);
        labels.extend([a, b, c]);
        labels.extend(iter);
        Self::Many(labels)
    }
}

impl From<Vec<LabeledSpan>> for Labels {
    fn from(labels: Vec<LabeledSpan>) -> Self {
        if labels.len() <= 2 { labels.into_iter().collect() } else { Self::Many(labels) }
    }
}

impl From<LabeledSpan> for Labels {
    fn from(label: LabeledSpan) -> Self {
        Self::One([label])
    }
}

impl From<[LabeledSpan; 1]> for Labels {
    fn from(labels: [LabeledSpan; 1]) -> Self {
        Self::One(labels)
    }
}

impl From<[LabeledSpan; 2]> for Labels {
    fn from(labels: [LabeledSpan; 2]) -> Self {
        Self::Two(labels)
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
    pub const fn new(label: Option<String>, offset: ByteOffset, len: u32) -> Self {
        Self { label, span: SourceSpan::new(SourceOffset(offset), len), primary: false }
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
    pub fn set_span_offset(&mut self, offset: ByteOffset) {
        self.span.offset = SourceOffset(offset);
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
    pub const fn offset(&self) -> ByteOffset {
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
    // Optional filename
    name: Option<Cow<'a, str>>,
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
        Self { data, span, line, column, line_count, name: None }
    }

    /// Make a new [`SpanContents`] object, with a name for its file.
    #[must_use]
    pub const fn new_named(
        name: Cow<'a, str>,
        data: &'a [u8],
        span: SourceSpan,
        line: usize,
        column: usize,
        line_count: usize,
    ) -> Self {
        Self { data, span, line, column, line_count, name: Some(name) }
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

    /// The source name, if one is available.
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }
}

/// Span within a [`SourceCode`]
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SourceSpan {
    /// The start of the span.
    offset: SourceOffset,
    /// The total length of the span, in bytes.
    length: u32,
}

impl SourceSpan {
    /// Create a new [`SourceSpan`].
    #[must_use]
    pub const fn new(start: SourceOffset, length: u32) -> Self {
        Self { offset: start, length }
    }

    /// The absolute offset, in bytes, from the beginning of a [`SourceCode`].
    #[must_use]
    #[expect(clippy::trivially_copy_pass_by_ref, reason = "retained for public API compatibility")]
    pub const fn offset(&self) -> ByteOffset {
        self.offset.offset()
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

impl From<(ByteOffset, u32)> for SourceSpan {
    fn from((start, len): (ByteOffset, u32)) -> Self {
        Self { offset: start.into(), length: len }
    }
}

impl From<(SourceOffset, u32)> for SourceSpan {
    fn from((start, len): (SourceOffset, u32)) -> Self {
        Self::new(start, len)
    }
}

impl From<Range<ByteOffset>> for SourceSpan {
    fn from(range: Range<ByteOffset>) -> Self {
        // `Range::len` returns `0` for empty/reversed ranges, matching the
        // previous behavior and avoiding underflow.
        let length = u32::try_from(range.len()).unwrap_or(u32::MAX);
        Self { offset: range.start.into(), length }
    }
}

impl From<SourceOffset> for SourceSpan {
    fn from(offset: SourceOffset) -> Self {
        Self { offset, length: 0 }
    }
}

impl From<ByteOffset> for SourceSpan {
    fn from(offset: ByteOffset) -> Self {
        Self { offset: offset.into(), length: 0 }
    }
}

/**
"Raw" type for the byte offset from the beginning of a [`SourceCode`].
*/
pub type ByteOffset = u32;

/**
Newtype that represents the [`ByteOffset`] from the beginning of a [`SourceCode`]
*/
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct SourceOffset(ByteOffset);

impl SourceOffset {
    /// Actual byte offset.
    #[must_use]
    #[expect(clippy::trivially_copy_pass_by_ref, reason = "retained for public API compatibility")]
    pub const fn offset(&self) -> ByteOffset {
        self.0
    }
}

impl From<ByteOffset> for SourceOffset {
    fn from(bytes: ByteOffset) -> Self {
        SourceOffset(bytes)
    }
}
