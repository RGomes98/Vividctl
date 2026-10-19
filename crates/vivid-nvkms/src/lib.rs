//! NVIDIA NVKMS backend: ioctl transport, ABI generations and driver version probing.
//!
//! This is the only crate in the workspace allowed to use `unsafe`, and the unsafe code is one
//! `ioctl` call in [`NvkmsNode`] (module `node`), with its `// SAFETY:` comment.
//!
//! What is here so far is the transport (ADR-0001):
//!
//! - [`NvkmsDevice`], one ioctl as "command number plus a parameter block of exactly this size"
//! - [`Block`], the aligned, bounds-checked parameter block
//! - [`NvkmsNode`], the real `/dev/nvidia-modeset`
//! - [`fake::ScriptedDevice`], a scripted stand-in so everything above the transport can be tested
//!   without a GPU
//!
//! The ABI generations and the backend itself (issue #14) are built on top of this.

mod block;
mod device;
pub mod fake;
mod node;

pub use block::{BLOCK_CAPACITY, Block, OutOfBounds};
pub use device::{CallError, NvkmsDevice};
pub use node::{DEVICE_PATH, NvkmsNode};
