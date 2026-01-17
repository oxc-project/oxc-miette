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
    // Example 1: Simple single-line fix
    let src = "let x =   5;";
    let diag = SpacingError {
        src: NamedSource::new("example.rs", src.to_string()),
        span: (6, 5),
        help: "Remove extra spaces between = and 5".to_string(),
        fix: Some("- let x =   5;\n+ let x = 5;".to_string()),
    };

    println!("Example 1: Simple single-line fix\n");
    let mut out = String::new();
    GraphicalReportHandler::new()
        .with_width(80)
        .render_report(&mut out, &diag)
        .unwrap();
    println!("{}", out);

    // Example 2: Multi-line replacement
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

    println!("\nExample 2: Multi-line replacement\n");
    let mut out2 = String::new();
    GraphicalReportHandler::new()
        .with_width(80)
        .render_report(&mut out2, &diag2)
        .unwrap();
    println!("{}", out2);

    // Example 3: Diff with context lines
    #[derive(Error, Debug, Diagnostic)]
    #[error("Missing semicolon")]
    struct MissingSemicolon {
        #[source_code]
        src: NamedSource<String>,
        #[label("semicolon needed here")]
        span: (usize, usize),
        #[help]
        help: String,
        #[fix_diff]
        fix: Option<String>,
    }

    let src3 = "function test() {\n  let x = 5\n  return x;\n}";
    let diag3 = MissingSemicolon {
        src: NamedSource::new("test.js", src3.to_string()),
        span: (27, 1),
        help: "Add semicolon after variable declaration".to_string(),
        fix: Some("  function test() {\n-   let x = 5\n+   let x = 5;\n    return x;\n  }".to_string()),
    };

    println!("\nExample 3: Diff with context lines\n");
    let mut out3 = String::new();
    GraphicalReportHandler::new()
        .with_width(80)
        .render_report(&mut out3, &diag3)
        .unwrap();
    println!("{}", out3);

    // Example 4: Large multi-line diff
    #[derive(Error, Debug, Diagnostic)]
    #[error("Refactoring needed")]
    struct RefactorError {
        #[source_code]
        src: NamedSource<String>,
        #[label("old pattern")]
        span: (usize, usize),
        #[help]
        help: String,
        #[fix_diff]
        fix: Option<String>,
    }

    let src4 = "if (x) {\n  doSomething();\n  doMore();\n}";
    let diag4 = RefactorError {
        src: NamedSource::new("refactor.js", src4.to_string()),
        span: (0, src4.len()),
        help: "Use guard clause pattern".to_string(),
        fix: Some("- if (x) {\n-   doSomething();\n-   doMore();\n- }\n+ if (!x) return;\n+ doSomething();\n+ doMore();".to_string()),
    };

    println!("\nExample 4: Large multi-line refactor\n");
    let mut out4 = String::new();
    GraphicalReportHandler::new()
        .with_width(80)
        .render_report(&mut out4, &diag4)
        .unwrap();
    println!("{}", out4);

    // Example 5: Mixed additions and removals
    #[derive(Error, Debug, Diagnostic)]
    #[error("Import reorganization")]
    struct ImportError {
        #[source_code]
        src: NamedSource<String>,
        #[label("imports here")]
        span: (usize, usize),
        #[help]
        help: String,
        #[fix_diff]
        fix: Option<String>,
    }

    let src5 = "import { b } from 'b';\nimport { a } from 'a';\nimport { c } from 'c';";
    let diag5 = ImportError {
        src: NamedSource::new("imports.ts", src5.to_string()),
        span: (0, src5.len()),
        help: "Sort imports alphabetically".to_string(),
        fix: Some("+ import { a } from 'a';\n  import { b } from 'b';\n- import { a } from 'a';\n  import { c } from 'c';".to_string()),
    };

    println!("\nExample 5: Mixed additions and removals\n");
    let mut out5 = String::new();
    GraphicalReportHandler::new()
        .with_width(80)
        .render_report(&mut out5, &diag5)
        .unwrap();
    println!("{}", out5);
}
