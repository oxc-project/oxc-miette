#![cfg(feature = "fancy-no-backtrace")]

use std::{fmt, fmt::Write as _, sync::Arc};

use miette::{
    Diagnostic, Error as Report, GraphicalReportHandler, GraphicalTheme, LabeledSpan,
    MietteDiagnostic, NamedSource, SourceSpan,
};
use thiserror::Error;

fn handler() -> GraphicalReportHandler {
    GraphicalReportHandler::new_themed(GraphicalTheme::none()).with_width(80)
}

fn source_lines(prefix: &str, count: usize) -> String {
    let mut source = String::new();
    for line in 0..count {
        writeln!(source, "{prefix} {line:02}").unwrap();
    }
    source
}

fn line_offset(prefix: &str, line: usize) -> u32 {
    u32::try_from(line * (prefix.len() + 4)).unwrap()
}

fn diagnostic(
    source: &Arc<NamedSource<String>>,
    message: impl Into<String>,
    span: SourceSpan,
) -> Report {
    let diagnostic =
        MietteDiagnostic::new(message.into()).with_label(LabeledSpan::new_with_span(None, span));
    Report::new(diagnostic).with_source_code(Arc::clone(source))
}

fn as_diagnostic(report: &Report) -> &dyn Diagnostic {
    report.as_ref()
}

fn render_individually(reports: &[Report]) -> (fmt::Result, String) {
    let mut output = String::new();
    let handler = handler();
    let result = reports
        .iter()
        .try_for_each(|report| handler.render_report(&mut output, as_diagnostic(report)));
    (result, output)
}

fn render_batch(reports: &[Report]) -> (fmt::Result, String) {
    let mut output = String::new();
    let result = handler().render_reports(&mut output, reports.iter().map(as_diagnostic));
    (result, output)
}

#[test]
fn preserves_input_order_for_shared_sources() {
    let source_text = source_lines("line", 40);
    let source = Arc::new(NamedSource::new("shared.rs", source_text));
    let reports = [
        diagnostic(&source, "last", (line_offset("line", 35), 7).into()),
        diagnostic(&source, "first", (line_offset("line", 2), 7).into()),
        diagnostic(&source, "middle", (line_offset("line", 20), 7).into()),
    ];

    let individual = render_individually(&reports);
    let batch = render_batch(&reports);
    assert_eq!(individual.0, batch.0);
    assert_eq!(individual.1, batch.1);
    assert!(batch.1.find("last").unwrap() < batch.1.find("first").unwrap());
    assert!(batch.1.find("first").unwrap() < batch.1.find("middle").unwrap());
}

#[test]
fn reuses_interleaved_sources_without_reordering() {
    let first = Arc::new(NamedSource::new("first.rs", source_lines("first", 30)));
    let second = Arc::new(NamedSource::new("second.rs", source_lines("second", 30)));
    let reports = [
        diagnostic(&first, "first-late", (line_offset("first", 25), 8).into()),
        diagnostic(&second, "second-late", (line_offset("second", 20), 9).into()),
        diagnostic(&first, "first-early", (line_offset("first", 3), 8).into()),
        diagnostic(&second, "second-early", (line_offset("second", 1), 9).into()),
    ];

    let individual = render_individually(&reports);
    let batch = render_batch(&reports);
    assert_eq!(individual.0, batch.0);
    assert_eq!(individual.1, batch.1);
}

#[test]
fn keeps_names_separate_when_sources_share_bytes() {
    static SOURCE: &str = "shared bytes\nsecond line\n";
    let first = Arc::new(NamedSource::new("first-name.rs", SOURCE));
    let second = Arc::new(NamedSource::new("second-name.rs", SOURCE));
    let reports = [
        Report::new(
            MietteDiagnostic::new("first name")
                .with_label(LabeledSpan::new_with_span(None, SourceSpan::from((13, 6)))),
        )
        .with_source_code(first),
        Report::new(
            MietteDiagnostic::new("second name")
                .with_label(LabeledSpan::new_with_span(None, SourceSpan::from((0, 6)))),
        )
        .with_source_code(second),
    ];

    let individual = render_individually(&reports);
    let batch = render_batch(&reports);
    assert_eq!(individual.0, batch.0);
    assert_eq!(individual.1, batch.1);
    assert!(batch.1.contains("first-name.rs"));
    assert!(batch.1.contains("second-name.rs"));
}

#[test]
fn preserves_related_diagnostics_and_inherited_sources() {
    #[derive(Debug, Diagnostic, Error)]
    #[error("child")]
    struct Child {
        #[label]
        span: SourceSpan,
    }

    #[derive(Debug, Diagnostic, Error)]
    #[error("parent")]
    struct Parent {
        #[source_code]
        src: Arc<NamedSource<String>>,
        #[label]
        span: SourceSpan,
        #[related]
        children: Vec<Child>,
    }

    let source = Arc::new(NamedSource::new("related.rs", source_lines("line", 30)));
    let reports = [Report::new(Parent {
        src: source,
        span: (line_offset("line", 20), 7).into(),
        children: vec![Child { span: (line_offset("line", 2), 7).into() }],
    })];

    let individual = render_individually(&reports);
    let batch = render_batch(&reports);
    assert_eq!(individual.0, batch.0);
    assert_eq!(individual.1, batch.1);
}

#[test]
fn preserves_partial_output_on_invalid_span() {
    let source = Arc::new(NamedSource::new("invalid.rs", "valid source\n".to_string()));
    let reports = [
        diagnostic(&source, "valid", (0, 5).into()),
        diagnostic(&source, "invalid", (100, 4).into()),
    ];

    let individual = render_individually(&reports);
    let batch = render_batch(&reports);
    assert!(individual.0.is_err());
    assert!(batch.0.is_err());
    assert_eq!(individual.1, batch.1);
}

#[test]
fn empty_batch_writes_nothing() {
    let reports: [Report; 0] = [];
    assert_eq!(render_batch(&reports), (Ok(()), String::new()));
}
