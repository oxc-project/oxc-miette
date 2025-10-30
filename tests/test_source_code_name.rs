use miette::{NamedSource, SourceCode};

#[test]
fn test_basic_source_code_name_is_none() {
    let source = "Hello, world!";
    assert_eq!(source.name(), None);

    let source = String::from("Hello, world!");
    assert_eq!(source.name(), None);
}

#[test]
fn test_named_source_returns_name() {
    let source = "Hello, world!";
    let named = NamedSource::new("test.txt", source);
    // Call the trait method explicitly through SourceCode trait
    assert_eq!(SourceCode::name(&named), Some("test.txt"));
}

#[test]
fn test_named_source_with_string() {
    let source = String::from("fn main() {}");
    let named = NamedSource::new("main.rs", source);
    // Call the trait method explicitly through SourceCode trait
    assert_eq!(SourceCode::name(&named), Some("main.rs"));
}
