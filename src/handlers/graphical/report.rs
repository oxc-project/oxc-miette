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
            writeln!(f, "{}", self.wrap(footer, opts))?;
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
                _ => "".to_string(),
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
            write!(header, "{}", code.style(severity_style),)?;
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

        let initial_indent = format!("  {} ", severity_icon.style(severity_style));
        let rest_indent = format!("  {} ", self.theme.characters.vbar.style(severity_style));
        let width = self.termwidth.saturating_sub(2);
        let opts = self.wrap_options(width, &initial_indent, &rest_indent);

        let title = match (self.links, diagnostic.url(), diagnostic.code()) {
            (LinkStyle::Link, Some(url), Some(code)) => {
                // magic unicode escape sequences to make the terminal print a hyperlink
                const CTL: &str = "\u{1b}]8;;";
                const END: &str = "\u{1b}]8;;\u{1b}\\";
                let code = code.style(severity_style);
                let message = diagnostic.to_string();
                let title = message.style(severity_style);
                format!("{CTL}{url}\u{1b}\\{code}{END}: {title}",)
            }
            (_, _, Some(code)) => {
                let title = format!("{code}: {diagnostic}");
                format!("{}", title.style(severity_style))
            }
            _ => {
                format!("{}", diagnostic.to_string().style(severity_style))
            }
        };
        let title = textwrap::fill(&title, opts);
        writeln!(f, "{title}")?;

        Ok(())
    }

    fn render_footer(&self, f: &mut impl fmt::Write, diagnostic: &dyn Diagnostic) -> fmt::Result {
        if let Some(help) = diagnostic.help() {
            let width = self.termwidth.saturating_sub(4);
            let initial_indent = "  help: ".style(self.theme.styles.help).to_string();
            let opts = self.wrap_options(width, &initial_indent, "        ");
            writeln!(f, "{}", self.wrap(&help, opts))?;
        }
        if let Some(note) = diagnostic.note() {
            // Renders as:
            //   note: This is a note about the error
            let width = self.termwidth.saturating_sub(4);
            let initial_indent = "  note: ".style(self.theme.styles.note).to_string();
            let opts = self.wrap_options(width, &initial_indent, "           ");
            writeln!(f, "{}", self.wrap(&note, opts))?;
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
                };
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
            textwrap::fill(text, opts)
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
}
