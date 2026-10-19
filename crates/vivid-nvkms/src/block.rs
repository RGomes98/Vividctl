//! The byte buffer an NVKMS ioctl reads and writes.
//!
//! Every NVKMS call passes one fixed-layout parameter block: a request followed by a reply. The
//! block must be 8-byte aligned (the structs hold 64-bit fields) and its size must match what the
//! driver expects exactly. [`Block`] is one aligned buffer big enough for every call of every
//! known driver generation; a call uses the first `size` bytes. Reads and writes are bounds
//! checked and return [`OutOfBounds`] instead of panicking.

use std::{error::Error, fmt};

/// Bytes available in a [`Block`]. The largest block of any known driver generation is 37,168.
pub const BLOCK_CAPACITY: usize = 40_960;

/// An access that falls outside the block.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OutOfBounds {
    /// Where the access started.
    pub offset: usize,
    /// How many bytes it wanted.
    pub len:    usize,
}

impl fmt::Display for OutOfBounds {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} bytes at offset {} do not fit in a {BLOCK_CAPACITY}-byte block",
            self.len, self.offset
        )
    }
}

impl Error for OutOfBounds {}

/// A zeroed, 8-byte aligned parameter block.
#[repr(C, align(8))]
pub struct Block {
    bytes: [u8; BLOCK_CAPACITY],
}

impl Block {
    /// A new block, all zeros, on the heap.
    #[must_use]
    // The 40 KiB array is built on the stack and moved into the box. Stacks are 2 MiB at least, and
    // zeroing it on the heap directly would need `unsafe`, which this module does not allow itself.
    #[allow(clippy::large_stack_arrays)]
    pub fn new() -> Box<Self> {
        Box::new(Self {
            bytes: [0; BLOCK_CAPACITY],
        })
    }

    /// Zeroes the first `len` bytes, so a call starts from a known state.
    ///
    /// # Errors
    ///
    /// [`OutOfBounds`] when `len` exceeds [`BLOCK_CAPACITY`].
    pub fn clear(&mut self, len: usize) -> Result<(), OutOfBounds> {
        self.bytes.get_mut(..len).ok_or(OutOfBounds { offset: 0, len })?.fill(0);
        Ok(())
    }

    /// A pointer to the start of the block, for the ioctl.
    pub(crate) fn as_mut_ptr(&mut self) -> *mut u8 {
        self.bytes.as_mut_ptr()
    }

    /// `len` bytes starting at `offset`.
    ///
    /// # Errors
    ///
    /// [`OutOfBounds`] when the range does not fit.
    pub fn bytes_at(&self, offset: usize, len: usize) -> Result<&[u8], OutOfBounds> {
        let end: usize = offset.checked_add(len).ok_or(OutOfBounds { offset, len })?;
        self.bytes.get(offset..end).ok_or(OutOfBounds { offset, len })
    }

    /// Writes `data` at `offset`.
    ///
    /// # Errors
    ///
    /// [`OutOfBounds`] when it does not fit.
    pub fn put_bytes(&mut self, offset: usize, data: &[u8]) -> Result<(), OutOfBounds> {
        let len: usize = data.len();
        let end: usize = offset.checked_add(len).ok_or(OutOfBounds { offset, len })?;
        self.bytes
            .get_mut(offset..end)
            .ok_or(OutOfBounds { offset, len })?
            .copy_from_slice(data);
        Ok(())
    }

    /// Writes a native-endian `u32` at `offset`.
    ///
    /// # Errors
    ///
    /// [`OutOfBounds`] when it does not fit.
    pub fn put_u32(&mut self, offset: usize, value: u32) -> Result<(), OutOfBounds> {
        self.put_bytes(offset, &value.to_ne_bytes())
    }

    /// Writes a native-endian `i64` at `offset`.
    ///
    /// # Errors
    ///
    /// [`OutOfBounds`] when it does not fit.
    pub fn put_i64(&mut self, offset: usize, value: i64) -> Result<(), OutOfBounds> {
        self.put_bytes(offset, &value.to_ne_bytes())
    }

    /// Reads a byte at `offset`.
    ///
    /// # Errors
    ///
    /// [`OutOfBounds`] when it does not fit.
    pub fn u8_at(&self, offset: usize) -> Result<u8, OutOfBounds> {
        self.bytes.get(offset).copied().ok_or(OutOfBounds { offset, len: 1 })
    }

    /// Reads a native-endian `u16` at `offset`.
    ///
    /// # Errors
    ///
    /// [`OutOfBounds`] when it does not fit.
    pub fn u16_at(&self, offset: usize) -> Result<u16, OutOfBounds> {
        let bytes: [u8; 2] = self.array_at(offset)?;
        Ok(u16::from_ne_bytes(bytes))
    }

    /// Reads a native-endian `u32` at `offset`.
    ///
    /// # Errors
    ///
    /// [`OutOfBounds`] when it does not fit.
    pub fn u32_at(&self, offset: usize) -> Result<u32, OutOfBounds> {
        let bytes: [u8; 4] = self.array_at(offset)?;
        Ok(u32::from_ne_bytes(bytes))
    }

    /// Reads a native-endian `i64` at `offset`.
    ///
    /// # Errors
    ///
    /// [`OutOfBounds`] when it does not fit.
    pub fn i64_at(&self, offset: usize) -> Result<i64, OutOfBounds> {
        let bytes: [u8; 8] = self.array_at(offset)?;
        Ok(i64::from_ne_bytes(bytes))
    }

    fn array_at<const N: usize>(&self, offset: usize) -> Result<[u8; N], OutOfBounds> {
        <[u8; N]>::try_from(self.bytes_at(offset, N)?).map_err(|_| OutOfBounds { offset, len: N })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_block_is_zeroed_and_eight_byte_aligned() {
        let mut block: Box<Block> = Block::new();
        assert_eq!(block.bytes_at(0, BLOCK_CAPACITY).map(|b| b.iter().all(|&x| x == 0)), Ok(true));
        assert_eq!(block.as_mut_ptr().addr() % 8, 0);
        assert_eq!(align_of::<Block>(), 8);
    }

    #[test]
    fn values_round_trip() {
        let mut block: Box<Block> = Block::new();
        assert_eq!(block.put_u32(4, 0xDEAD_BEEF), Ok(()));
        assert_eq!(block.put_i64(16, -1024), Ok(()));
        assert_eq!(block.put_bytes(32, b"595.99.02"), Ok(()));
        assert_eq!(block.u32_at(4), Ok(0xDEAD_BEEF));
        assert_eq!(block.i64_at(16), Ok(-1024));
        assert_eq!(block.bytes_at(32, 9), Ok(&b"595.99.02"[..]));
        assert_eq!(block.u8_at(32), Ok(b'5'));
        assert_eq!(block.put_bytes(8, &[0x34, 0x12]), Ok(()));
        assert_eq!(block.u16_at(8), Ok(u16::from_ne_bytes([0x34, 0x12])));
    }

    #[test]
    fn the_last_bytes_are_reachable_and_one_past_is_not() {
        let mut block: Box<Block> = Block::new();
        assert_eq!(block.put_u32(BLOCK_CAPACITY - 4, 7), Ok(()));
        assert_eq!(block.u32_at(BLOCK_CAPACITY - 4), Ok(7));
        assert_eq!(
            block.put_u32(BLOCK_CAPACITY - 3, 7),
            Err(OutOfBounds {
                offset: BLOCK_CAPACITY - 3,
                len:    4,
            })
        );
        assert_eq!(
            block.u32_at(BLOCK_CAPACITY - 3),
            Err(OutOfBounds {
                offset: BLOCK_CAPACITY - 3,
                len:    4,
            })
        );
        assert_eq!(
            block.u8_at(BLOCK_CAPACITY),
            Err(OutOfBounds {
                offset: BLOCK_CAPACITY,
                len:    1,
            })
        );
    }

    #[test]
    fn an_offset_that_would_overflow_is_an_error_not_a_panic() {
        let mut block: Box<Block> = Block::new();
        assert!(block.bytes_at(usize::MAX, 2).is_err());
        assert!(block.put_bytes(usize::MAX - 1, &[1, 2, 3]).is_err());
        assert!(block.u32_at(usize::MAX).is_err());
    }

    #[test]
    fn clear_zeroes_only_the_requested_prefix() {
        let mut block: Box<Block> = Block::new();
        assert_eq!(block.put_u32(0, 1), Ok(()));
        assert_eq!(block.put_u32(100, 2), Ok(()));
        assert_eq!(block.clear(64), Ok(()));
        assert_eq!(block.u32_at(0), Ok(0));
        assert_eq!(block.u32_at(100), Ok(2));
        assert_eq!(
            block.clear(BLOCK_CAPACITY + 1),
            Err(OutOfBounds {
                offset: 0,
                len:    BLOCK_CAPACITY + 1,
            })
        );
    }

    #[test]
    fn the_error_says_what_did_not_fit() {
        let message: String = OutOfBounds {
            offset: 40_000,
            len:    2_000,
        }
        .to_string();
        assert_eq!(message, "2000 bytes at offset 40000 do not fit in a 40960-byte block");
    }
}
