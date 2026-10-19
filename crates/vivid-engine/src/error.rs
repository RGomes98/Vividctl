//! Why a request cannot be turned into a set of outputs.

use std::{error::Error, fmt};

/// The user asked for outputs that cannot be resolved.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SelectError {
    /// No output was named and none is connected to a backend that can drive it.
    NoConnectedOutputs,
    /// Nothing matches what the user named.
    NoMatch {
        /// What the user typed.
        selector: String,
    },
    /// What the user named matches more than one output.
    Ambiguous {
        /// What the user typed.
        selector:   String,
        /// The outputs it matches, in the form that names exactly one of them.
        candidates: Vec<String>,
    },
}

impl fmt::Display for SelectError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoConnectedOutputs => {
                f.write_str("no connected output can be controlled; run `vividctl list` to see what was found")
            }
            Self::NoMatch { selector } => {
                write!(f, "no output matches {selector:?}; run `vividctl list` to see the outputs")
            }
            Self::Ambiguous { selector, candidates } => write!(
                f,
                "{selector:?} matches {} outputs ({}); name one of them",
                candidates.len(),
                candidates.join(", ")
            ),
        }
    }
}

impl Error for SelectError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_tell_the_user_what_to_do() {
        assert!(SelectError::NoConnectedOutputs.to_string().contains("vividctl list"));
        assert!(
            SelectError::NoMatch {
                selector: "DP-9".into(),
            }
            .to_string()
            .contains("\"DP-9\"")
        );
        let ambiguous: SelectError = SelectError::Ambiguous {
            selector:   "DEL:U2720Q:0x1".into(),
            candidates: vec!["DP-1".into(), "DP-2".into()],
        };
        assert_eq!(
            ambiguous.to_string(),
            "\"DEL:U2720Q:0x1\" matches 2 outputs (DP-1, DP-2); name one of them"
        );
    }
}
