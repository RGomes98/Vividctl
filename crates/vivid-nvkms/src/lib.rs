//! NVIDIA NVKMS backend: ioctl transport, ABI generations and driver version probing.
//!
//! This is the only crate in the workspace allowed to use `unsafe`, and the unsafe code is one
//! `ioctl` call in [`NvkmsNode`] (module `node`), with its `// SAFETY:` comment.
//!
//! The transport (ADR-0001):
//!
//! - [`NvkmsDevice`], one ioctl as "command number plus a parameter block of exactly this size"
//! - [`Block`], the aligned, bounds-checked parameter block
//! - [`NvkmsNode`], the real `/dev/nvidia-modeset`
//! - [`fake::ScriptedDevice`] and [`emulator::Emulator`], stand-ins so everything above the
//!   transport can be tested without a GPU
//!
//! The ABI generations (ADR-0004):
//!
//! - [`GENERATIONS`], the table of layouts, generated from `tests/abi/generations.json`
//! - [`candidates`], which layouts to try for a driver [`Version`]
//!
//! And the backend:
//!
//! - [`NvkmsBackend`], which implements [`vivid_core::Backend`] on top of all of the above

mod backend;
mod block;
mod device;
pub mod emulator;
pub mod fake;
mod generations;
mod node;
mod select;
mod version;

pub use backend::{NvkmsBackend, VERSION_OVERRIDE_ENV};
pub use block::{BLOCK_CAPACITY, Block, OutOfBounds};
pub use device::{CallError, NvkmsDevice};
pub use generations::{GENERATIONS, Generation, Layout};
pub use node::{DEVICE_PATH, NvkmsNode};
pub use select::{Candidate, Confidence, candidates};
pub use version::{Version, VersionError};
