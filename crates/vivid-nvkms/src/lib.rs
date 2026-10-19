//! NVIDIA NVKMS backend: ioctl transport, ABI generations and driver version probing.
//!
//! This is the only crate in the workspace allowed to use `unsafe`. Every unsafe block needs a `// SAFETY:` comment.

