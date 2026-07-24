//! Diagnostic-level rendering: everything except the source snippets.
//!
//! [`render_report`](GraphicalReportHandler::render_report) is the entry point.
//! It renders the title/causes, hands off to
//! [`render_snippets`](GraphicalReportHandler::render_snippets), then renders
//! the help/note footer, any related diagnostics, and the handler's global
//! footer. Each block of prose is wrapped to the terminal width using the
//! shared [`wrap_options`](GraphicalReportHandler::wrap_options) /
//! [`wrap`](GraphicalReportHandler::wrap) helpers.

use std::fmt::{self, Write};

use owo_colors::OwoColorize;

use super::handler::{GraphicalReportHandler, LinkStyle};
use crate::{Diagnostic, Severity, SourceCode};

impl GraphicalReportHandler {
    /// Render a [`Diagnostic`]. This function is mostly internal and meant to
    /// be called by the toplevel [`ReportHandler`](crate::ReportHandler)
    /// handler, but is made public to make it easier (possible) to test in
    /// isolation from global state.
    ///
    /// # Errors
    ///
    /// Returns an error when writing the rendered report fails.
    pub fn render_report(
        &self,
        f: &mut impl fmt::Write,
        diagnostic: &dyn Diagnostic,
    ) -> fmt::Result {
        writeln!(f)?;
        self.render_causes(f, diagnostic)?;
        let src = diagnostic.source_code();
        self.render_snippets(f, diagnostic, src)?;
        self.render_footer(f, diagnostic)?;
        self.render_related(f, diagnostic, src)?;
        if let Some(footer) = &self.footer {
            writeln!(f)?;
            let width = self.termwidth.saturating_sub(4);
            let opts = self.wrap_options(width, "  ", "  ");
            self.write_wrap(f, footer, opts)?;
            f.write_char('\n')?;
        }
        Ok(())
    }

    fn render_header(&self, f: &mut impl fmt::Write, diagnostic: &dyn Diagnostic) -> fmt::Result {
        let severity_style = match diagnostic.severity() {
            Some(Severity::Error) | None => self.theme.styles.error,
            Some(Severity::Warning) => self.theme.styles.warning,
            Some(Severity::Advice) => self.theme.styles.advice,
        };
        let mut header = String::new();
        if self.links == LinkStyle::Link && diagnostic.url().is_some() {
            let url = diagnostic.url().unwrap(); // safe
            let code = match diagnostic.code() {
                Some(code) => {
                    format!("{code} ")
                }
                _ => String::new(),
            };
            let display_text = self.link_display_text.as_deref().unwrap_or("(link)");
            let link = format!(
                "\u{1b}]8;;{}\u{1b}\\{}{}\u{1b}]8;;\u{1b}\\",
                url,
                code.style(severity_style),
                display_text.style(self.theme.styles.link)
            );
            write!(header, "{link}")?;
            writeln!(f, "{header}")?;
            writeln!(f)?;
        } else if let Some(code) = diagnostic.code() {
            write!(header, "{}", code.style(severity_style))?;
            if self.links == LinkStyle::Text && diagnostic.url().is_some() {
                let url = diagnostic.url().unwrap(); // safe
                write!(header, " ({})", url.style(self.theme.styles.link))?;
            }
            writeln!(f, "{header}")?;
            writeln!(f)?;
        }
        Ok(())
    }

    fn render_causes(&self, f: &mut impl fmt::Write, diagnostic: &dyn Diagnostic) -> fmt::Result {
        let (severity_style, severity_icon) = match diagnostic.severity() {
            Some(Severity::Error) | None => (self.theme.styles.error, &self.theme.characters.error),
            Some(Severity::Warning) => (self.theme.styles.warning, &self.theme.characters.warning),
            Some(Severity::Advice) => (self.theme.styles.advice, &self.theme.characters.advice),
        };

        // No-color themes can bypass owo-colors' formatting machinery entirely.
        let (initial_indent, rest_indent) = if severity_style.is_plain() {
            (format!("  {severity_icon} "), format!("  {} ", self.theme.characters.vbar))
        } else {
            (
                format!("  {} ", severity_icon.style(severity_style)),
                format!("  {} ", self.theme.characters.vbar.style(severity_style)),
            )
        };
        let width = self.termwidth.saturating_sub(2);
        let opts = self.wrap_options(width, &initial_indent, &rest_indent);

        let title = match (self.links, diagnostic.url(), diagnostic.code()) {
            (LinkStyle::Link, Some(url), Some(code)) => {
                // magic unicode escape sequences to make the terminal print a hyperlink
                const CTL: &str = "\u{1b}]8;;";
                const END: &str = "\u{1b}]8;;\u{1b}\\";
                let code = code.style(severity_style);
                let title = diagnostic.style(severity_style);
                format!("{CTL}{url}\u{1b}\\{code}{END}: {title}")
            }
            (_, _, Some(code)) if severity_style.is_plain() => format!("{code}: {diagnostic}"),
            (_, _, Some(code)) => {
                format!("{}", format_args!("{code}: {diagnostic}").style(severity_style))
            }
            _ if severity_style.is_plain() => diagnostic.to_string(),
            _ => format!("{}", diagnostic.style(severity_style)),
        };
        Self::write_fill(f, &title, opts)?;
        f.write_char('\n')?;

        Ok(())
    }

    fn render_footer(&self, f: &mut impl fmt::Write, diagnostic: &dyn Diagnostic) -> fmt::Result {
        if let Some(help) = diagnostic.help() {
            let width = self.termwidth.saturating_sub(4);
            let initial_indent = "  help: ".style(self.theme.styles.help).to_string();
            let opts = self.wrap_options(width, &initial_indent, "        ");
            self.write_wrap(f, &help, opts)?;
            f.write_char('\n')?;
        }
        if let Some(note) = diagnostic.note() {
            // Renders as:
            //   note: This is a note about the error
            let width = self.termwidth.saturating_sub(4);
            let initial_indent = "  note: ".style(self.theme.styles.note).to_string();
            let opts = self.wrap_options(width, &initial_indent, "           ");
            self.write_wrap(f, &note, opts)?;
            f.write_char('\n')?;
        }
        Ok(())
    }

    fn render_related(
        &self,
        f: &mut impl fmt::Write,
        diagnostic: &dyn Diagnostic,
        parent_src: Option<&dyn SourceCode>,
    ) -> fmt::Result {
        let related = diagnostic.related();
        if !related.is_empty() {
            let inner_renderer = self.clone();
            writeln!(f)?;
            for rel in related.iter().copied() {
                match rel.severity() {
                    Some(Severity::Error) | None => write!(f, "Error: ")?,
                    Some(Severity::Warning) => write!(f, "Warning: ")?,
                    Some(Severity::Advice) => write!(f, "Advice: ")?,
                }
                inner_renderer.render_header(f, rel)?;
                inner_renderer.render_causes(f, rel)?;
                let src = rel.source_code().or(parent_src);
                inner_renderer.render_snippets(f, rel, src)?;
                inner_renderer.render_footer(f, rel)?;
                inner_renderer.render_related(f, rel, src)?;
            }
        }
        Ok(())
    }

    /// Builds the [`textwrap::Options`] shared by every wrapped block — the
    /// title/causes, the help and note footers, and the global footer. Applies
    /// the handler's `break_words` setting plus any configured word separator
    /// and splitter, so those options stay consistent across all of them.
    fn wrap_options<'a>(
        &self,
        width: usize,
        initial_indent: &'a str,
        subsequent_indent: &'a str,
    ) -> textwrap::Options<'a> {
        let mut opts = textwrap::Options::new(width)
            .initial_indent(initial_indent)
            .subsequent_indent(subsequent_indent)
            .break_words(self.break_words);
        if let Some(word_separator) = self.word_separator {
            opts = opts.word_separator(word_separator);
        }
        if let Some(word_splitter) = self.word_splitter.clone() {
            opts = opts.word_splitter(word_splitter);
        }
        opts
    }

    fn wrap(&self, text: &str, opts: textwrap::Options<'_>) -> String {
        if self.wrap_lines {
            Self::fill(text, opts)
        } else {
            // Format without wrapping, but retain the indentation options
            // Implementation based on `textwrap::indent`
            let mut result = String::with_capacity(2 * text.len());
            let trimmed_indent = opts.subsequent_indent.trim_end();
            for (idx, line) in text.split_terminator('\n').enumerate() {
                if idx > 0 {
                    result.push('\n');
                }
                if idx == 0 {
                    if line.trim().is_empty() {
                        result.push_str(opts.initial_indent.trim_end());
                    } else {
                        result.push_str(opts.initial_indent);
                    }
                } else if line.trim().is_empty() {
                    result.push_str(trimmed_indent);
                } else {
                    result.push_str(opts.subsequent_indent);
                }
                result.push_str(line);
            }
            if text.ends_with('\n') {
                // split_terminator will have eaten the final '\n'.
                result.push('\n');
            }
            result
        }
    }

    fn write_wrap(
        &self,
        f: &mut impl fmt::Write,
        text: &str,
        opts: textwrap::Options<'_>,
    ) -> fmt::Result {
        if self.wrap_lines {
            Self::write_fill(f, text, opts)
        } else {
            f.write_str(&self.wrap(text, opts))
        }
    }

    fn write_fill(f: &mut impl fmt::Write, text: &str, opts: textwrap::Options<'_>) -> fmt::Result {
        if Self::fits_on_line(text, &opts) {
            f.write_str(opts.initial_indent)?;
            f.write_str(text.trim_end_matches(' '))
        } else {
            f.write_str(&textwrap::fill(text, opts))
        }
    }

    /// Skip word separation and optimal-fit layout when the text demonstrably
    /// fits on its first line. `textwrap` only provides this fast path without
    /// indentation, while every diagnostic block has an initial indent.
    fn fill(text: &str, opts: textwrap::Options<'_>) -> String {
        if Self::fits_on_line(text, &opts) {
            let text = text.trim_end_matches(' ');
            let mut result = String::with_capacity(opts.initial_indent.len() + text.len());
            result.push_str(opts.initial_indent);
            result.push_str(text);
            return result;
        }
        textwrap::fill(text, opts)
    }

    fn fits_on_line(text: &str, opts: &textwrap::Options<'_>) -> bool {
        if memchr::memchr(b'\n', text.as_bytes()).is_some() {
            return false;
        }

        // UTF-8 byte length is an upper bound on terminal display width,
        // including for ANSI escape sequences. Avoid both width scans when
        // even that conservative bound fits.
        opts.initial_indent.len().saturating_add(text.len()) <= opts.width || {
            let available = opts.width.saturating_sub(Self::display_width(opts.initial_indent));
            Self::display_width(text) <= available
        }
    }

    /// Compute terminal width bytewise for ASCII, including the CSI and OSC
    /// escape sequences recognized by `textwrap`. Unicode retains its full
    /// width calculation.
    fn display_width(text: &str) -> usize {
        if !text.is_ascii() {
            return textwrap::core::display_width(text);
        }

        let bytes = text.as_bytes();
        let mut width = 0;
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] != b'\x1b' {
                width += usize::from((b' '..=b'~').contains(&bytes[i]));
                i += 1;
                continue;
            }

            i += 1;
            let Some(&kind) = bytes.get(i) else { break };
            i += 1;
            match kind {
                b'[' => {
                    while i < bytes.len() {
                        let byte = bytes[i];
                        i += 1;
                        if (b'@'..=b'~').contains(&byte) {
                            break;
                        }
                    }
                }
                b']' => {
                    while i < bytes.len() {
                        if bytes[i] == b'\x07' {
                            i += 1;
                            break;
                        }
                        if bytes[i] == b'\x1b' && bytes.get(i + 1) == Some(&b'\\') {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                _ => {}
            }
        }
        width
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(
        miri,
        ignore = "exhaustive equivalence check over safe text wrapping code; interpreting every \
                  textwrap case under Miri takes more than 16 minutes"
    )]
    fn fill_fast_path_matches_textwrap() {
        let texts = [
            "",
            "short diagnostic",
            "trailing spaces   ",
            "  leading spaces",
            "two  inner  spaces",
            "Café 火",
            "combining e\u{301}",
            "emoji 🐂",
            "\u{1b}[31mstyled text\u{1b}[0m",
            "\u{1b}]8;;https://example.com\u{1b}\\linked\u{1b}]8;;\u{1b}\\",
            "\u{1b}]0;title\u{7}visible",
            "control\tcharacters\u{7}",
            "incomplete \u{1b}[31",
            "first\nsecond",
        ];
        for width in 0..32 {
            for initial_indent in ["", "  ", "  help: ", "\u{1b}[31m  × \u{1b}[0m"] {
                for text in texts {
                    let opts = textwrap::Options::new(width)
                        .initial_indent(initial_indent)
                        .subsequent_indent("    ");
                    assert_eq!(
                        GraphicalReportHandler::fill(text, opts.clone()),
                        textwrap::fill(text, opts.clone()),
                        "width={width}, indent={initial_indent:?}, text={text:?}"
                    );
                    let mut output = String::new();
                    GraphicalReportHandler::write_fill(&mut output, text, opts.clone()).unwrap();
                    assert_eq!(
                        output,
                        textwrap::fill(text, opts),
                        "streaming: width={width}, indent={initial_indent:?}, text={text:?}"
                    );
                }
            }
        }
    }
}
