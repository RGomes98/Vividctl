//! A scripted stand-in for the NVIDIA device, so everything above the transport runs without a GPU.
//!
//! [`ScriptedDevice`] behaves like the kernel does towards a client that makes the wrong call:
//! a call that is not the next scripted one is rejected with `EPERM` and consumes nothing. That
//! is also how NVKMS treats a wrong command, block size or driver version. Tests assert on
//! [`ScriptedDevice::calls`] to check exactly what was sent.

use std::collections::VecDeque;

use crate::{BLOCK_CAPACITY, Block, CallError, NvkmsDevice};

/// What NVKMS answers to a call it does not accept.
const EPERM: i32 = 1;

/// One call as the device saw it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Call {
    /// The NVKMS command number.
    pub command:  u32,
    /// The block size passed.
    pub size:     usize,
    /// The first `size` bytes of the block when the call was made.
    pub request:  Vec<u8>,
    /// Whether the device accepted it (it matched the next scripted step).
    pub accepted: bool,
}

#[derive(Debug)]
enum Outcome {
    Reply(Vec<(usize, Vec<u8>)>),
    Fail(i32),
}

#[derive(Debug)]
struct Step {
    command: u32,
    size:    usize,
    outcome: Outcome,
}

/// A device that answers from a script, in order.
#[derive(Debug, Default)]
pub struct ScriptedDevice {
    steps: VecDeque<Step>,
    calls: Vec<Call>,
}

impl ScriptedDevice {
    /// A device with an empty script: every call is rejected.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Expects `command` with a block of `size` bytes, and answers by writing each `(offset, bytes)`
    /// patch into the block.
    #[must_use]
    pub fn expect_reply(mut self, command: u32, size: usize, patches: Vec<(usize, Vec<u8>)>) -> Self {
        self.steps.push_back(Step {
            command,
            size,
            outcome: Outcome::Reply(patches),
        });
        self
    }

    /// Expects `command` with a block of `size` bytes, and fails it with OS error `errno`.
    #[must_use]
    pub fn expect_failure(mut self, command: u32, size: usize, errno: i32) -> Self {
        self.steps.push_back(Step {
            command,
            size,
            outcome: Outcome::Fail(errno),
        });
        self
    }

    /// Every call made so far, accepted or not, in order.
    #[must_use]
    pub fn calls(&self) -> &[Call] {
        &self.calls
    }

    /// How many scripted steps have not been consumed.
    #[must_use]
    pub fn remaining(&self) -> usize {
        self.steps.len()
    }

    /// Whether the whole script was consumed.
    #[must_use]
    pub fn finished(&self) -> bool {
        self.steps.is_empty()
    }
}

impl NvkmsDevice for ScriptedDevice {
    fn call(&mut self, command: u32, block: &mut Block, size: usize) -> Result<(), CallError> {
        if size > BLOCK_CAPACITY {
            return Err(CallError::SizeTooLarge { size });
        }
        let request: Vec<u8> = block.bytes_at(0, size).map(<[u8]>::to_vec).unwrap_or_default();
        let matches: bool = self
            .steps
            .front()
            .is_some_and(|step| step.command == command && step.size == size);
        self.calls.push(Call {
            command,
            size,
            request,
            accepted: matches,
        });
        if !matches {
            return Err(CallError::Os(EPERM));
        }
        match self.steps.pop_front().map(|step| step.outcome) {
            Some(Outcome::Reply(patches)) => {
                for (offset, bytes) in patches {
                    block
                        .put_bytes(offset, &bytes)
                        .map_err(|_| CallError::SizeTooLarge { size })?;
                }
                Ok(())
            }
            Some(Outcome::Fail(errno)) => Err(CallError::Os(errno)),
            None => Err(CallError::Os(EPERM)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_matching_call_writes_the_scripted_reply_into_the_block() {
        let mut device: ScriptedDevice = ScriptedDevice::new().expect_reply(7, 16, vec![(4, vec![1, 2, 3, 4])]);
        let mut block: Box<Block> = Block::new();
        assert_eq!(device.call(7, &mut block, 16), Ok(()));
        assert_eq!(block.bytes_at(4, 4), Ok(&[1, 2, 3, 4][..]));
        assert!(device.finished());
    }

    #[test]
    fn the_request_is_recorded_as_it_was_when_the_call_was_made() {
        let mut device: ScriptedDevice = ScriptedDevice::new().expect_reply(7, 8, vec![(0, vec![9; 8])]);
        let mut block: Box<Block> = Block::new();
        assert_eq!(block.put_bytes(0, &[5; 8]), Ok(()));
        assert_eq!(device.call(7, &mut block, 8), Ok(()));
        assert_eq!(device.calls().first().map(|c| c.request.clone()), Some(vec![5; 8]));
        assert_eq!(block.bytes_at(0, 8), Ok(&[9; 8][..]));
    }

    #[test]
    fn a_wrong_command_is_rejected_with_eperm_and_consumes_nothing() {
        let mut device: ScriptedDevice = ScriptedDevice::new().expect_reply(7, 16, vec![]);
        let mut block: Box<Block> = Block::new();
        assert_eq!(device.call(8, &mut block, 16), Err(CallError::Os(1)));
        assert_eq!(device.remaining(), 1);
        assert_eq!(device.call(7, &mut block, 16), Ok(()));
    }

    #[test]
    fn a_wrong_block_size_is_rejected_like_the_kernel_does() {
        let mut device: ScriptedDevice = ScriptedDevice::new().expect_reply(7, 16, vec![]);
        let mut block: Box<Block> = Block::new();
        assert_eq!(device.call(7, &mut block, 12), Err(CallError::Os(1)));
        assert_eq!(device.call(7, &mut block, 20), Err(CallError::Os(1)));
        assert_eq!(device.remaining(), 1);
    }

    #[test]
    fn rejected_probes_are_visible_in_the_call_log() {
        let mut device: ScriptedDevice = ScriptedDevice::new().expect_reply(22, 24, vec![]);
        let mut block: Box<Block> = Block::new();
        assert!(device.call(21, &mut block, 24).is_err());
        assert!(device.call(22, &mut block, 24).is_ok());
        let accepted: Vec<bool> = device.calls().iter().map(|c| c.accepted).collect();
        assert_eq!(accepted, [false, true]);
    }

    #[test]
    fn steps_are_consumed_in_order() {
        let mut device: ScriptedDevice = ScriptedDevice::new()
            .expect_reply(1, 8, vec![])
            .expect_reply(2, 8, vec![]);
        let mut block: Box<Block> = Block::new();
        assert_eq!(device.call(2, &mut block, 8), Err(CallError::Os(1)));
        assert_eq!(device.call(1, &mut block, 8), Ok(()));
        assert_eq!(device.call(2, &mut block, 8), Ok(()));
        assert!(device.finished());
    }

    #[test]
    fn a_scripted_failure_returns_its_errno() {
        let mut device: ScriptedDevice = ScriptedDevice::new().expect_failure(3, 8, 5);
        let mut block: Box<Block> = Block::new();
        assert_eq!(device.call(3, &mut block, 8), Err(CallError::Os(5)));
        assert!(device.finished());
    }

    #[test]
    fn an_exhausted_script_rejects_everything() {
        let mut device: ScriptedDevice = ScriptedDevice::new();
        let mut block: Box<Block> = Block::new();
        assert_eq!(device.call(1, &mut block, 8), Err(CallError::Os(1)));
        assert_eq!(device.calls().len(), 1);
    }

    #[test]
    fn an_oversized_block_is_refused_and_not_logged_as_accepted() {
        let mut device: ScriptedDevice = ScriptedDevice::new().expect_reply(1, BLOCK_CAPACITY + 1, vec![]);
        let mut block: Box<Block> = Block::new();
        let size: usize = BLOCK_CAPACITY + 1;
        assert_eq!(device.call(1, &mut block, size), Err(CallError::SizeTooLarge { size }));
        assert_eq!(device.calls().to_vec(), Vec::<Call>::new());
    }

    #[test]
    fn a_boxed_device_is_a_device() {
        let mut device: Box<dyn NvkmsDevice> = Box::new(ScriptedDevice::new().expect_reply(1, 8, vec![]));
        let mut block: Box<Block> = Block::new();
        assert_eq!(device.call(1, &mut block, 8), Ok(()));
    }
}
