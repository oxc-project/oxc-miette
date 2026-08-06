//! Source-snippet layout.
//!
//! [`render_snippets`](GraphicalReportHandler::render_snippets) reads every
//! label's span (sharing a single forward scan when the source exposes its
//! backing buffer) and merges overlapping spans into contexts.
//! [`render_context`](GraphicalReportHandler::render_context) then draws one
//! context: the `[file:line:col]` header, each source line (via
//! [`render_line_text`](GraphicalReportHandler::render_line_text)), and the
//! gutters/underlines/labels delegated to the sibling modules.

use std::{borrow::Cow, cmp::max, fmt};

use owo_colors::OwoColorize;
use rustc_hash::FxHashMap;

use super::{
    handler::GraphicalReportHandler,
    label::write_repeated_char,
    span::{FancySpan, LabelRenderMode},
};
use crate::{
    Diagnostic, LabeledSpan, MietteSpanContents, SourceCode, SourceSpan, SpanContents,
    source_impls::SpanScanner,
};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct SourceKey {
    pointer: *const u8,
    length: usize,
}

impl SourceKey {
    fn new(bytes: &[u8]) -> Self {
        Self { pointer: bytes.as_ptr(), length: bytes.len() }
    }
}

pub(super) struct SpanScannerCache<'a> {
    context_lines: usize,
    scanners: FxHashMap<SourceKey, SpanScanner<'a>>,
}

impl<'a> SpanScannerCache<'a> {
    pub(super) fn new(context_lines: usize) -> Self {
        Self { context_lines, scanners: FxHashMap::default() }
    }

    fn scanner(&mut self, bytes: &'a [u8]) -> &mut SpanScanner<'a> {
        self.scanners.entry(SourceKey::new(bytes)).or_insert_with(|| {
            SpanScanner::new_batch(bytes, self.context_lines, self.context_lines)
        })
    }
}

impl GraphicalReportHandler {
    pub(super) fn render_snippets<'a>(
        &self,
        f: &mut impl fmt::Write,
        diagnostic: &'a dyn Diagnostic,
        opt_source: Option<&'a dyn SourceCode>,
        scanner_cache: Option<&mut SpanScannerCache<'a>>,
    ) -> fmt::Result {
        let Some(source) = opt_source else { return Ok(()) };
        let mut labels = diagnostic.labels();
        if labels.is_empty() {
            return Ok(());
        }
        labels.sort_unstable_by_key(|l| l.inner().offset());

        let source_name = source.name();
        if let Some(bytes) = source.contiguous_bytes() {
            if let Some(scanner_cache) = scanner_cache {
                let scanner = scanner_cache.scanner(bytes);
                return self.render_snippets_with(f, &labels, |span| {
                    scanner
                        .read_span(*span)
                        .map(|contents| Self::attach_source_name(contents, source_name))
                });
            }

            // A single report still shares one forward scan across all of its
            // labels and merge attempts, without retaining a batch cache.
            let mut scanner = SpanScanner::new(bytes, self.context_lines, self.context_lines);
            return self.render_snippets_with(f, &labels, |span| {
                scanner
                    .read_span(*span)
                    .map(|contents| Self::attach_source_name(contents, source_name))
            });
        }

        self.render_snippets_with(f, &labels, |span| {
            source.read_span(span, self.context_lines, self.context_lines)
        })
    }

    fn attach_source_name<'a>(
        contents: MietteSpanContents<'a>,
        source_name: Option<&'a str>,
    ) -> MietteSpanContents<'a> {
        match source_name {
            Some(name) => MietteSpanContents::new_named(
                Cow::Borrowed(name),
                contents.data(),
                *contents.span(),
                contents.line(),
                contents.column(),
                contents.line_count(),
            ),
            None => contents,
        }
    }

    fn render_snippets_with<'a>(
        &self,
        f: &mut impl fmt::Write,
        labels: &[LabeledSpan],
        mut read: impl FnMut(&SourceSpan) -> Result<MietteSpanContents<'a>, crate::MietteError>,
    ) -> fmt::Result {
        if let [label] = labels {
            let contents = read(label.inner()).map_err(|_| fmt::Error)?;
            return self.render_context(f, label, &contents, labels);
        }

        let mut contexts: Vec<(Cow<'_, LabeledSpan>, _)> = Vec::with_capacity(labels.len());
        for right in labels {
            let right_conts = read(right.inner()).map_err(|_| fmt::Error)?;

            if contexts.is_empty() {
                contexts.push((Cow::Borrowed(right), right_conts));
                continue;
            }

            let (left, left_conts) = contexts.last().unwrap();
            if left_conts.line() + left_conts.line_count() >= right_conts.line() {
                // The snippets will overlap, so we create one Big Chunky Boi
                let left_end = left.offset() + left.len();
                let right_end = right.offset() + right.len();
                let new_end = max(left_end, right_end);

                let new_span = LabeledSpan::new(
                    left.label().map(String::from),
                    left.offset(),
                    new_end - left.offset(),
                );
                // Check that the two contexts can be combined
                if let Ok(new_conts) = read(new_span.inner()) {
                    contexts.pop();
                    contexts.push((Cow::Owned(new_span), new_conts));
                    continue;
                }
            }

            contexts.push((Cow::Borrowed(right), right_conts));
        }
        for (ctx, conts) in contexts {
            self.render_context(f, &ctx, &conts, labels)?;
        }

        Ok(())
    }

    pub(super) fn render_context(
        &self,
        f: &mut impl fmt::Write,
        context: &LabeledSpan,
        contents: &MietteSpanContents<'_>,
        labels: &[LabeledSpan],
    ) -> fmt::Result {
        let lines = self.get_lines(contents);

        // only consider labels from the context as primary label
        let ctx_labels = labels.iter().filter(|l| {
            context.inner().offset() <= l.inner().offset()
                && l.inner().offset() + l.inner().len()
                    <= context.inner().offset() + context.inner().len()
        });
        let primary_label =
            ctx_labels.clone().find(|label| label.primary()).or_else(|| ctx_labels.clone().next());

        // sorting is your friend
        let labels = labels
            .iter()
            .zip(self.theme.styles.highlights.iter().copied().cycle())
            .map(|(label, st)| FancySpan::new(label.label(), *label.inner(), st))
            .collect::<Vec<_>>();

        // The max number of gutter-lines that will be active at any given
        // point. We need this to figure out indentation, so we do one loop
        // over the lines to see what the damage is gonna be.
        let mut max_gutter = 0usize;
        for line in &lines {
            let mut num_highlights = 0;
            for hl in &labels {
                if !line.span_line_only(hl) && line.span_applies_gutter(hl) {
                    num_highlights += 1;
                }
            }
            max_gutter = max(max_gutter, num_highlights);
        }

        // Oh and one more thing: We need to figure out how much room our line
        // numbers need!
        let linum_width = lines
            .last()
            .map_or(1, |line| line.number.checked_ilog10().map_or(1, |width| width as usize + 1));

        // Header
        write_repeated_char(f, ' ', linum_width + 2)?;
        f.write_char(self.theme.characters.ltop)?;
        f.write_char(self.theme.characters.hbar)?;

        // The snippet header reports the primary label's line/column. Rather
        // than issuing a second full `read_span` (which re-scans the source
        // from byte 0 — as costly as the read that produced `contents`),
        // derive them from `contents`: its data begins at a line boundary at
        // line `contents.line()`, and the primary label always lies within it,
        // so only the short prefix up to the label needs to be walked.
        let (primary_line, primary_column) = match primary_label {
            Some(label) => {
                contents.line_column_at(label.inner().offset() as usize).ok_or(fmt::Error)?
            }
            None => (contents.line(), contents.column()),
        };

        match contents.name() {
            Some(source_name) => {
                let source_name = source_name.style(self.theme.styles.link);
                writeln!(f, "[{}:{}:{}]", source_name, primary_line + 1, primary_column + 1)?;
            }
            _ => {
                if lines.len() <= 1 {
                    write_repeated_char(f, self.theme.characters.hbar, 3)?;
                    f.write_char('\n')?;
                } else {
                    writeln!(f, "[{}:{}]", primary_line + 1, primary_column + 1)?;
                }
            }
        }

        // Now it's time for the fun part--actually rendering everything!
        for line in &lines {
            // Line number, appropriately padded.
            self.write_linum(f, linum_width, line.number)?;

            // Then, we need to print the gutter, along with any fly-bys We
            // have separate gutters depending on whether we're on the actual
            // line, or on one of the "highlight lines" below it.
            self.render_line_gutter(f, max_gutter, line, &labels)?;

            // And _now_ we can print out the line text itself!
            self.render_line_text(f, line.text)?;

            // Next, we write all the highlights that apply to this particular line.
            let (single_line, multi_line): (Vec<_>, Vec<_>) = labels
                .iter()
                .filter(|hl| line.span_applies(hl))
                .partition(|hl| line.span_line_only(hl));
            if !single_line.is_empty() {
                // no line number!
                self.write_no_linum(f, linum_width)?;
                // gutter _again_
                self.render_highlight_gutter(
                    f,
                    max_gutter,
                    line,
                    &labels,
                    LabelRenderMode::SingleLine,
                )?;
                self.render_single_line_highlights(
                    f,
                    line,
                    linum_width,
                    max_gutter,
                    &single_line,
                    &labels,
                )?;
            }
            for hl in multi_line {
                if hl.has_label() && line.span_ends(hl) && !line.span_starts(hl) {
                    self.render_multi_line_end(f, &labels, max_gutter, linum_width, line, hl)?;
                }
            }
        }
        write_repeated_char(f, ' ', linum_width + 2)?;
        f.write_char(self.theme.characters.lbot)?;
        write_repeated_char(f, self.theme.characters.hbar, 4)?;
        f.write_char('\n')?;
        Ok(())
    }

    /// Renders a line to the output formatter, replacing tabs with spaces.
    pub(super) fn render_line_text(&self, f: &mut impl fmt::Write, text: &str) -> fmt::Result {
        if !text.contains('\t') {
            f.write_str(text)?;
            return f.write_char('\n');
        }

        for (c, width) in text.chars().zip(self.line_visual_char_width(text)) {
            if c == '\t' {
                for _ in 0..width {
                    f.write_char(' ')?;
                }
            } else {
                f.write_char(c)?;
            }
        }
        f.write_char('\n')?;
        Ok(())
    }
}
