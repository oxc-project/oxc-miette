#![allow(clippy::print_stdout, clippy::unnecessary_wraps)]

mod json_handler {
    use miette::{Diagnostic, JSONReportHandler, MietteDiagnostic, MietteError};

    fn fmt_report(diag: &dyn Diagnostic) -> String {
        let mut out = String::new();
        JSONReportHandler::new().render_report(&mut out, diag).unwrap();
        out
    }

    #[test]
    fn note_appears_in_json_output() -> Result<(), MietteError> {
        let diag =
            MietteDiagnostic::new("oops!").with_code("oops::my::bad").with_note("this is a note");

        let out = fmt_report(&diag);
        println!("Error: {out}");
        assert!(out.contains(r#""note": "this is a note""#));
        Ok(())
    }

    #[test]
    fn note_and_help_both_in_json() -> Result<(), MietteError> {
        let diag = MietteDiagnostic::new("oops!").with_help("try this").with_note("note context");

        let out = fmt_report(&diag);
        println!("Error: {out}");
        assert!(out.contains(r#""help": "try this""#));
        assert!(out.contains(r#""note": "note context""#));
        Ok(())
    }

    #[test]
    fn note_with_special_chars_escaped_in_json() -> Result<(), MietteError> {
        let diag = MietteDiagnostic::new("oops!").with_note(r#"note with "quotes" and \backslash"#);

        let out = fmt_report(&diag);
        println!("Error: {out}");
        assert!(out.contains("note"));
        Ok(())
    }

    #[test]
    fn no_note_field_when_absent() -> Result<(), MietteError> {
        let diag = MietteDiagnostic::new("oops!");

        let out = fmt_report(&diag);
        println!("Error: {out}");
        assert!(!out.contains(r#""note""#));
        Ok(())
    }

    #[test]
    fn note_with_code() -> Result<(), MietteError> {
        let diag = MietteDiagnostic::new("oops!")
            .with_code("oops::my::bad")
            .with_note("this is a note about the error");

        let out = fmt_report(&diag);
        println!("Error: {out}");
        assert!(out.contains(r#""note": "this is a note about the error""#));
        assert!(out.contains(r#""code": "oops::my::bad""#));
        Ok(())
    }
}

mod miette_diagnostic_tests {
    use miette::{Diagnostic, MietteDiagnostic};

    #[test]
    fn note_field_none_by_default() {
        let diag = MietteDiagnostic::new("test message");
        assert_eq!(diag.note, None);
    }

    #[test]
    fn with_note_sets_field() {
        let diag = MietteDiagnostic::new("test message").with_note("test note");
        assert_eq!(diag.note, Some("test note".to_string()));
    }

    #[test]
    fn with_note_returns_self() {
        let diag = MietteDiagnostic::new("test message").with_note("note 1").with_help("help text");
        assert_eq!(diag.note, Some("note 1".to_string()));
        assert_eq!(diag.help, Some("help text".to_string()));
    }

    #[test]
    fn note_builder_accepts_into_string() {
        let note_str = "note text";
        let diag = MietteDiagnostic::new("test").with_note(note_str);
        assert_eq!(diag.note, Some("note text".to_string()));

        let diag2 = MietteDiagnostic::new("test").with_note("note".to_string());
        assert_eq!(diag2.note, Some("note".to_string()));
    }

    #[test]
    fn trait_method_note_returns_some() {
        let diag = MietteDiagnostic::new("test message").with_note("test note");
        let note = diag.note();
        assert!(note.is_some());
        assert_eq!(note.unwrap().to_string(), "test note");
    }

    #[test]
    fn trait_method_note_returns_none() {
        let diag = MietteDiagnostic::new("test message");
        let note = diag.note();
        assert!(note.is_none());
    }

    #[test]
    fn note_with_help_both_accessible() {
        let diag = MietteDiagnostic::new("test").with_help("help text").with_note("note text");

        let help = diag.help();
        let note = diag.note();

        assert_eq!(help.unwrap().to_string(), "help text");
        assert_eq!(note.unwrap().to_string(), "note text");
    }

    #[test]
    fn note_with_all_fields() {
        let diag = MietteDiagnostic::new("message")
            .with_code("E001")
            .with_severity(miette::Severity::Error)
            .with_help("help")
            .with_note("note")
            .with_url("https://example.com");

        assert_eq!(diag.message, "message");
        assert_eq!(diag.code, Some("E001".to_string()));
        assert_eq!(diag.help, Some("help".to_string()));
        assert_eq!(diag.note, Some("note".to_string()));
        assert_eq!(diag.url, Some("https://example.com".to_string()));
    }

    #[test]
    #[cfg(feature = "serde")]
    fn note_serializes_to_json() {
        use serde_json::json;

        let diag = MietteDiagnostic::new("message").with_note("note text");
        let json = serde_json::to_value(&diag).unwrap();

        assert_eq!(json["message"], "message");
        assert_eq!(json["note"], "note text");
    }

    #[test]
    #[cfg(feature = "serde")]
    fn note_absent_skips_in_serde() {
        use serde_json::json;

        let diag = MietteDiagnostic::new("message");
        let json = serde_json::to_value(&diag).unwrap();

        assert!(!json.as_object().unwrap().contains_key("note"));
    }

    #[test]
    #[cfg(feature = "serde")]
    fn note_deserializes_from_json() {
        use serde_json::json;

        let json = json!({
            "message": "test",
            "note": "note text"
        });

        let diag: MietteDiagnostic = serde_json::from_value(json).unwrap();
        assert_eq!(diag.message, "test");
        assert_eq!(diag.note, Some("note text".to_string()));
    }
}

mod debug_handler_tests {
    use miette::MietteDiagnostic;
    use std::fmt::Write;

    #[test]
    fn debug_output_includes_note() {
        let diag = MietteDiagnostic::new("test message").with_note("test note");

        let mut out = String::new();
        write!(&mut out, "{:?}", diag).unwrap();

        assert!(out.contains("note"));
        assert!(out.contains("test note"));
    }

    #[test]
    fn debug_output_with_only_message() {
        let diag = MietteDiagnostic::new("test message");

        let mut out = String::new();
        write!(&mut out, "{:?}", diag).unwrap();

        assert!(out.contains("test message"));
    }

    #[test]
    fn debug_output_with_note_and_help() {
        let diag = MietteDiagnostic::new("message").with_help("help text").with_note("note text");

        let mut out = String::new();
        write!(&mut out, "{:?}", diag).unwrap();

        assert!(out.contains("help"));
        assert!(out.contains("help text"));
        assert!(out.contains("note"));
        assert!(out.contains("note text"));
    }
}

mod trait_implementation_tests {
    use miette::{Diagnostic, MietteDiagnostic};

    #[test]
    fn diagnostic_trait_note_method_exists() {
        let diag = MietteDiagnostic::new("test");
        let _note: Option<Box<dyn std::fmt::Display>> = diag.note();
    }

    #[test]
    fn note_method_is_boxed_display() {
        let diag = MietteDiagnostic::new("test").with_note("note");
        if let Some(note) = diag.note() {
            let note_str = note.to_string();
            assert_eq!(note_str, "note");
        } else {
            panic!("Expected Some(note)");
        }
    }

    #[test]
    fn default_note_implementation() {
        let diag = MietteDiagnostic::new("test");
        assert!(diag.note().is_none());
    }
}

mod integration_tests {
    use miette::{Diagnostic, MietteDiagnostic};

    #[test]
    fn note_visible_through_error_trait() {
        let diag: Box<dyn Diagnostic> =
            Box::new(MietteDiagnostic::new("test").with_note("integration note"));

        if let Some(note) = diag.note() {
            assert_eq!(note.to_string(), "integration note");
        } else {
            panic!("Expected note to be accessible through trait object");
        }
    }

    #[test]
    fn multiple_diagnostics_independent_notes() {
        let diag1 = MietteDiagnostic::new("error1").with_note("note1");
        let diag2 = MietteDiagnostic::new("error2").with_note("note2");
        let diag3 = MietteDiagnostic::new("error3");

        assert_eq!(diag1.note, Some("note1".to_string()));
        assert_eq!(diag2.note, Some("note2".to_string()));
        assert_eq!(diag3.note, None);
    }

    #[test]
    fn note_with_unicode() {
        let note = "This note contains unicode: 🎉 ✨ 🦀";
        let diag = MietteDiagnostic::new("test").with_note(note);

        assert_eq!(diag.note, Some(note.to_string()));
    }

    #[test]
    fn note_with_multiline_text() {
        let note = "Line 1\nLine 2\nLine 3";
        let diag = MietteDiagnostic::new("test").with_note(note);

        assert_eq!(diag.note, Some(note.to_string()));
    }
}
