use std::sync::Arc;

use miette::{NamedSource, SourceCode};

#[test]
fn basic_sources_have_no_name() {
    let source = "Hello, world!";
    assert_eq!(source.name(), None);

    let source = String::from("Hello, world!");
    assert_eq!(source.name(), None);
}

#[test]
fn named_str_source_returns_name() {
    let source = "Hello, world!";
    let named = NamedSource::new("test.txt", source);
    // Call the trait method explicitly through SourceCode trait
    assert_eq!(SourceCode::name(&named), Some("test.txt"));
}

#[test]
fn named_string_source_returns_name() {
    let source = String::from("fn main() {}");
    let named = NamedSource::new("main.rs", source);
    // Call the trait method explicitly through SourceCode trait
    assert_eq!(SourceCode::name(&named), Some("main.rs"));
}

#[test]
fn arc_named_source_returns_name() {
    let source = String::from("fn main() {}");
    let named = Arc::new(NamedSource::new("main.rs", source));
    assert_eq!(SourceCode::name(&named), Some("main.rs"));
    assert_eq!(named.name(), Some("main.rs"));
}
