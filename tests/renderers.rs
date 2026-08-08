use std::{borrow::Cow, fmt};

use miette::{
    Diagnostic, GraphicalReportHandler, GraphicalTheme, JSONReportHandler, LabeledSpan,
    NamedSource, Severity, SourceCode,
};

#[derive(Debug)]
struct TestDiagnostic {
    source: NamedSource<String>,
    labels: [LabeledSpan; 1],
}

impl fmt::Display for TestDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unexpected token")
    }
}

impl std::error::Error for TestDiagnostic {}

impl Diagnostic for TestDiagnostic {
    fn code(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("parser::unexpected"))
    }

    fn severity(&self) -> Option<Severity> {
        Some(Severity::Error)
    }

    fn help(&self) -> Option<Cow<'_, str>> {
        Some(Cow::Borrowed("remove it"))
    }

    fn labels(&self) -> &[LabeledSpan] {
        &self.labels
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        Some(&self.source)
    }
}

fn diagnostic() -> TestDiagnostic {
    TestDiagnostic {
        source: NamedSource::new("test.js", String::from("let ? = 1;")),
        labels: [LabeledSpan::at(4..5, "here")],
    }
}

#[test]
fn graphical_renderer_is_explicit() {
    let mut output = String::new();
    GraphicalReportHandler::new_themed(GraphicalTheme::none())
        .with_width(80)
        .with_links(false)
        .render_report(&mut output, &diagnostic())
        .unwrap();

    assert!(output.contains("unexpected token"));
    assert!(output.contains("test.js"));
    assert!(output.contains("here"));
    assert!(output.contains("remove it"));
}

#[test]
fn json_renderer_is_explicit() {
    let mut output = String::new();
    JSONReportHandler::new().render_report(&mut output, &diagnostic()).unwrap();

    assert!(output.contains(r#""message": "unexpected token""#));
    assert!(output.contains(r#""code": "parser::unexpected""#));
    assert!(output.contains(r#""filename": "test.js""#));
    assert!(output.contains(r#""label": "here""#));
}
