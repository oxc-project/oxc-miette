//! This example demonstrates the fix_diff feature for diagnostics.
//! 
//! The fix_diff field allows you to provide suggested fixes as a diff
//! that will be rendered below the source code snippet.

use miette::{Diagnostic, GraphicalReportHandler, NamedSource};
use thiserror::Error;

#[derive(Error, Debug, Diagnostic)]
#[error("Incorrect spacing in assignment")]
struct SpacingError {
    #[source_code]
    src: NamedSource<String>,
    #[label("extra spaces here")]
    span: (usize, usize),
    #[help]
    help: String,
    #[fix_diff]
    fix: Option<String>,
}

fn main() {
    // Example 1: Fix diff showing spacing correction
    let src = "let x =   5;";
    let diag = SpacingError {
        src: NamedSource::new("example.rs", src.to_string()),
        span: (6, 5),
        help: "Remove extra spaces between = and 5".to_string(),
        fix: Some("- let x =   5;\n+ let x = 5;".to_string()),
    };

    println!("Example 1: Simple spacing fix\n");
    let mut out = String::new();
    GraphicalReportHandler::new()
        .with_width(80)
        .render_report(&mut out, &diag)
        .unwrap();
    println!("{}", out);

    // Example 2: Multi-line fix diff
    let src2 = "function  add(a,b){\nreturn a+b\n}";
    
    #[derive(Error, Debug, Diagnostic)]
    #[error("Code formatting issues")]
    struct FormattingError {
        #[source_code]
        src: NamedSource<String>,
        #[label("formatting needed")]
        span: (usize, usize),
        #[help]
        help: String,
        #[fix_diff]
        fix: Option<String>,
    }

    let diag2 = FormattingError {
        src: NamedSource::new("formatter.js", src2.to_string()),
        span: (0, src2.len()),
        help: "Run formatter to fix spacing and punctuation".to_string(),
        fix: Some("- function  add(a,b){\n- return a+b\n- }\n+ function add(a, b) {\n+   return a + b;\n+ }".to_string()),
    };

    println!("\nExample 2: Multi-line formatting fix\n");
    let mut out2 = String::new();
    GraphicalReportHandler::new()
        .with_width(80)
        .render_report(&mut out2, &diag2)
        .unwrap();
    println!("{}", out2);
}
