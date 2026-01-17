#![cfg(feature = "fancy-no-backtrace")]

use miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme, NamedSource, SourceSpan};
use std::fmt;

#[derive(Debug)]
struct TestDiagnostic {
    src: NamedSource<String>,
    span: SourceSpan,
    help: String,
    fix_diff: Option<String>,
}

impl fmt::Display for TestDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Invalid syntax")
    }
}

impl std::error::Error for TestDiagnostic {}

impl Diagnostic for TestDiagnostic {
    fn help<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        Some(Box::new(&self.help))
    }

    fn source_code(&self) -> Option<&dyn miette::SourceCode> {
        Some(&self.src)
    }

    fn labels(&self) -> Option<Box<dyn Iterator<Item = miette::LabeledSpan> + '_>> {
        Some(Box::new(std::iter::once(
            miette::LabeledSpan::at(self.span, "here"),
        )))
    }

    fn fix_diff<'a>(&'a self) -> Option<Box<dyn fmt::Display + 'a>> {
        self.fix_diff.as_ref().map(|d| Box::new(d) as Box<dyn fmt::Display>)
    }
}

#[test]
fn test_fix_diff_rendering() {
    let src = "let x =   5;";
    let diag = TestDiagnostic {
        src: NamedSource::new("example.rs", src.to_string()),
        span: (0, 12).into(),
        help: "Remove extra spaces".to_string(),
        fix_diff: Some("- let x =   5;\n+ let x = 5;".to_string()),
    };

    let mut out = String::new();
    GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
        .with_width(80)
        .render_report(&mut out, &diag)
        .unwrap();

    println!("Output:\n{}", out);

    // Basic check that the output contains expected elements
    assert!(out.contains("Invalid syntax"));
    assert!(out.contains("example.rs"));
    assert!(out.contains("help: Remove extra spaces"));
    assert!(out.contains("- let x =   5;"));
    assert!(out.contains("+ let x = 5;"));
}

#[test]
fn test_no_fix_diff() {
    let src = "let x = 5;";
    let diag = TestDiagnostic {
        src: NamedSource::new("example.rs", src.to_string()),
        span: (0, 10).into(),
        help: "This is fine".to_string(),
        fix_diff: None,
    };

    let mut out = String::new();
    GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
        .with_width(80)
        .render_report(&mut out, &diag)
        .unwrap();

    println!("Output:\n{}", out);
    assert!(out.contains("Invalid syntax"));
    assert!(!out.contains("- let"));
    assert!(!out.contains("+ let"));
}

