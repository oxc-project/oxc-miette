use std::fmt;

/**
Error enum for miette. Used by certain operations in the protocol.
*/
#[derive(Debug)]
pub enum MietteError {
    /// Returned when a [`SourceSpan`](crate::SourceSpan) extends beyond the
    /// bounds of a given [`SourceCode`](crate::SourceCode).
    OutOfBounds,
}

impl fmt::Display for MietteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("The given offset is outside the bounds of its Source")
    }
}

impl std::error::Error for MietteError {}
