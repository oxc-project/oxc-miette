use std::io;

use miette::{Result, WrapErr, bail};

fn f() -> Result<()> {
    bail!(io::Error::new(io::ErrorKind::PermissionDenied, "oh no!"));
}

fn g() -> Result<()> {
    f().wrap_err("f failed")
}

fn h() -> Result<()> {
    g().wrap_err("g failed")
}

const EXPECTED_ALTDISPLAY_F: &str = "oh no!";

const EXPECTED_ALTDISPLAY_G: &str = "f failed: oh no!";

const EXPECTED_ALTDISPLAY_H: &str = "g failed: f failed: oh no!";

const EXPECTED_DEBUG_F: &str = "oh no!";

const EXPECTED_DEBUG_G: &str = "\
f failed

Caused by:
    oh no!\
";

const EXPECTED_DEBUG_H: &str = "\
g failed

Caused by:
   0: f failed
   1: oh no!\
";

const EXPECTED_ALTDEBUG_F: &str = "\
Custom {
    kind: PermissionDenied,
    error: \"oh no!\",
}\
";

const EXPECTED_ALTDEBUG_G: &str = "\
Error {
    msg: \"f failed\",
    source: Custom {
        kind: PermissionDenied,
        error: \"oh no!\",
    },
}\
";

const EXPECTED_ALTDEBUG_H: &str = "\
Error {
    msg: \"g failed\",
    source: Error {
        msg: \"f failed\",
        source: Custom {
            kind: PermissionDenied,
            error: \"oh no!\",
        },
    },
}\
";

#[test]
fn test_display() {
    assert_eq!("g failed", h().unwrap_err().to_string());
}

#[test]
fn test_altdisplay() {
    assert_eq!(EXPECTED_ALTDISPLAY_F, format!("{:#}", f().unwrap_err()));
    assert_eq!(EXPECTED_ALTDISPLAY_G, format!("{:#}", g().unwrap_err()));
    assert_eq!(EXPECTED_ALTDISPLAY_H, format!("{:#}", h().unwrap_err()));
}

#[test]
#[ignore = "not really gonna work with the current printers"]
fn test_debug() {
    assert_eq!(EXPECTED_DEBUG_F, format!("{:?}", f().unwrap_err()));
    assert_eq!(EXPECTED_DEBUG_G, format!("{:?}", g().unwrap_err()));
    assert_eq!(EXPECTED_DEBUG_H, format!("{:?}", h().unwrap_err()));
}

#[test]
fn test_altdebug() {
    assert_eq!(EXPECTED_ALTDEBUG_F, format!("{:#?}", f().unwrap_err()));
    assert_eq!(EXPECTED_ALTDEBUG_G, format!("{:#?}", g().unwrap_err()));
    assert_eq!(EXPECTED_ALTDEBUG_H, format!("{:#?}", h().unwrap_err()));
}
