//! The annotations drawn under the source text.
//!
//! For spans that begin and end on one line,
//! [`render_single_line_highlights`](GraphicalReportHandler::render_single_line_highlights)
//! draws the `───┬───` underlines and then, via
//! [`write_label_text`](GraphicalReportHandler::write_label_text), the label
//! text hanging off each one. For multi-line spans the closing label is drawn
//! by [`render_multi_line_end`](GraphicalReportHandler::render_multi_line_end).

use std::{
    cmp::max,
    fmt::{self, Write},
};

use owo_colors::{OwoColorize, Style};

use super::{
    handler::GraphicalReportHandler,
    line::Line,
    span::{FancySpan, LabelRenderMode},
};
use crate::ThemeCharacters;

impl GraphicalReportHandler {
    pub(super) fn render_single_line_highlights(
        &self,
        f: &mut impl fmt::Write,
        line: &Line<'_>,
        linum_width: usize,
        max_gutter: usize,
        single_liners: &[&FancySpan],
        all_highlights: &[FancySpan],
    ) -> fmt::Result {
        let mut underlines = String::new();
        let mut highest = 0;

        let chars = &self.theme.characters;
        let vbar_offsets: Vec<_> = single_liners
            .iter()
            .map(|hl| {
                let byte_start = hl.offset();
                let byte_end = hl.offset() + hl.len();
                let start = self.visual_offset(line, byte_start, true).max(highest);
                let end = if hl.len() == 0 {
                    start + 1
                } else {
                    self.visual_offset(line, byte_end, false).max(start + 1)
                };

                let vbar_offset = (start + end) / 2;
                let num_left = vbar_offset - start;
                let num_right = end - vbar_offset - 1;
                // Throws `Formatting argument out of range` when width is above u16::MAX.
                let width = start.saturating_sub(highest).min(u16::MAX as usize);
                let _ = write!(
                    underlines,
                    "{}",
                    format!(
                        "{:width$}{}{}{}",
                        "",
                        chars.underline.to_string().repeat(num_left),
                        if hl.len() == 0 {
                            chars.uarrow
                        } else if hl.has_label() {
                            chars.underbar
                        } else {
                            chars.underline
                        },
                        chars.underline.to_string().repeat(num_right),
                    )
                    .style(hl.style)
                );
                highest = max(highest, end);

                (hl, vbar_offset)
            })
            .collect();
        writeln!(f, "{underlines}")?;

        for hl in single_liners.iter().rev() {
            if let Some(label) = hl.label_parts() {
                if label.len() == 1 {
                    self.write_label_text(
                        f,
                        line,
                        linum_width,
                        max_gutter,
                        all_highlights,
                        chars,
                        &vbar_offsets,
                        hl,
                        &label[0],
                        LabelRenderMode::SingleLine,
                    )?;
                } else {
                    let mut first = true;
                    for label_line in label {
                        self.write_label_text(
                            f,
                            line,
                            linum_width,
                            max_gutter,
                            all_highlights,
                            chars,
                            &vbar_offsets,
                            hl,
                            label_line,
                            if first {
                                LabelRenderMode::BlockFirst
                            } else {
                                LabelRenderMode::BlockRest
                            },
                        )?;
                        first = false;
                    }
                }
            }
        }
        Ok(())
    }

    // I know it's not good practice, but making this a function makes a lot of sense
    // and making a struct for this does not...
    #[allow(clippy::too_many_arguments)]
    pub(super) fn write_label_text(
        &self,
        f: &mut impl fmt::Write,
        line: &Line<'_>,
        linum_width: usize,
        max_gutter: usize,
        all_highlights: &[FancySpan],
        chars: &ThemeCharacters,
        vbar_offsets: &[(&&FancySpan, usize)],
        hl: &&FancySpan,
        label: &str,
        render_mode: LabelRenderMode,
    ) -> fmt::Result {
        self.write_no_linum(f, linum_width)?;
        self.render_highlight_gutter(
            f,
            max_gutter,
            line,
            all_highlights,
            LabelRenderMode::SingleLine,
        )?;
        let mut curr_offset = 1usize;
        for (offset_hl, vbar_offset) in vbar_offsets {
            while curr_offset < *vbar_offset + 1 {
                write!(f, " ")?;
                curr_offset += 1;
            }
            if *offset_hl != hl {
                write!(f, "{}", chars.vbar.to_string().style(offset_hl.style))?;
                curr_offset += 1;
            } else {
                let lines = match render_mode {
                    LabelRenderMode::SingleLine => {
                        format!("{}{} {}", chars.lbot, chars.hbar.to_string().repeat(2), label,)
                    }
                    LabelRenderMode::BlockFirst => {
                        format!("{}{}{} {}", chars.lbot, chars.hbar, chars.rcross, label,)
                    }
                    LabelRenderMode::BlockRest => {
                        format!("  {} {}", chars.vbar, label,)
                    }
                };
                writeln!(f, "{}", lines.style(hl.style))?;
                break;
            }
        }
        Ok(())
    }

    pub(super) fn render_multi_line_end(
        &self,
        f: &mut impl fmt::Write,
        labels: &[FancySpan],
        max_gutter: usize,
        linum_width: usize,
        line: &Line<'_>,
        label: &FancySpan,
    ) -> fmt::Result {
        // no line number!
        self.write_no_linum(f, linum_width)?;

        if let Some(label_parts) = label.label_parts() {
            // if it has a label, how long is it?
            let (first, rest) = label_parts
                .split_first()
                .expect("cannot crash because rest would have been None, see docs on the `label` field of FancySpan");

            if rest.is_empty() {
                // gutter _again_
                self.render_highlight_gutter(
                    f,
                    max_gutter,
                    line,
                    labels,
                    LabelRenderMode::SingleLine,
                )?;

                self.render_multi_line_end_single(
                    f,
                    first,
                    label.style,
                    LabelRenderMode::SingleLine,
                )?;
            } else {
                // gutter _again_
                self.render_highlight_gutter(
                    f,
                    max_gutter,
                    line,
                    labels,
                    LabelRenderMode::BlockFirst,
                )?;

                self.render_multi_line_end_single(
                    f,
                    first,
                    label.style,
                    LabelRenderMode::BlockFirst,
                )?;
                for label_line in rest {
                    // no line number!
                    self.write_no_linum(f, linum_width)?;
                    // gutter _again_
                    self.render_highlight_gutter(
                        f,
                        max_gutter,
                        line,
                        labels,
                        LabelRenderMode::BlockRest,
                    )?;
                    self.render_multi_line_end_single(
                        f,
                        label_line,
                        label.style,
                        LabelRenderMode::BlockRest,
                    )?;
                }
            }
        } else {
            // gutter _again_
            self.render_highlight_gutter(f, max_gutter, line, labels, LabelRenderMode::SingleLine)?;
            // has no label
            writeln!(f, "{}", self.theme.characters.hbar.style(label.style))?;
        }

        Ok(())
    }

    pub(super) fn render_multi_line_end_single(
        &self,
        f: &mut impl fmt::Write,
        label: &str,
        style: Style,
        render_mode: LabelRenderMode,
    ) -> fmt::Result {
        match render_mode {
            LabelRenderMode::SingleLine => {
                writeln!(f, "{} {}", self.theme.characters.hbar.style(style), label)?;
            }
            LabelRenderMode::BlockFirst => {
                writeln!(f, "{} {}", self.theme.characters.rcross.style(style), label)?;
            }
            LabelRenderMode::BlockRest => {
                writeln!(f, "{} {}", self.theme.characters.vbar.style(style), label)?;
            }
        }

        Ok(())
    }
}
