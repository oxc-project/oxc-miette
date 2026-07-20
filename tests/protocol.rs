//! Tests for the core protocol types. Moved out of `src/protocol.rs` so the
//! source module carries only library code.

use miette::{LabeledSpan, SourceOffset};
#[cfg(feature = "serde")]
use miette::{Severity, SourceSpan};

#[cfg(feature = "serde")]
#[test]
fn test_serialize_severity() {
    use serde_json::json;

    assert_eq!(json!(Severity::Advice), json!("Advice"));
    assert_eq!(json!(Severity::Warning), json!("Warning"));
    assert_eq!(json!(Severity::Error), json!("Error"));
}
#[cfg(feature = "serde")]
#[test]
fn test_deserialize_severity() {
    use serde_json::json;

    let severity: Severity = serde_json::from_value(json!("Advice")).unwrap();
    assert_eq!(severity, Severity::Advice);

    let severity: Severity = serde_json::from_value(json!("Warning")).unwrap();
    assert_eq!(severity, Severity::Warning);

    let severity: Severity = serde_json::from_value(json!("Error")).unwrap();
    assert_eq!(severity, Severity::Error);
}
#[test]
fn test_set_span_offset() {
    let mut span = LabeledSpan::new(None, 10, 10);
    assert_eq!(span.offset(), 10);

    span.set_span_offset(20);
    assert_eq!(span.offset(), 20);
}
#[cfg(feature = "serde")]
#[test]
fn test_serialize_labeled_span() {
    use serde_json::json;

    assert_eq!(
        json!(LabeledSpan::new(None, 0, 0)),
        json!({
            "span": { "offset": 0, "length": 0, },
            "primary": false,
        })
    );

    assert_eq!(
        json!(LabeledSpan::new(Some("label".to_string()), 0, 0)),
        json!({
            "label": "label",
            "span": { "offset": 0, "length": 0, },
            "primary": false,
        })
    );
}
#[cfg(feature = "serde")]
#[test]
fn test_deserialize_labeled_span() {
    use serde_json::json;

    let span: LabeledSpan = serde_json::from_value(json!({
        "label": null,
        "span": { "offset": 0, "length": 0, },
        "primary": false,
    }))
    .unwrap();
    assert_eq!(span, LabeledSpan::new(None, 0, 0));

    let span: LabeledSpan = serde_json::from_value(json!({
        "span": { "offset": 0, "length": 0, },
        "primary": false
    }))
    .unwrap();
    assert_eq!(span, LabeledSpan::new(None, 0, 0));

    let span: LabeledSpan = serde_json::from_value(json!({
        "label": "label",
        "span": { "offset": 0, "length": 0, },
        "primary": false
    }))
    .unwrap();
    assert_eq!(span, LabeledSpan::new(Some("label".to_string()), 0, 0));
}
#[cfg(feature = "serde")]
#[test]
fn test_serialize_source_span() {
    use serde_json::json;

    assert_eq!(json!(SourceSpan::from(0)), json!({ "offset": 0, "length": 0}));
}
#[cfg(feature = "serde")]
#[test]
fn test_deserialize_source_span() {
    use serde_json::json;

    let span: SourceSpan = serde_json::from_value(json!({ "offset": 0, "length": 0})).unwrap();
    assert_eq!(span, SourceSpan::from(0));
}
#[test]
fn test_source_offset_from_location() {
    let source = "f\n\noo\r\nbar";

    assert_eq!(SourceOffset::from_location(source, 1, 1).offset(), 0);
    assert_eq!(SourceOffset::from_location(source, 1, 2).offset(), 1);
    assert_eq!(SourceOffset::from_location(source, 2, 1).offset(), 2);
    assert_eq!(SourceOffset::from_location(source, 3, 1).offset(), 3);
    assert_eq!(SourceOffset::from_location(source, 3, 2).offset(), 4);
    assert_eq!(SourceOffset::from_location(source, 3, 3).offset(), 5);
    assert_eq!(SourceOffset::from_location(source, 3, 4).offset(), 6);
    assert_eq!(SourceOffset::from_location(source, 4, 1).offset(), 7);
    assert_eq!(SourceOffset::from_location(source, 4, 2).offset(), 8);
    assert_eq!(SourceOffset::from_location(source, 4, 3).offset(), 9);
    assert_eq!(SourceOffset::from_location(source, 4, 4).offset(), 10);

    // Out-of-range
    assert_eq!(SourceOffset::from_location(source, 5, 1).offset(), source.len() as u32);
}
#[cfg(feature = "serde")]
#[test]
fn test_serialize_source_offset() {
    use serde_json::json;

    assert_eq!(json!(SourceOffset::from(0)), 0);
}
#[cfg(feature = "serde")]
#[test]
fn test_deserialize_source_offset() {
    let offset: SourceOffset = serde_json::from_str("0").unwrap();
    assert_eq!(offset, SourceOffset::from(0));
}
