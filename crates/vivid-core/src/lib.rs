//! Domain types, error model and the `Backend` trait shared by every vividctl crate.
//!
//! This crate has no I/O and no dependencies. It defines:
//!
//! - [`Vibrance`], the validated Digital Vibrance value
//! - [`OutputId`] and [`MonitorSpec`], the identity of a monitor (ADR-0003), and [`assign_ids`]
//! - [`Profile`] and [`Setting`], what to apply to which monitor
//! - [`Backend`], what NVKMS and the GNOME Shell extension implement, with [`BackendOutput`],
//!   [`OutputHandle`] and [`Capabilities`]
//! - the error types [`BackendError`] and [`VibranceOutOfRange`]

#![forbid(unsafe_code)]

mod backend;
mod error;
mod output;
mod profile;
mod vibrance;

pub use backend::{Backend, BackendOutput, Capabilities, OutputHandle};
pub use error::{BackendError, VibranceOutOfRange};
pub use output::{MonitorSpec, OutputId, SerialQuality, assign_ids};
pub use profile::{Profile, Setting};
pub use vibrance::Vibrance;
