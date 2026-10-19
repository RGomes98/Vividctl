//! Error types of the shared domain model.
//!
//! Written by hand rather than with `thiserror`: see `docs/adr/0005-errors-without-thiserror.md`.

use std::{error::Error, fmt};

use crate::{OutputHandle, Vibrance};

/// A number is outside the Digital Vibrance range.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VibranceOutOfRange(pub i64);

impl fmt::Display for VibranceOutOfRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "vibrance {} is outside {}..={}", self.0, Vibrance::MIN, Vibrance::MAX)
    }
}

impl Error for VibranceOutOfRange {}

/// Why bytes could not be read as an EDID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum EdidError {
    /// Fewer than the 128 bytes of the base block.
    TooShort {
        /// How many bytes there were.
        len: usize,
    },
    /// The fixed 8-byte header is missing: these bytes are not an EDID.
    BadHeader,
}

impl fmt::Display for EdidError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooShort { len } => write!(f, "EDID too short: {len} bytes, need 128"),
            Self::BadHeader => f.write_str("not an EDID: the fixed header is missing"),
        }
    }
}

impl Error for EdidError {}

/// Why a backend could not do what it was asked.
///
/// Backends convert their own failures into one of these, so the engine and the frontends can
/// react to the kind of failure without knowing the backend.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BackendError {
    /// The backend cannot run on this system (driver not loaded, no compositor extension, ...).
    Unavailable(String),
    /// The backend runs, but cannot handle this request (output not driven by it, unknown layout).
    Unsupported(String),
    /// The output handle does not belong to the backend, or the output is gone.
    UnknownOutput(OutputHandle),
    /// The system refused the request.
    Rejected(String),
    /// A value was written but reading it back gave something else.
    ReadbackMismatch {
        /// What was written.
        expected: Vibrance,
        /// What the readback returned.
        actual:   Vibrance,
    },
    /// Talking to the system failed (device, bus, file).
    Io(String),
}

impl fmt::Display for BackendError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(reason) => write!(f, "backend unavailable: {reason}"),
            Self::Unsupported(reason) => write!(f, "not supported: {reason}"),
            Self::UnknownOutput(handle) => write!(f, "unknown output {handle}"),
            Self::Rejected(reason) => write!(f, "request rejected: {reason}"),
            Self::ReadbackMismatch { expected, actual } => {
                write!(f, "wrote vibrance {expected} but read back {actual}")
            }
            Self::Io(reason) => write!(f, "i/o error: {reason}"),
        }
    }
}

impl Error for BackendError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_name_the_problem() {
        assert_eq!(VibranceOutOfRange(2000).to_string(), "vibrance 2000 is outside -1024..=1023");
        assert_eq!(
            BackendError::Unavailable("module not loaded".into()).to_string(),
            "backend unavailable: module not loaded"
        );
        assert_eq!(
            BackendError::ReadbackMismatch {
                expected: Vibrance::NEUTRAL,
                actual:   Vibrance::MAX,
            }
            .to_string(),
            "wrote vibrance 0 but read back 1023"
        );
    }

    #[test]
    fn errors_are_std_errors() {
        fn assert_error<E: Error>() {}
        assert_error::<VibranceOutOfRange>();
        assert_error::<EdidError>();
        assert_error::<BackendError>();
    }
}
