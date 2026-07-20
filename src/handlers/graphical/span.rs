//! The span model used while drawing a snippet.
//!
//! A [`FancySpan`] is one of the diagnostic's labels paired with the [`Style`]
//! it will be drawn in. Its label text is pre-split into display lines up front
//! so the drawing code can lay out multi-line labels without re-splitting.

use owo_colors::{OwoColorize, Style};

use crate::SourceSpan;

/// How a label is being drawn on the current output line.
///
/// A label can be a single trailing line, or a block that spans several output
/// lines (the first line drawn differently from the rest).
#[derive(PartialEq, Debug)]
pub(super) enum LabelRenderMode {
    /// we're rendering a single line label (or not rendering in any special way)
    SingleLine,
    /// we're rendering a multiline label
    BlockFirst,
    /// we're rendering the rest of a multiline label
    BlockRest,
}

#[derive(Debug, Clone)]
pub(super) struct FancySpan {
    /// this is deliberately an option of a vec because I wanted to be very explicit
    /// that there can also be *no* label. If there is a label, it can have multiple
    /// lines which is what the vec is for.
    label: Option<Vec<String>>,
    span: SourceSpan,
    pub(super) style: Style,
}

impl PartialEq for FancySpan {
    fn eq(&self, other: &Self) -> bool {
        self.label == other.label && self.span == other.span
    }
}

fn split_label(v: &str, style: Style) -> Vec<String> {
    v.split('\n').map(|i| i.style(style).to_string()).collect()
}

impl FancySpan {
    pub(super) fn new(label: Option<&str>, span: SourceSpan, style: Style) -> Self {
        FancySpan { label: label.map(|l| split_label(l, style)), span, style }
    }

    pub(super) fn has_label(&self) -> bool {
        self.label.is_some()
    }

    pub(super) fn label_parts(&self) -> Option<&[String]> {
        self.label.as_deref()
    }

    pub(super) fn offset(&self) -> usize {
        self.span.offset() as usize
    }

    pub(super) fn len(&self) -> usize {
        self.span.len() as usize
    }
}
