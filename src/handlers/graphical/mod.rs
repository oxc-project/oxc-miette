//! The graphical (fancy) diagnostic renderer.
//!
//! [`GraphicalReportHandler`] turns a [`Diagnostic`] into a richly formatted
//! report: a colored title, the relevant source-code snippets with each label
//! underlined and named, and any help/note/related diagnostics.
//!
//! The rendering pipeline is split across the submodules, roughly in the order
//! output is produced:
//!
//! - [`handler`] — the [`GraphicalReportHandler`] type and its builder API.
//! - [`report`] — the top level: title, causes, help/note, related, wrapping.
//! - [`snippet`] — reads the labelled spans and lays out the source snippets.
//! - [`gutter`] — the line-number column and multi-line span gutters.
//! - [`label`] — the underlines and labels drawn under the source text.
//! - [`line`] — the [`Line`](line::Line) model, line splitting, and width math.
//! - [`span`] — [`FancySpan`](span::FancySpan), a styled labelled span.

mod gutter;
mod handler;
mod label;
mod line;
mod report;
mod snippet;
mod span;

use std::fmt;

pub use handler::GraphicalReportHandler;

use crate::{Diagnostic, ReportHandler};

impl ReportHandler for GraphicalReportHandler {
    fn debug(&self, diagnostic: &dyn Diagnostic, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if f.alternate() {
            return fmt::Debug::fmt(diagnostic, f);
        }

        self.render_report(f, diagnostic)
    }
}
