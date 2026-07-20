//! Tests for `MietteDiagnostic` (de)serialization. Moved out of
//! `src/miette_diagnostic.rs`.
#![cfg(feature = "serde")]

use miette::{LabeledSpan, Severity};

#[test]
fn test_serialize_miette_diagnostic() {
    use serde_json::json;

    use miette::diagnostic;

    let diag = diagnostic!("message");
    let json = json!({ "message": "message" });
    assert_eq!(json!(diag), json);

    let diag = diagnostic!(
        code = "code",
        help = "help",
        url = "url",
        labels = [LabeledSpan::at_offset(0, "label1"), LabeledSpan::at(1..3, "label2")],
        severity = Severity::Warning,
        "message"
    );
    let json = json!({
        "message": "message",
        "code": "code",
        "help": "help",
        "url": "url",
        "severity": "Warning",
        "labels": [
            {
                "span": {
                    "offset": 0,
                    "length": 0
                },
                "label": "label1",
                "primary": false
            },
            {
                "span": {
                    "offset": 1,
                    "length": 2
                },
                "label": "label2",
                "primary": false
            }
        ]
    });
    assert_eq!(json!(diag), json);
}
#[test]
fn test_deserialize_miette_diagnostic() {
    use serde_json::json;

    use miette::diagnostic;

    let json = json!({ "message": "message" });
    let diag = diagnostic!("message");
    assert_eq!(diag, serde_json::from_value(json).unwrap());

    let json = json!({
        "message": "message",
        "help": null,
        "code": null,
        "severity": null,
        "url": null,
        "labels": null
    });
    assert_eq!(diag, serde_json::from_value(json).unwrap());

    let diag = diagnostic!(
        code = "code",
        help = "help",
        url = "url",
        labels = [LabeledSpan::at_offset(0, "label1"), LabeledSpan::at(1..3, "label2")],
        severity = Severity::Warning,
        "message"
    );
    let json = json!({
        "message": "message",
        "code": "code",
        "help": "help",
        "url": "url",
        "severity": "Warning",
        "labels": [
            {
                "span": {
                    "offset": 0,
                    "length": 0
                },
                "label": "label1",
                "primary": false
            },
            {
                "span": {
                    "offset": 1,
                    "length": 2
                },
                "label": "label2",
                "primary": false
            }
        ]
    });
    assert_eq!(diag, serde_json::from_value(json).unwrap());
}
