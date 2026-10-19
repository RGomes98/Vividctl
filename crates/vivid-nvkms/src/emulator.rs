//! An emulation of the NVKMS kernel interface, for testing the backend without a GPU.
//!
//! [`Emulator`] behaves like `nvidia-modeset` as far as the ioctls vividctl uses: it accepts a
//! call only if both the command number and the block size are the ones its layout expects, it
//! checks the driver version string, handles and attributes, and answers in the layout's byte
//! positions. Run against every generation of the table it checks that the backend is consistent
//! with each layout. It cannot tell whether the table matches a real driver: that is what
//! `tests/abi` and the hardware tests are for.

use std::{cell::RefCell, rc::Rc};

use crate::{Block, CallError, Layout, NvkmsDevice};

const EPERM: i32 = 1;
const EFAULT: i32 = 14;

const DEVICE_HANDLE: u32 = 0x0A0B_0C01;
const DISP_HANDLE: u32 = 0x0D0E_0F01;
const CONNECTOR_BASE: u32 = 0x1000;
const DPY_BASE: u32 = 0x100;

#[derive(Clone, Debug)]
struct Output {
    connector_type: u32,
    type_index:     u32,
    connected:      bool,
    edid:           Vec<u8>,
    vibrance:       i64,
}

#[derive(Debug)]
struct State {
    layout:           Layout,
    driver_version:   String,
    gpu_count:        u32,
    outputs:          Vec<Output>,
    open:             bool,
    freed:            u32,
    calls:            u32,
    bogus_disp_count: Option<u32>,
    corrupt_readback: bool,
    reject_set:       bool,
}

/// A fake `nvidia-modeset`. Cloning gives another handle to the same state, so a test can keep one
/// to inspect while the backend owns the other.
#[derive(Clone, Debug)]
pub struct Emulator {
    state: Rc<RefCell<State>>,
}

impl Emulator {
    /// An emulator for a driver of the given version with one GPU and no outputs.
    #[must_use]
    pub fn new(layout: &Layout, driver_version: &str) -> Self {
        Self {
            state: Rc::new(RefCell::new(State {
                layout:           *layout,
                driver_version:   driver_version.to_owned(),
                gpu_count:        1,
                outputs:          Vec::new(),
                open:             false,
                freed:            0,
                calls:            0,
                bogus_disp_count: None,
                corrupt_readback: false,
                reject_set:       false,
            })),
        }
    }

    /// Adds an output. `edid` is what the monitor reports, if anything.
    #[must_use]
    pub fn with_output(self, connector_type: u32, type_index: u32, connected: bool, edid: Option<Vec<u8>>) -> Self {
        self.add_output(connector_type, type_index, connected, edid);
        self
    }

    /// Adds an output after the fact, like a hotplug.
    pub fn add_output(&self, connector_type: u32, type_index: u32, connected: bool, edid: Option<Vec<u8>>) {
        self.state.borrow_mut().outputs.push(Output {
            connector_type,
            type_index,
            connected,
            edid: edid.unwrap_or_default(),
            vibrance: 0,
        });
    }

    /// Makes the emulator report this many disps instead of one, to model a driver that answers
    /// nonsense.
    pub fn set_bogus_disp_count(&self, count: Option<u32>) {
        self.state.borrow_mut().bogus_disp_count = count;
    }

    /// Makes every read of the vibrance return a different value than the one written.
    pub fn set_corrupt_readback(&self, corrupt: bool) {
        self.state.borrow_mut().corrupt_readback = corrupt;
    }

    /// Makes `SET_DPY_ATTRIBUTE` fail.
    pub fn set_reject_set(&self, reject: bool) {
        self.state.borrow_mut().reject_set = reject;
    }

    /// The vibrance stored for output `index` (in the order outputs were added).
    #[must_use]
    pub fn vibrance_of(&self, index: usize) -> Option<i64> {
        self.state.borrow().outputs.get(index).map(|o| o.vibrance)
    }

    /// How many `FREE_DEVICE` calls were accepted.
    #[must_use]
    pub fn freed(&self) -> u32 {
        self.state.borrow().freed
    }

    /// Whether a device handle is currently allocated.
    #[must_use]
    pub fn is_open(&self) -> bool {
        self.state.borrow().open
    }

    /// How many calls reached the emulator, accepted or not.
    #[must_use]
    pub fn calls(&self) -> u32 {
        self.state.borrow().calls
    }
}

fn fault<T, E>(_: E) -> Result<T, CallError> {
    Err(CallError::Os(EFAULT))
}

impl NvkmsDevice for Emulator {
    fn call(&mut self, command: u32, block: &mut Block, size: usize) -> Result<(), CallError> {
        let mut state = self.state.borrow_mut();
        state.calls += 1;
        let l: Layout = state.layout;
        if command == l.cmd_alloc_device && size == l.alloc_size {
            alloc(&mut state, block)
        } else if command == l.cmd_free_device && size == l.free_size {
            free(&mut state, block)
        } else if command == l.cmd_query_disp && size == l.query_disp_size {
            query_disp(&state, block)
        } else if command == l.cmd_query_connector_static_data && size == l.connector_size {
            connector(&state, block)
        } else if command == l.cmd_query_dpy_dynamic_data && size == l.dynamic_size {
            dynamic(&state, block)
        } else if command == l.cmd_set_dpy_attribute && size == l.set_size {
            set(&mut state, block)
        } else if command == l.cmd_get_dpy_attribute && size == l.get_size {
            get(&state, block)
        } else {
            Err(CallError::Os(EPERM))
        }
    }
}

fn alloc(state: &mut State, block: &mut Block) -> Result<(), CallError> {
    let l: Layout = state.layout;
    let reply: usize = l.alloc_reply_offset;
    let Ok(text) = block.bytes_at(l.alloc_version_string_offset, l.version_string_len) else {
        return fault(());
    };
    let end: usize = text.iter().position(|&b| b == 0).unwrap_or(text.len());
    let version_ok: bool = text.get(..end) == Some(state.driver_version.as_bytes());
    let Ok(gpu) = block.u32_at(l.alloc_device_id_offset) else {
        return fault(());
    };

    let reject = |block: &mut Block, status: u32| -> Result<(), CallError> {
        block.put_u32(reply + l.alloc_status_offset, status).or_else(fault)?;
        Err(CallError::Os(EPERM))
    };
    if !version_ok {
        return reject(block, l.status_version_mismatch);
    }
    if gpu >= state.gpu_count {
        return reject(block, l.status_no_hardware_available);
    }
    state.open = true;
    let disps: u32 = state.bogus_disp_count.unwrap_or(1);
    block
        .put_u32(reply + l.alloc_status_offset, l.status_success)
        .or_else(fault)?;
    block
        .put_u32(reply + l.alloc_device_handle_offset, DEVICE_HANDLE)
        .or_else(fault)?;
    block.put_u32(reply + l.alloc_num_disps_offset, disps).or_else(fault)?;
    if disps >= 1 {
        block
            .put_u32(reply + l.alloc_disp_handles_offset, DISP_HANDLE)
            .or_else(fault)?;
    }
    Ok(())
}

fn free(state: &mut State, block: &Block) -> Result<(), CallError> {
    let handle: u32 = block.u32_at(state.layout.free_device_handle_offset).or_else(fault)?;
    if !state.open || handle != DEVICE_HANDLE {
        return Err(CallError::Os(EPERM));
    }
    state.open = false;
    state.freed += 1;
    Ok(())
}

fn check_handles(state: &State, block: &Block, device_offset: usize, disp_offset: usize) -> Result<(), CallError> {
    let device: u32 = block.u32_at(device_offset).or_else(fault)?;
    let disp: u32 = block.u32_at(disp_offset).or_else(fault)?;
    if state.open && device == DEVICE_HANDLE && disp == DISP_HANDLE {
        Ok(())
    } else {
        Err(CallError::Os(EPERM))
    }
}

fn query_disp(state: &State, block: &mut Block) -> Result<(), CallError> {
    let l: Layout = state.layout;
    check_handles(state, block, l.query_disp_device_handle_offset, l.query_disp_disp_handle_offset)?;
    let reply: usize = l.query_disp_reply_offset;
    let count: u32 = u32::try_from(state.outputs.len()).or_else(fault)?;
    block
        .put_u32(reply + l.disp_num_connectors_offset, count)
        .or_else(fault)?;
    for index in 0..count {
        let at: usize = reply + l.disp_connector_handles_offset + 4 * usize::try_from(index).or_else(fault)?;
        block.put_u32(at, CONNECTOR_BASE + index).or_else(fault)?;
    }
    Ok(())
}

fn connector(state: &State, block: &mut Block) -> Result<(), CallError> {
    let l: Layout = state.layout;
    check_handles(state, block, l.connector_device_handle_offset, l.connector_disp_handle_offset)?;
    let handle: u32 = block.u32_at(l.connector_connector_handle_offset).or_else(fault)?;
    let index: usize = usize::try_from(handle.wrapping_sub(CONNECTOR_BASE)).or_else(fault)?;
    let Some(output) = state.outputs.get(index) else {
        return Err(CallError::Os(EPERM));
    };
    let reply: usize = l.connector_reply_offset;
    block
        .put_u32(reply + l.connector_dpy_id_offset, dpy_id(index)?)
        .or_else(fault)?;
    block
        .put_u32(reply + l.connector_type_offset, output.connector_type)
        .or_else(fault)?;
    block
        .put_u32(reply + l.connector_type_index_offset, output.type_index)
        .or_else(fault)
}

fn dpy_id(index: usize) -> Result<u32, CallError> {
    let shift: u32 = u32::try_from(index).or_else(fault)?;
    DPY_BASE.checked_shl(shift).ok_or(CallError::Os(EFAULT))
}

fn find_dpy(state: &State, block: &Block, offset: usize) -> Result<usize, CallError> {
    let id: u32 = block.u32_at(offset).or_else(fault)?;
    (0..state.outputs.len())
        .find(|&index| dpy_id(index).is_ok_and(|candidate| candidate == id))
        .ok_or(CallError::Os(EPERM))
}

fn dynamic(state: &State, block: &mut Block) -> Result<(), CallError> {
    let l: Layout = state.layout;
    check_handles(state, block, l.dynamic_device_handle_offset, l.dynamic_disp_handle_offset)?;
    let index: usize = find_dpy(state, block, l.dynamic_dpy_id_offset)?;
    let Some(output) = state.outputs.get(index) else {
        return Err(CallError::Os(EPERM));
    };
    let reply: usize = l.dynamic_reply_offset;
    block
        .put_bytes(reply + l.dynamic_connected_offset, &[u8::from(output.connected)])
        .or_else(fault)?;
    if output.connected && !output.edid.is_empty() {
        let size: u16 = u16::try_from(output.edid.len()).or_else(fault)?;
        block
            .put_bytes(reply + l.dynamic_edid_size_offset, &size.to_ne_bytes())
            .or_else(fault)?;
        block
            .put_bytes(reply + l.dynamic_edid_valid_offset, &[1])
            .or_else(fault)?;
        block
            .put_bytes(reply + l.dynamic_edid_buffer_offset, &output.edid)
            .or_else(fault)?;
    }
    Ok(())
}

fn set(state: &mut State, block: &Block) -> Result<(), CallError> {
    let l: Layout = state.layout;
    check_handles(state, block, l.set_device_handle_offset, l.set_disp_handle_offset)?;
    let index: usize = find_dpy(state, block, l.set_dpy_id_offset)?;
    let attribute: u32 = block.u32_at(l.set_attribute_offset).or_else(fault)?;
    let value: i64 = block.i64_at(l.set_value_offset).or_else(fault)?;
    if attribute != l.attribute_digital_vibrance || state.reject_set {
        return Err(CallError::Os(EPERM));
    }
    let Some(output) = state.outputs.get_mut(index) else {
        return Err(CallError::Os(EPERM));
    };
    if !output.connected {
        return Err(CallError::Os(EPERM));
    }
    output.vibrance = value;
    Ok(())
}

fn get(state: &State, block: &mut Block) -> Result<(), CallError> {
    let l: Layout = state.layout;
    check_handles(state, block, l.get_device_handle_offset, l.get_disp_handle_offset)?;
    let index: usize = find_dpy(state, block, l.get_dpy_id_offset)?;
    let attribute: u32 = block.u32_at(l.get_attribute_offset).or_else(fault)?;
    if attribute != l.attribute_digital_vibrance {
        return Err(CallError::Os(EPERM));
    }
    let Some(output) = state.outputs.get(index) else {
        return Err(CallError::Os(EPERM));
    };
    let value: i64 = if state.corrupt_readback {
        output.vibrance.wrapping_add(1)
    } else {
        output.vibrance
    };
    block
        .put_i64(l.get_reply_offset + l.get_value_offset, value)
        .or_else(fault)
}
