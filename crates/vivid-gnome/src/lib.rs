//! GNOME Shell compositor backend for outputs the NVIDIA display engine does not drive.
//!
//! This crate is the client side of the `org.vividctl.Shell1` D-Bus interface that
//! `extensions/gnome` exports, plus Mutter's monitor list. It speaks saturation, not vibrance:
//! the mapping between the two is the engine's business (`vivid-color`), so this crate does not
//! depend on it.
//!
//! - [`Transport`], the four calls the backend needs, with [`SessionBus`] as the real thing
//! - [`MutterMonitor`] and [`ExtensionInfo`], what comes back
//! - [`fake::FakeShell`], an in-memory stand-in for tests
//! - [`GnomeError`], which converts into [`vivid_core::BackendError`]

#![forbid(unsafe_code)]

mod bus;
mod error;
pub mod fake;
mod transport;

pub use bus::SessionBus;
pub use error::GnomeError;
pub use transport::{ExtensionInfo, INTERFACE_VERSION, MutterMonitor, Transport};
