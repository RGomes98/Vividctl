//! The real device, `/dev/nvidia-modeset`.
//!
//! This module holds the only `unsafe` in the workspace: the single `ioctl` call.

use rustix::{
    fd::OwnedFd,
    fs::{Mode, OFlags, open},
    io::Errno,
    ioctl::{Opcode, Updater, ioctl, opcode},
};
use vivid_core::BackendError;

use crate::{BLOCK_CAPACITY, Block, CallError, NvkmsDevice};

/// Where the NVIDIA modeset device lives.
pub const DEVICE_PATH: &str = "/dev/nvidia-modeset";

/// `struct NvKmsIoctlParams`: the envelope every NVKMS ioctl goes through.
#[repr(C)]
struct IoctlParams {
    command: u32,
    size:    u32,
    address: u64,
}

/// `_IOWR('m', 0, struct NvKmsIoctlParams)`, the opcode `nvidia-modeset` implements.
const NVKMS_IOCTL: Opcode = opcode::read_write::<IoctlParams>(b'm', 0);

/// An open `/dev/nvidia-modeset`.
pub struct NvkmsNode {
    fd: OwnedFd,
}

impl NvkmsNode {
    /// Opens [`DEVICE_PATH`].
    ///
    /// # Errors
    ///
    /// [`BackendError::Unavailable`] when the device is missing or not accessible,
    /// [`BackendError::Io`] for anything else.
    pub fn open() -> Result<Self, BackendError> {
        Self::open_at(DEVICE_PATH)
    }

    /// Opens a device node at `path`. Used by tests to exercise the plumbing on a file that is not
    /// the NVIDIA device.
    ///
    /// # Errors
    ///
    /// As [`NvkmsNode::open`].
    pub fn open_at(path: &str) -> Result<Self, BackendError> {
        match open(path, OFlags::RDWR | OFlags::CLOEXEC, Mode::empty()) {
            Ok(fd) => Ok(Self { fd }),
            Err(Errno::NOENT) => Err(BackendError::Unavailable(format!(
                "{path} does not exist; the NVIDIA kernel module is not loaded for the running kernel"
            ))),
            Err(Errno::ACCESS | Errno::PERM) => Err(BackendError::Unavailable(format!("no permission to open {path}"))),
            Err(other) => Err(BackendError::Io(format!("opening {path}: {other}"))),
        }
    }
}

impl NvkmsDevice for NvkmsNode {
    // The one place in the workspace that needs `unsafe`: the ioctl itself.
    #[allow(unsafe_code)]
    fn call(&mut self, command: u32, block: &mut Block, size: usize) -> Result<(), CallError> {
        if size > BLOCK_CAPACITY {
            return Err(CallError::SizeTooLarge { size });
        }
        // `size <= BLOCK_CAPACITY` (40,960), so both conversions are lossless on every target.
        let size_u32: u32 = u32::try_from(size).map_err(|_| CallError::SizeTooLarge { size })?;
        let address: u64 = u64::try_from(block.as_mut_ptr().addr()).map_err(|_| CallError::SizeTooLarge { size })?;
        let mut params: IoctlParams = IoctlParams {
            command,
            size: size_u32,
            address,
        };

        // SAFETY: `NVKMS_IOCTL` is `_IOWR('m', 0, IoctlParams)`, the opcode `nvidia-modeset`
        // implements, and `Updater<_, IoctlParams>` passes a pointer to an `IoctlParams` of exactly
        // that type. `params` is live and properly aligned for the whole call. `params.address`
        // points to the start of `block`, which is exclusively borrowed here, is 8-byte aligned,
        // and holds at least `size` initialized bytes (checked against `BLOCK_CAPACITY` above), so
        // the kernel only reads and writes memory this call owns.
        unsafe { ioctl(&self.fd, Updater::<{ NVKMS_IOCTL }, IoctlParams>::new(&mut params)) }
            .map_err(|errno| CallError::Os(errno.raw_os_error()))
    }
}

#[cfg(test)]
// Tests expect devices that exist on every Linux system, and fail loudly otherwise.
#[allow(clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_device_is_unavailable_and_says_why() {
        let error: Option<BackendError> = NvkmsNode::open_at("/dev/this-device-does-not-exist").err();
        match error {
            Some(BackendError::Unavailable(reason)) => assert!(reason.contains("not loaded"), "{reason}"),
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }

    #[test]
    fn a_directory_is_an_i_o_error_not_unavailable() {
        // Opening a directory read-write fails with EISDIR: neither missing nor a permission problem.
        let error: Option<BackendError> = NvkmsNode::open_at("/").err();
        assert!(matches!(error, Some(BackendError::Io(_))), "{error:?}");
    }

    /// The ioctl plumbing, without a GPU: `/dev/null` has no `_IOWR('m', 0, ..)` handler, so the
    /// kernel answers `ENOTTY` (25). That proves the call reaches the kernel with a valid
    /// envelope and that the error number comes back.
    #[test]
    fn the_ioctl_path_reaches_the_kernel_and_returns_its_error() {
        let mut node: NvkmsNode = NvkmsNode::open_at("/dev/null").expect("/dev/null opens");
        let mut block: Box<Block> = Block::new();
        assert_eq!(node.call(0, &mut block, 64), Err(CallError::Os(25)));
    }

    #[test]
    fn an_oversized_block_is_refused_before_any_system_call() {
        let mut node: NvkmsNode = NvkmsNode::open_at("/dev/null").expect("/dev/null opens");
        let mut block: Box<Block> = Block::new();
        let size: usize = BLOCK_CAPACITY + 1;
        assert_eq!(node.call(0, &mut block, size), Err(CallError::SizeTooLarge { size }));
    }

    /// Hardware test: the real device exists and rejects a call it does not understand, with
    /// `EPERM` (1) as NVKMS always does. Run with `VIVID_HW_TESTS=1 cargo test -- --ignored`.
    #[test]
    #[ignore = "needs the NVIDIA kernel module; run with VIVID_HW_TESTS=1 and --ignored"]
    fn the_nvidia_device_rejects_an_unknown_call_with_eperm() {
        if std::env::var_os("VIVID_HW_TESTS").is_none() {
            return;
        }
        let mut node: NvkmsNode = NvkmsNode::open().expect("/dev/nvidia-modeset opens");
        let mut block: Box<Block> = Block::new();
        assert_eq!(node.call(0xFFFF, &mut block, 8), Err(CallError::Os(1)));
    }
}
