use std::{borrow::Cow, io};

use thiserror::Error;

use crate::Diagnostic;

/**
Error enum for miette. Used by certain operations in the protocol.
*/
#[derive(Debug, Error)]
pub enum MietteError {
    /// Wrapper around [`std::io::Error`]. This is returned when something went
    /// wrong while reading a [`SourceCode`](crate::SourceCode).
    #[error(transparent)]
    IoError(#[from] io::Error),

    /// Returned when a [`SourceSpan`](crate::SourceSpan) extends beyond the
    /// bounds of a given [`SourceCode`](crate::SourceCode).
    #[error("The given offset is outside the bounds of its Source")]
    OutOfBounds,
}

impl Diagnostic for MietteError {
    fn code(&self) -> Option<Cow<'_, str>> {
        match self {
            MietteError::IoError(_) => Some(Cow::Borrowed("miette::io_error")),
            MietteError::OutOfBounds => Some(Cow::Borrowed("miette::span_out_of_bounds")),
        }
    }

    fn help(&self) -> Option<Cow<'_, str>> {
        match self {
            MietteError::IoError(_) => None,
            MietteError::OutOfBounds => {
                Some(Cow::Borrowed("Double-check your spans. Do you have an off-by-one error?"))
            }
        }
    }

    fn url(&self) -> Option<Cow<'_, str>> {
        let crate_version = env!("CARGO_PKG_VERSION");
        let variant = match self {
            MietteError::IoError(_) => "#variant.IoError",
            MietteError::OutOfBounds => "#variant.OutOfBounds",
        };
        Some(Cow::Owned(format!(
            "https://docs.rs/miette/{crate_version}/miette/enum.MietteError.html{variant}",
        )))
    }
}
