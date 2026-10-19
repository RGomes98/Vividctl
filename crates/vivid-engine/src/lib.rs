//! Use cases shared by the command line and the GUI: list the outputs of every backend, name a
//! monitor, read and change its vibrance.
//!
//! - [`Engine`] holds the backends ([`Slot`]) and acts on their outputs
//! - [`Listing`] and [`Entry`] are the outputs found, with the identity of the connected ones
//! - [`Selector`] is how the user names a monitor: by identity or by connector name
//! - [`Outcome`] is the result for one output; an action on several never stops at the first failure

#![forbid(unsafe_code)]

mod engine;
mod error;
mod selector;

pub use engine::{BackendReport, Engine, Entry, Listing, Outcome, Problem, Slot};
pub use error::SelectError;
pub use selector::Selector;
