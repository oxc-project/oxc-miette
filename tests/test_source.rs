use std::{
    error::Error as StdError,
    fmt::{self, Display},
    io,
};

use miette::{Report, miette};

#[derive(Debug)]
enum TestError {
    Io(io::Error),
}

impl Display for TestError {
    fn fmt(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        match self {
            TestError::Io(e) => Display::fmt(e, formatter),
        }
    }
}

impl StdError for TestError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            TestError::Io(io) => Some(io),
        }
    }
}

#[test]
fn test_literal_source() {
    let error: Report = miette!("oh no!");
    assert!(error.source().is_none());
}

#[test]
fn test_variable_source() {
    let msg = "oh no!";
    let error = miette!(msg);
    assert!(error.source().is_none());

    let msg = msg.to_owned();
    let error: Report = miette!(msg);
    assert!(error.source().is_none());
}

#[test]
fn test_fmt_source() {
    let error: Report = miette!("{} {}!", "oh", "no");
    assert!(error.source().is_none());
}

#[test]
#[ignore = "Again with the io::Error source issue?"]
fn test_io_source() {
    let io = io::Error::other("oh no!");
    let error: Report = miette!(TestError::Io(io));
    assert_eq!("oh no!", error.source().unwrap().to_string());
}

#[test]
fn test_miette_from_miette() {
    let error: Report = miette!("oh no!").wrap_err("context");
    let error = miette!(error);
    assert_eq!("oh no!", error.source().unwrap().to_string());
}
