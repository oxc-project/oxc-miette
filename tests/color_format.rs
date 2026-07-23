#![cfg(all(feature = "fancy-no-backtrace", not(feature = "fancy-no-syscall"), not(miri),))]

use std::{
    fmt::{self, Debug},
    process::Command,
};

use miette::{Diagnostic, MietteHandler, MietteHandlerOpts, ReportHandler, RgbColors};
use regex::Regex;
use thiserror::Error;

#[derive(Eq, PartialEq, Debug)]
enum ColorFormat {
    NoColor,
    Ansi,
    Rgb,
}

#[derive(Clone, Copy)]
enum HandlerOptions {
    Default,
    ColorNever,
    ColorAlways,
    RgbPreferred,
    RgbAlways,
    ColorAlwaysRgbAlways,
}

impl HandlerOptions {
    fn apply(self, options: MietteHandlerOpts) -> MietteHandlerOpts {
        match self {
            HandlerOptions::Default => options,
            HandlerOptions::ColorNever => options.color(false),
            HandlerOptions::ColorAlways => options.color(true),
            HandlerOptions::RgbPreferred => options.rgb_colors(RgbColors::Preferred),
            HandlerOptions::RgbAlways => options.rgb_colors(RgbColors::Always),
            HandlerOptions::ColorAlwaysRgbAlways => {
                options.color(true).rgb_colors(RgbColors::Always)
            }
        }
    }

    const fn name(self) -> &'static str {
        match self {
            HandlerOptions::Default => "default",
            HandlerOptions::ColorNever => "color-never",
            HandlerOptions::ColorAlways => "color-always",
            HandlerOptions::RgbPreferred => "rgb-preferred",
            HandlerOptions::RgbAlways => "rgb-always",
            HandlerOptions::ColorAlwaysRgbAlways => "color-always-rgb-always",
        }
    }

    fn from_name(name: &str) -> Self {
        match name {
            "default" => Self::Default,
            "color-never" => Self::ColorNever,
            "color-always" => Self::ColorAlways,
            "rgb-preferred" => Self::RgbPreferred,
            "rgb-always" => Self::RgbAlways,
            "color-always-rgb-always" => Self::ColorAlwaysRgbAlways,
            _ => panic!("unknown handler options: {name}"),
        }
    }
}

#[derive(Debug, Diagnostic, Error)]
#[error("oops!")]
#[diagnostic(code(oops::my::bad), help("try doing it better next time?"))]
struct MyBad;

struct FormatTester(MietteHandler);

impl Debug for FormatTester {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.0.debug(&MyBad, f)
    }
}

/// Check the color format used by a handler.
fn color_format(handler: MietteHandler) -> ColorFormat {
    let out = format!("{:?}", FormatTester(handler));

    let rgb_colors = Regex::new(r"\u{1b}\[[34]8;2;").unwrap();
    let ansi_colors = Regex::new(r"\u{1b}\[(3|4|9|10)[0-7][m;]").unwrap();
    if rgb_colors.is_match(&out) {
        ColorFormat::Rgb
    } else if ansi_colors.is_match(&out) {
        ColorFormat::Ansi
    } else {
        ColorFormat::NoColor
    }
}

const CHILD_OPTIONS: &str = "MIETTE_TEST_HANDLER_OPTIONS";
const CHILD_RESULT: &str = "MIETTE_TEST_COLOR_FORMAT=";

fn color_format_in_child(
    options: HandlerOptions,
    no_color: Option<&str>,
    force_color: Option<&str>,
) -> ColorFormat {
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "report_color_format", "--nocapture"])
        .env(CHILD_OPTIONS, options.name())
        .env_remove("NO_COLOR")
        .env_remove("FORCE_COLOR");
    if let Some(value) = no_color {
        command.env("NO_COLOR", value);
    }
    if let Some(value) = force_color {
        command.env("FORCE_COLOR", value);
    }

    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "child failed:\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    if stdout.contains(&format!("{CHILD_RESULT}Rgb")) {
        ColorFormat::Rgb
    } else if stdout.contains(&format!("{CHILD_RESULT}Ansi")) {
        ColorFormat::Ansi
    } else if stdout.contains(&format!("{CHILD_RESULT}NoColor")) {
        ColorFormat::NoColor
    } else {
        panic!("child did not report a color format:\n{stdout}")
    }
}

#[test]
fn report_color_format() {
    let Ok(options) = std::env::var(CHILD_OPTIONS) else {
        return;
    };
    let handler = HandlerOptions::from_name(&options).apply(MietteHandlerOpts::new()).build();
    println!("{CHILD_RESULT}{:?}", color_format(handler));
}

/// Assert the color format used by a handler with different levels of terminal
/// support.
fn check_colors(
    options: HandlerOptions,
    no_support: ColorFormat,
    ansi_support: ColorFormat,
    rgb_support: ColorFormat,
) {
    assert_eq!(color_format_in_child(options, Some("1"), None), no_support);
    assert_eq!(color_format_in_child(options, None, Some("1")), ansi_support);
    assert_eq!(color_format_in_child(options, None, Some("3")), rgb_support);
}

#[test]
fn no_color_preference() {
    use ColorFormat::*;
    check_colors(HandlerOptions::Default, NoColor, Ansi, Ansi);
}

#[test]
fn color_never() {
    use ColorFormat::*;
    check_colors(HandlerOptions::ColorNever, NoColor, NoColor, NoColor);
}

#[test]
fn color_always() {
    use ColorFormat::*;
    check_colors(HandlerOptions::ColorAlways, Ansi, Ansi, Ansi);
}

#[test]
fn rgb_preferred() {
    use ColorFormat::*;
    check_colors(HandlerOptions::RgbPreferred, NoColor, Ansi, Rgb);
}

#[test]
fn rgb_always() {
    use ColorFormat::*;
    check_colors(HandlerOptions::RgbAlways, NoColor, Rgb, Rgb);
}

#[test]
fn color_always_rgb_always() {
    use ColorFormat::*;
    check_colors(HandlerOptions::ColorAlwaysRgbAlways, Rgb, Rgb, Rgb);
}
