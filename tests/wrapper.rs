//! Tests for source-code wrapping on `Report` (`with_source_code`). Moved out
//! of `src/wrap_err.rs`.

use thiserror::Error;

use miette::{Diagnostic, LabeledSpan, Report, SourceCode, SourceSpan, SpanContents};

#[derive(Error, Debug)]
#[error("inner")]
struct Inner {
    pub(crate) at: SourceSpan,
    pub(crate) source_code: Option<String>,
}

impl Diagnostic for Inner {
    fn labels(&self) -> miette::Labels {
        miette::Labels::One([LabeledSpan::underline(self.at)])
    }

    fn source_code(&self) -> Option<&dyn SourceCode> {
        self.source_code.as_ref().map(|s| s as _)
    }
}

#[derive(Error, Debug)]
#[error("outer")]
#[cfg_attr(not(feature = "fancy"), expect(dead_code))]
struct Outer {
    pub(crate) errors: Vec<Inner>,
}

impl Diagnostic for Outer {
    fn related(&self) -> miette::Related<'_> {
        self.errors.iter().map(|e| e as &dyn Diagnostic).collect()
    }
}

#[test]
fn no_override() {
    let inner_source = "hello world";
    let outer_source = "abc";

    let report =
        Report::from(Inner { at: (0..5).into(), source_code: Some(inner_source.to_string()) })
            .with_source_code(outer_source.to_string());

    let underlined = String::from_utf8(
        report.source_code().unwrap().read_span(&(0..5).into(), 0, 0).unwrap().data().to_vec(),
    )
    .unwrap();
    assert_eq!(underlined, "hello");
}

#[test]
#[cfg(feature = "fancy")]
fn two_source_codes() {
    let inner_source = "hello world";
    let outer_source = "abc";

    let report = Report::from(Outer {
        errors: vec![
            Inner { at: (0..5).into(), source_code: Some(inner_source.to_string()) },
            Inner { at: (1..2).into(), source_code: None },
        ],
    })
    .with_source_code(outer_source.to_string());

    let message = format!("{report:?}");
    assert!(message.contains(inner_source));
    assert!(message.contains(outer_source));
}
