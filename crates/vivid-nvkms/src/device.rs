//! The transport: the one thing the rest of the crate needs from the system.

use std::{error::Error, fmt};

use vivid_core::BackendError;

use crate::{BLOCK_CAPACITY, Block};

/// An NVKMS ioctl was rejected, or could not be made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CallError {
    /// The system call failed with this OS error number.
    ///
    /// NVKMS answers `EPERM` to every rejection (wrong driver version, wrong block size, bad
    /// handle), so the number alone does not say which. See ADR-0001.
    Os(i32),
    /// The requested block size exceeds [`BLOCK_CAPACITY`]; nothing was sent.
    SizeTooLarge {
        /// The size asked for.
        size: usize,
    },
}

impl fmt::Display for CallError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Os(errno) => write!(f, "ioctl failed with OS error {errno}"),
            Self::SizeTooLarge { size } => {
                write!(f, "block of {size} bytes exceeds the {BLOCK_CAPACITY}-byte capacity")
            }
        }
    }
}

impl Error for CallError {}

/// A way of making NVKMS ioctls: the real `/dev/nvidia-modeset` device, or a fake in tests.
///
/// One call is one ioctl: an NVKMS command number and a parameter block of exactly `size` bytes.
/// The kernel checks that size against the one it expects for the command, so `size` is not a
/// formality. The reply is written into the same block.
pub trait NvkmsDevice {
    /// Makes one ioctl.
    ///
    /// # Errors
    ///
    /// [`CallError::Os`] when the system rejects the call, [`CallError::SizeTooLarge`] when `size`
    /// does not fit in a [`Block`].
    fn call(&mut self, command: u32, block: &mut Block, size: usize) -> Result<(), CallError>;
}

/// What NVKMS answers to every rejection.
const EPERM: i32 = 1;

impl From<CallError> for BackendError {
    fn from(error: CallError) -> Self {
        match error {
            CallError::Os(EPERM) => {
                Self::Rejected("NVKMS rejected the call (driver version, block size or handle not accepted)".to_owned())
            }
            CallError::Os(errno) => {
                Self::Io(format!("ioctl on the NVIDIA modeset device failed with OS error {errno}"))
            }
            CallError::SizeTooLarge { size } => {
                Self::Unsupported(format!("a block of {size} bytes does not fit the transport"))
            }
        }
    }
}

impl<T: NvkmsDevice + ?Sized> NvkmsDevice for Box<T> {
    fn call(&mut self, command: u32, block: &mut Block, size: usize) -> Result<(), CallError> {
        (**self).call(command, block, size)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_say_what_happened() {
        assert_eq!(CallError::Os(1).to_string(), "ioctl failed with OS error 1");
        assert_eq!(
            CallError::SizeTooLarge { size: 50_000 }.to_string(),
            "block of 50000 bytes exceeds the 40960-byte capacity"
        );
    }

    #[test]
    fn eperm_is_a_rejection() {
        assert!(matches!(BackendError::from(CallError::Os(1)), BackendError::Rejected(_)));
    }

    #[test]
    fn other_os_errors_are_i_o_errors_that_keep_the_number() {
        match BackendError::from(CallError::Os(25)) {
            BackendError::Io(message) => assert!(message.contains("25"), "{message}"),
            other => assert_eq!(other, BackendError::Io(String::new())),
        }
    }

    #[test]
    fn an_oversized_block_is_unsupported() {
        assert!(matches!(
            BackendError::from(CallError::SizeTooLarge { size: 99_999 }),
            BackendError::Unsupported(_)
        ));
    }

    #[test]
    fn a_call_error_is_a_std_error() {
        fn assert_error<E: Error>() {}
        assert_error::<CallError>();
    }
}
