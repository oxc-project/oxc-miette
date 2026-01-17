#![cfg(feature = "fancy-no-backtrace")]

use miette::{Diagnostic, GraphicalReportHandler, GraphicalTheme, NamedSource, SourceSpan};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[error("Invalid syntax")]
struct DiagnosticWithFixDiffField {
    #[source_code]
    src: NamedSource<String>,
    #[label("here")]
    span: SourceSpan,
    #[help]
    help: String,
    #[fix_diff]
    fix: Option<String>,
}

#[derive(Error, Debug, Diagnostic)]
#[error("Invalid spacing")]
#[diagnostic(fix_diff = "- let x =   5;\n+ let x = 5;")]
struct DiagnosticWithFixDiffAttr {
    #[source_code]
    src: NamedSource<String>,
    #[label("here")]
    span: SourceSpan,
    #[help]
    help: String,
}

#[test]
fn test_fix_diff_field_annotation() {
    let src = "let x =   5;";
    let diag = DiagnosticWithFixDiffField {
        src: NamedSource::new("example.rs", src.to_string()),
        span: (0, 12).into(),
        help: "Remove extra spaces".to_string(),
        fix: Some("- let x =   5;\n+ let x = 5;".to_string()),
    };

    let mut out = String::new();
    GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
        .with_width(80)
        .render_report(&mut out, &diag)
        .unwrap();

    println!("Output:\n{}", out);

    assert!(out.contains("Invalid syntax"));
    assert!(out.contains("example.rs"));
    assert!(out.contains("help: Remove extra spaces"));
    assert!(out.contains("- let x =   5;"));
    assert!(out.contains("+ let x = 5;"));
}

#[test]
fn test_fix_diff_attribute() {
    let src = "let x =   5;";
    let diag = DiagnosticWithFixDiffAttr {
        src: NamedSource::new("example.rs", src.to_string()),
        span: (0, 12).into(),
        help: "Remove extra spaces".to_string(),
    };

    let mut out = String::new();
    GraphicalReportHandler::new_themed(GraphicalTheme::unicode_nocolor())
        .with_width(80)
        .render_report(&mut out, &diag)
        .unwrap();

    println!("Output:\n{}", out);

    assert!(out.contains("Invalid spacing"));
    assert!(out.contains("example.rs"));
    assert!(out.contains("help: Remove extra spaces"));
    assert!(out.contains("- let x =   5;"));
    assert!(out.contains("+ let x = 5;"));
}

#[test]
fn test_none_fix_diff() {
    let src = "let x = 5;";
    let diag = DiagnosticWithFixDiffField {
        src: NamedSource::new("example.rs", src.to_string()),
        span: (0, 10).into(),
        help: "This is fine".to_string(),
        fix: None,
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
