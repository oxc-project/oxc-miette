use std::fmt;

use miette::{Diagnostic, LabeledSpan, Labels, SourceOffset, SourceSpan};

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
fn labels_store_small_collections_inline() {
    let mut labels = Labels::default();
    labels.push(LabeledSpan::at(1..2, "first"));
    assert!(matches!(labels, Labels::One(_)));
    labels.push(LabeledSpan::at(3..4, "second"));
    assert!(matches!(labels, Labels::Two(_)));
    labels.push(LabeledSpan::at(5..6, "third"));
    assert!(matches!(labels, Labels::Many(_)));
}

#[test]
fn spans_convert_from_offsets_and_ranges() {
    assert_eq!(SourceSpan::from((3, 4)).offset(), 3);
    assert_eq!(SourceSpan::from((3, 4)).len(), 4);
    assert_eq!(SourceSpan::from(3..7), SourceSpan::from((3, 4)));
    assert_eq!(SourceOffset::from_location("a\nb", 2, 1).offset(), 2);
}

#[test]
fn diagnostics_can_be_owned_as_trait_objects() {
    let diagnostic: Box<dyn Diagnostic + Send + Sync> = TestDiagnostic.into();
    assert_eq!(diagnostic.to_string(), "broken");
}
