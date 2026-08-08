use std::fmt;

use miette::{Diagnostic, SourceSpan};

#[derive(Debug)]
struct TestDiagnostic;

impl fmt::Display for TestDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("broken")
    }
}

impl std::error::Error for TestDiagnostic {}
impl Diagnostic for TestDiagnostic {}

#[test]
fn spans_convert_from_offsets_and_ranges() {
    assert_eq!(SourceSpan::from((3, 4)).offset(), 3);
    assert_eq!(SourceSpan::from((3, 4)).len(), 4);
    assert_eq!(SourceSpan::from(3..7), SourceSpan::from((3, 4)));
}

#[test]
fn diagnostics_can_be_owned_as_trait_objects() {
    let diagnostic: Box<dyn Diagnostic + Send + Sync> = Box::new(TestDiagnostic);
    assert_eq!(diagnostic.to_string(), "broken");
}
