//! Why talking to the GNOME Shell extension failed.
//!
//! Written by hand rather than with `thiserror`: see `docs/adr/0005-errors-without-thiserror.md`.

use std::{error::Error, fmt};

use vivid_core::BackendError;

/// The ways the session bus, Mutter or the extension can fail.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GnomeError {
    /// There is no session bus, or the connection to it failed.
    Bus(String),
    /// Mutter does not answer: this is not a GNOME session.
    NoMutter(String),
    /// The extension is not running (not installed, not enabled, or the session was not restarted).
    NoExtension(String),
    /// The extension speaks another interface version than this build.
    VersionMismatch {
        /// What the extension reports.
        found:     u32,
        /// What this build understands.
        supported: u32,
    },
    /// The extension refused a call (unknown connector, saturation out of range).
    Rejected(String),
    /// A reply did not have the shape the interface promises.
    BadReply(String),
    /// Installing or removing the extension failed (files, or the `gsettings` command).
    Io(String),
}

impl fmt::Display for GnomeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bus(reason) => write!(f, "cannot reach the session bus: {reason}"),
            Self::NoMutter(reason) => write!(f, "Mutter does not answer (not a GNOME session?): {reason}"),
            Self::NoExtension(reason) => write!(
                f,
                "the vividctl GNOME Shell extension is not running ({reason}); run `vividctl extension install` and log in again"
            ),
            Self::VersionMismatch { found, supported } => write!(
                f,
                "the extension speaks interface version {found}, this vividctl understands {supported}; update the extension"
            ),
            Self::Rejected(reason) => write!(f, "the extension refused the request: {reason}"),
            Self::BadReply(reason) => write!(f, "unexpected reply: {reason}"),
            Self::Io(reason) => f.write_str(reason),
        }
    }
}

impl Error for GnomeError {}

impl From<GnomeError> for BackendError {
    fn from(error: GnomeError) -> Self {
        let reason: String = error.to_string();
        match error {
            GnomeError::NoMutter(_) | GnomeError::NoExtension(_) | GnomeError::VersionMismatch { .. } => {
                Self::Unavailable(reason)
            }
            GnomeError::Rejected(_) => Self::Rejected(reason),
            GnomeError::Bus(_) | GnomeError::BadReply(_) | GnomeError::Io(_) => Self::Io(reason),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_extension_tells_the_user_what_to_do() {
        let message: String = GnomeError::NoExtension("name not owned".into()).to_string();
        assert!(message.contains("vividctl extension install") && message.contains("log in again"));
    }

    #[test]
    fn errors_map_to_the_backend_kind_the_engine_reacts_to() {
        let kind = |e: GnomeError| BackendError::from(e);
        assert!(matches!(kind(GnomeError::NoMutter("x".into())), BackendError::Unavailable(_)));
        assert!(matches!(
            kind(GnomeError::NoExtension("x".into())),
            BackendError::Unavailable(_)
        ));
        assert!(matches!(
            kind(GnomeError::VersionMismatch {
                found:     2,
                supported: 1,
            }),
            BackendError::Unavailable(_)
        ));
        assert!(matches!(kind(GnomeError::Rejected("x".into())), BackendError::Rejected(_)));
        assert!(matches!(kind(GnomeError::Bus("x".into())), BackendError::Io(_)));
        assert!(matches!(kind(GnomeError::BadReply("x".into())), BackendError::Io(_)));
        assert!(matches!(kind(GnomeError::Io("x".into())), BackendError::Io(_)));
    }
}
