//! The line model and its measurements.
//!
//! [`get_lines`](GraphicalReportHandler::get_lines) splits already-read span
//! contents into [`Line`]s. Each `Line` knows its position in the source so it
//! can answer geometry questions about a [`FancySpan`] — does the span start,
//! end, or fly by on this line — which drives gutter and underline rendering.
//!
//! The [`visual_offset`](GraphicalReportHandler::visual_offset) /
//! [`line_visual_char_width`](GraphicalReportHandler::line_visual_char_width)
//! helpers translate byte offsets into terminal columns, accounting for tabs,
//! ANSI escapes, and wide/combining Unicode graphemes.

use std::str::{CharIndices, from_utf8};

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::{handler::GraphicalReportHandler, span::FancySpan};
use crate::{MietteSpanContents, SpanContents};

#[derive(Debug)]
pub(super) struct Line<'a> {
    pub(super) line_number: usize,
    pub(super) offset: usize,
    pub(super) length: usize,
    pub(super) text: &'a str,
}

impl Line<'_> {
    pub(super) fn span_line_only(&self, span: &FancySpan) -> bool {
        span.offset() >= self.offset && span.offset() + span.len() <= self.offset + self.length
    }

    /// Returns whether `span` should be visible on this line, either in the gutter or under the
    /// text on this line
    pub(super) fn span_applies(&self, span: &FancySpan) -> bool {
        let spanlen = if span.len() == 0 { 1 } else { span.len() };
        // Span starts in this line

        (span.offset() >= self.offset && span.offset() < self.offset + self.length)
            // Span passes through this line
            || (span.offset() < self.offset && span.offset() + spanlen > self.offset + self.length) //todo
            // Span ends on this line
            || (span.offset() + spanlen > self.offset && span.offset() + spanlen <= self.offset + self.length)
    }

    /// Returns whether `span` should be visible on this line in the gutter (so this excludes spans
    /// that are only visible on this line and do not span multiple lines)
    pub(super) fn span_applies_gutter(&self, span: &FancySpan) -> bool {
        let spanlen = if span.len() == 0 { 1 } else { span.len() };
        // Span starts in this line
        self.span_applies(span)
            && !(
                // as long as it doesn't start *and* end on this line
                (span.offset() >= self.offset && span.offset() < self.offset + self.length)
                    && (span.offset() + spanlen > self.offset
                        && span.offset() + spanlen <= self.offset + self.length)
            )
    }

    // A 'flyby' is a multi-line span that technically covers this line, but
    // does not begin or end within the line itself. This method is used to
    // calculate gutters.
    pub(super) fn span_flyby(&self, span: &FancySpan) -> bool {
        // The span itself starts before this line's starting offset (so, in a
        // prev line).
        span.offset() < self.offset
            // ...and it stops after this line's end.
            && span.offset() + span.len() > self.offset + self.length
    }

    // Does this line contain the *beginning* of this multiline span?
    // This assumes self.span_applies() is true already.
    pub(super) fn span_starts(&self, span: &FancySpan) -> bool {
        span.offset() >= self.offset
    }

    // Does this line contain the *end* of this multiline span?
    // This assumes self.span_applies() is true already.
    pub(super) fn span_ends(&self, span: &FancySpan) -> bool {
        span.offset() + span.len() >= self.offset
            && span.offset() + span.len() <= self.offset + self.length
    }
}

/// Iterator over the visual (terminal-column) width of each `char` in a line.
///
/// ASCII text takes a fast path where every printable char is width 1. For
/// non-ASCII text we pre-compute grapheme boundaries and only charge the
/// grapheme's width to its first `char`, so combining marks contribute 0.
/// ANSI escape sequences (`\x1b … m`) are consumed at zero width.
struct CharWidthIterator<'a> {
    chars: CharIndices<'a>,
    grapheme_boundaries: Option<Vec<(usize, usize)>>, // (byte_pos, width) - None for ASCII
    current_grapheme_idx: usize,
    column: usize,
    escaped: bool,
    tab_width: usize,
}

impl Iterator for CharWidthIterator<'_> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let (byte_pos, c) = self.chars.next()?;

        let width = match (self.escaped, c) {
            (false, '\t') => self.tab_width - self.column % self.tab_width,
            (false, '\x1b') => {
                self.escaped = true;
                0
            }
            (false, _) => {
                if let Some(ref boundaries) = self.grapheme_boundaries {
                    // Unicode path: check if we're at a grapheme boundary
                    if self.current_grapheme_idx < boundaries.len()
                        && boundaries[self.current_grapheme_idx].0 == byte_pos
                    {
                        let width = boundaries[self.current_grapheme_idx].1;
                        self.current_grapheme_idx += 1;
                        width
                    } else {
                        0 // Not at a grapheme boundary
                    }
                } else {
                    // ASCII path: all non-control chars are width 1
                    1
                }
            }
            (true, 'm') => {
                self.escaped = false;
                0
            }
            (true, _) => 0,
        };

        self.column += width;
        Some(width)
    }
}

impl GraphicalReportHandler {
    /// Returns an iterator over the visual width of each character in a line.
    pub(super) fn line_visual_char_width<'a>(
        &self,
        text: &'a str,
    ) -> impl Iterator<Item = usize> + 'a + use<'a> {
        // Only compute grapheme boundaries for non-ASCII text
        let grapheme_boundaries = if text.is_ascii() {
            None
        } else {
            // Collect grapheme boundaries with their widths
            Some(
                text.grapheme_indices(true)
                    .map(|(pos, grapheme)| (pos, grapheme.width()))
                    .collect(),
            )
        };

        CharWidthIterator {
            chars: text.char_indices(),
            grapheme_boundaries,
            current_grapheme_idx: 0,
            column: 0,
            escaped: false,
            tab_width: self.tab_width,
        }
    }

    /// Returns the visual column position of a byte offset on a specific line.
    ///
    /// If the offset occurs in the middle of a character, the returned column
    /// corresponds to that character's first column in `start` is true, or its
    /// last column if `start` is false.
    pub(super) fn visual_offset(&self, line: &Line<'_>, offset: usize, start: bool) -> usize {
        let line_range = line.offset..=(line.offset + line.length);
        assert!(line_range.contains(&offset));

        let mut text_index = offset - line.offset;
        while text_index <= line.text.len() && !line.text.is_char_boundary(text_index) {
            if start {
                text_index -= 1;
            } else {
                text_index += 1;
            }
        }
        let text = &line.text[..text_index.min(line.text.len())];
        // Plain ASCII is exactly one terminal column per byte.
        let text_width =
            if text.is_ascii() && memchr::memchr2(b'\t', b'\x1b', text.as_bytes()).is_none() {
                text.len()
            } else {
                self.line_visual_char_width(text).sum()
            };
        if text_index > line.text.len() {
            // Spans extending past the end of the line are always rendered as
            // one column past the end of the visible line.
            //
            // This doesn't necessarily correspond to a specific byte-offset,
            // since a span extending past the end of the line could contain:
            //  - an actual \n character (1 byte)
            //  - a CRLF (2 bytes)
            //  - EOF (0 bytes)
            text_width + 1
        } else {
            text_width
        }
    }

    /// Splits already-read span contents into [`Line`]s. Takes the contents
    /// produced by the `read_span` call in
    /// [`render_snippets`](GraphicalReportHandler::render_snippets) so the span
    /// doesn't have to be re-read (each read is a scan of the source up to the
    /// span).
    pub(super) fn get_lines<'a>(&self, context_data: &MietteSpanContents<'a>) -> Vec<Line<'a>> {
        let context = from_utf8(context_data.data()).expect("Bad utf8 detected");
        let mut line = context_data.line();
        let mut column = context_data.column();
        // Byte offset into the original source.
        let mut offset = context_data.span().offset() as usize;
        // Byte offset of `context[0]` into the original source, used to map a
        // source offset to an index into `context`.
        let base = offset;
        let mut line_offset = offset;
        // Number of bytes of visible text accumulated for the current line
        // (i.e. excluding the line terminator).
        let mut line_len = 0usize;
        let mut lines = Vec::with_capacity(1);
        let mut iter = context.chars().peekable();
        while let Some(char) = iter.next() {
            offset += char.len_utf8();
            let mut at_end_of_file = false;
            match char {
                '\r' => {
                    if iter.next_if_eq(&'\n').is_some() {
                        offset += 1;
                        line += 1;
                        column = 0;
                    } else {
                        line_len += char.len_utf8();
                        column += 1;
                    }
                    at_end_of_file = iter.peek().is_none();
                }
                '\n' => {
                    at_end_of_file = iter.peek().is_none();
                    line += 1;
                    column = 0;
                }
                _ => {
                    line_len += char.len_utf8();
                    column += 1;
                }
            }

            if iter.peek().is_none() && !at_end_of_file {
                line += 1;
            }

            if column == 0 || iter.peek().is_none() {
                // The visible text is a contiguous slice of `context`, starting
                // at the line's offset and excluding the line terminator.
                let text_start = line_offset - base;
                lines.push(Line {
                    line_number: line,
                    offset: line_offset,
                    length: offset - line_offset,
                    text: &context[text_start..text_start + line_len],
                });
                line_len = 0;
                line_offset = offset;
            }
        }
        lines
    }
}
