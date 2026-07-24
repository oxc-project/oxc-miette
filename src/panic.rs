use std::io::Write as _;
use std::{env, fmt::Write, mem::size_of, panic::set_hook};

use backtrace::Backtrace;
use thiserror::Error;

use crate::{self as miette, Diagnostic, Report};

/// Tells miette to render panics using its rendering engine.
pub fn set_panic_hook() {
    set_hook(Box::new(move |info| {
        let payload = info.payload();
        let mut message = payload
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| payload.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("Something went wrong")
            .to_owned();
        if let Some(loc) = info.location() {
            let _ = write!(message, "\n\tat {}:{}:{}", loc.file(), loc.line(), loc.column());
        }
        let report: Report = Panic(message).into();
        let _ = writeln!(std::io::stderr().lock(), "Error: {report:?}");
    }));
}

#[derive(Debug, Error, Diagnostic)]
#[error("{0}{trace}", trace = Panic::backtrace())]
#[diagnostic(help("set the `RUST_BACKTRACE=1` environment variable to display a backtrace."))]
struct Panic(String);

impl Panic {
    fn backtrace() -> String {
        use Write;

        const HEX_WIDTH: usize = size_of::<usize>() + 2;
        // Padding for next lines after frame's address
        const NEXT_SYMBOL_PADDING: usize = HEX_WIDTH + 6;

        if !env::var("RUST_BACKTRACE").is_ok_and(|var| !var.is_empty() && var != "0") {
            return String::new();
        }

        let mut backtrace = String::new();
        let trace = Backtrace::new();
        let frames = backtrace_ext::short_frames_strict(&trace).enumerate();
        for (idx, (frame, sub_frames)) in frames {
            let ip = frame.ip();
            let _ = write!(backtrace, "\n{idx:4}: {ip:HEX_WIDTH$?}");

            let symbols = frame.symbols();
            if symbols.is_empty() {
                let _ = write!(backtrace, " - <unresolved>");
                continue;
            }

            for (idx, symbol) in symbols[sub_frames].iter().enumerate() {
                // Print symbols from this address,
                // if there are several addresses
                // we need to put it on next line
                if idx != 0 {
                    let _ = write!(backtrace, "\n{:1$}", "", NEXT_SYMBOL_PADDING);
                }

                if let Some(name) = symbol.name() {
                    let _ = write!(backtrace, " - {name}");
                } else {
                    let _ = write!(backtrace, " - <unknown>");
                }

                // See if there is debug information with file name and line
                if let (Some(file), Some(line)) = (symbol.filename(), symbol.lineno()) {
                    let _ = write!(
                        backtrace,
                        "\n{:3$}at {}:{}",
                        "",
                        file.display(),
                        line,
                        NEXT_SYMBOL_PADDING
                    );
                }
            }
        }
        backtrace
    }
}
