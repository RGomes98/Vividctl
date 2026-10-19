//! The NVKMS backend: Digital Vibrance through `/dev/nvidia-modeset`.
//!
//! It follows ADR-0001 and ADR-0004:
//!
//! - the layout comes from the table, selected by the exact driver version;
//! - a version the table does not know is a guess that is validated against the driver before any
//!   value is written, and refused if nothing validates;
//! - every write is read back.

use std::{env, fs};

use vivid_core::{Backend, BackendError, BackendOutput, Capabilities, Edid, MonitorSpec, OutputHandle, Vibrance};

use crate::{
    Block, CallError, Candidate, Confidence, Layout, NvkmsDevice, NvkmsNode, OutOfBounds, Version, select::candidates,
};

/// Where the running driver publishes its version.
const SYSFS_VERSION: &str = "/sys/module/nvidia/version";
/// Overrides the driver version, for diagnostics when sysfs cannot be read.
pub const VERSION_OVERRIDE_ENV: &str = "VIVID_NVIDIA_VERSION";

/// The status NVKMS leaves in the reply of a rejected call is not proof of anything unless it is
/// one of the values we know.
impl From<OutOfBounds> for BackendError {
    fn from(error: OutOfBounds) -> Self {
        Self::Unsupported(format!("the layout does not fit the parameter block: {error}"))
    }
}

#[derive(Clone, Debug)]
struct Output {
    handle:    OutputHandle,
    disp:      u32,
    dpy_id:    u32,
    connector: String,
    spec:      MonitorSpec,
    connected: bool,
}

struct Session {
    layout:           Layout,
    generation:       u32,
    confidence:       Confidence,
    device_handle:    u32,
    disps:            Vec<u32>,
    outputs:          Vec<Output>,
    /// Whether a no-op write has been accepted and read back, proving the `SET`/`GET` pair.
    writes_validated: bool,
}

/// Why one candidate layout did not work out.
enum Attempt {
    /// This layout is wrong for the driver; try the next.
    Rejected(String),
    /// Nothing else would work either (version mismatch, no such GPU).
    Fatal(BackendError),
}

/// The NVKMS backend, generic over the transport so it runs against the real device or an
/// emulator.
pub struct NvkmsBackend<D: NvkmsDevice> {
    device:       D,
    block:        Box<Block>,
    version_text: String,
    version:      Version,
    gpu:          u32,
    session:      Option<Session>,
}

impl NvkmsBackend<NvkmsNode> {
    /// The backend for the system's NVIDIA GPU 0.
    ///
    /// The driver version is read from sysfs, or from `VIVID_NVIDIA_VERSION` when it is set.
    ///
    /// # Errors
    ///
    /// [`BackendError::Unavailable`] when the NVIDIA module is not loaded or the device cannot be
    /// opened, [`BackendError::Unsupported`] when the version text is not a driver version.
    pub fn open_system() -> Result<Self, BackendError> {
        let version: String = match env::var(VERSION_OVERRIDE_ENV) {
            Ok(text) if !text.trim().is_empty() => text,
            _ => fs::read_to_string(SYSFS_VERSION).map_err(|error| {
                BackendError::Unavailable(format!(
                    "cannot read the NVIDIA driver version from {SYSFS_VERSION} ({error}); the NVIDIA kernel module is not loaded"
                ))
            })?,
        };
        Self::new(NvkmsNode::open()?, &version, 0)
    }
}

impl<D: NvkmsDevice> NvkmsBackend<D> {
    /// A backend over `device` for a driver whose version string is `version`, driving GPU `gpu`.
    ///
    /// Nothing is sent to the device until [`Backend::probe`] or the first call that needs it.
    ///
    /// # Errors
    ///
    /// [`BackendError::Unsupported`] when `version` is not a driver version, or does not fit the
    /// driver's version string field.
    pub fn new(device: D, version: &str, gpu: u32) -> Result<Self, BackendError> {
        let version_text: String = version.trim().to_owned();
        let parsed: Version = version_text
            .parse()
            .map_err(|error| BackendError::Unsupported(format!("{error}")))?;
        Ok(Self {
            device,
            block: Block::new(),
            version_text,
            version: parsed,
            gpu,
            session: None,
        })
    }

    /// The driver version this backend was given.
    #[must_use]
    pub fn driver_version(&self) -> &str {
        &self.version_text
    }

    fn ensure_session(&mut self) -> Result<(), BackendError> {
        if self.session.is_some() {
            return Ok(());
        }
        let candidates: Vec<Candidate> = candidates(self.version);
        let mut last_reason: String = "the layout table is empty".to_owned();
        for candidate in &candidates {
            match attempt(&mut self.device, &mut self.block, &self.version_text, self.gpu, candidate) {
                Ok(session) => {
                    self.session = Some(session);
                    return Ok(());
                }
                Err(Attempt::Rejected(reason)) => last_reason = reason,
                Err(Attempt::Fatal(error)) => return Err(error),
            }
        }
        Err(BackendError::Unsupported(format!(
            "no known NVKMS layout fits NVIDIA driver {} ({last_reason}); \
             run `vividctl doctor` and report the driver version",
            self.version_text
        )))
    }

    fn output(&self, handle: OutputHandle) -> Result<Output, BackendError> {
        self.session
            .as_ref()
            .and_then(|session| session.outputs.iter().find(|output| output.handle == handle))
            .cloned()
            .ok_or(BackendError::UnknownOutput(handle))
    }
}

impl<D: NvkmsDevice> Backend for NvkmsBackend<D> {
    fn name(&self) -> &'static str {
        "nvkms"
    }

    fn probe(&mut self) -> Result<Capabilities, BackendError> {
        self.ensure_session()?;
        let session: &Session = self
            .session
            .as_ref()
            .ok_or_else(|| BackendError::Io("session lost".to_owned()))?;
        Ok(Capabilities {
            description:   format!(
                "NVIDIA driver {}, NVKMS ABI generation {} ({})",
                self.version_text, session.generation, session.confidence
            ),
            can_read_back: true,
        })
    }

    fn outputs(&mut self) -> Result<Vec<BackendOutput>, BackendError> {
        self.ensure_session()?;
        let session: &mut Session = self
            .session
            .as_mut()
            .ok_or_else(|| BackendError::Io("session lost".to_owned()))?;
        // Monitors come and go, so ask again every time.
        session.outputs = enumerate(
            &mut self.device,
            &mut self.block,
            &session.layout,
            session.device_handle,
            &session.disps,
        )
        .map_err(|failure| match failure {
            Failure::Call(error) => BackendError::from(error),
            Failure::Layout(error) => BackendError::from(error),
            Failure::Implausible(reason) => BackendError::Rejected(reason),
        })?;
        Ok(session
            .outputs
            .iter()
            .map(|o| BackendOutput {
                handle:    o.handle,
                connector: o.connector.clone(),
                spec:      o.spec.clone(),
                connected: o.connected,
            })
            .collect())
    }

    fn vibrance(&mut self, handle: OutputHandle) -> Result<Vibrance, BackendError> {
        self.ensure_session()?;
        let output: Output = self.output(handle)?;
        let session: &Session = self
            .session
            .as_ref()
            .ok_or_else(|| BackendError::Io("session lost".to_owned()))?;
        read_vibrance(
            &mut self.device,
            &mut self.block,
            &session.layout,
            session.device_handle,
            &output,
        )
    }

    fn set_vibrance(&mut self, handle: OutputHandle, value: Vibrance) -> Result<(), BackendError> {
        self.ensure_session()?;
        let output: Output = self.output(handle)?;
        if !output.connected {
            return Err(BackendError::Unsupported(format!(
                "{} has no monitor attached",
                output.connector
            )));
        }
        let session: &mut Session = self
            .session
            .as_mut()
            .ok_or_else(|| BackendError::Io("session lost".to_owned()))?;
        let (layout, device_handle) = (session.layout, session.device_handle);

        if !session.writes_validated {
            // Prove the SET/GET pair with a write that changes nothing before any real value goes
            // out: if the command numbers were wrong, the worst case is a rejected call.
            let current: Vibrance = read_vibrance(&mut self.device, &mut self.block, &layout, device_handle, &output)?;
            write_vibrance(&mut self.device, &mut self.block, &layout, device_handle, &output, current)?;
            let again: Vibrance = read_vibrance(&mut self.device, &mut self.block, &layout, device_handle, &output)?;
            if again != current {
                return Err(BackendError::ReadbackMismatch {
                    expected: current,
                    actual:   again,
                });
            }
            session.writes_validated = true;
        }

        write_vibrance(&mut self.device, &mut self.block, &layout, device_handle, &output, value)?;
        let actual: Vibrance = read_vibrance(&mut self.device, &mut self.block, &layout, device_handle, &output)?;
        if actual == value {
            Ok(())
        } else {
            Err(BackendError::ReadbackMismatch {
                expected: value,
                actual,
            })
        }
    }
}

impl<D: NvkmsDevice> Drop for NvkmsBackend<D> {
    fn drop(&mut self) {
        if let Some(session) = self.session.take() {
            // Best effort: the driver also releases the device when the file descriptor closes.
            let l: Layout = session.layout;
            let sent: Result<(), OutOfBounds> = self
                .block
                .clear(l.free_size)
                .and_then(|()| self.block.put_u32(l.free_device_handle_offset, session.device_handle));
            if sent.is_ok() {
                let _ = self.device.call(l.cmd_free_device, &mut self.block, l.free_size);
            }
        }
    }
}

enum Failure {
    Call(CallError),
    Layout(OutOfBounds),
    Implausible(String),
}

impl From<CallError> for Failure {
    fn from(error: CallError) -> Self {
        Self::Call(error)
    }
}

impl From<OutOfBounds> for Failure {
    fn from(error: OutOfBounds) -> Self {
        Self::Layout(error)
    }
}

fn describe(failure: &Failure) -> String {
    match failure {
        Failure::Call(error) => error.to_string(),
        Failure::Layout(error) => error.to_string(),
        Failure::Implausible(reason) => reason.clone(),
    }
}

/// Opens a session with one candidate layout: allocate the device, list the outputs, and when one
/// is connected prove the `GET` command.
fn attempt<D: NvkmsDevice>(
    device: &mut D,
    block: &mut Block,
    version_text: &str,
    gpu: u32,
    candidate: &Candidate,
) -> Result<Session, Attempt> {
    let l: Layout = candidate.layout;
    let rejected = |reason: String| Attempt::Rejected(format!("generation {}: {reason}", candidate.generation));

    if version_text.len() >= l.version_string_len {
        return Err(Attempt::Fatal(BackendError::Unsupported(format!(
            "the version string {version_text:?} does not fit the driver's {}-byte field",
            l.version_string_len
        ))));
    }
    let (device_handle, disps) = allocate(device, block, &l, version_text, gpu)?;

    let free = |device: &mut D, block: &mut Block| {
        if block.clear(l.free_size).is_ok() && block.put_u32(l.free_device_handle_offset, device_handle).is_ok() {
            let _ = device.call(l.cmd_free_device, block, l.free_size);
        }
    };

    let outputs: Vec<Output> = match enumerate(device, block, &l, device_handle, &disps) {
        Ok(outputs) => outputs,
        Err(failure) => {
            free(device, block);
            return Err(rejected(describe(&failure)));
        }
    };

    if let Some(connected) = outputs.iter().find(|o| o.connected)
        && let Err(error) = read_vibrance(device, block, &l, device_handle, connected)
    {
        free(device, block);
        return Err(rejected(format!("reading the vibrance failed: {error}")));
    }
    Ok(Session {
        layout: l,
        generation: candidate.generation,
        confidence: candidate.confidence,
        device_handle,
        disps,
        outputs,
        writes_validated: false,
    })
}

fn allocate<D: NvkmsDevice>(
    device: &mut D,
    block: &mut Block,
    l: &Layout,
    version_text: &str,
    gpu: u32,
) -> Result<(u32, Vec<u32>), Attempt> {
    let layout_error = |error: OutOfBounds| Attempt::Rejected(format!("{error}"));
    block.clear(l.alloc_size).map_err(layout_error)?;
    block
        .put_bytes(l.alloc_version_string_offset, version_text.as_bytes())
        .map_err(layout_error)?;
    block.put_u32(l.alloc_device_id_offset, gpu).map_err(layout_error)?;
    block.put_bytes(l.alloc_no3d_offset, &[1]).map_err(layout_error)?;

    let reply: usize = l.alloc_reply_offset;
    if let Err(error) = device.call(l.cmd_alloc_device, block, l.alloc_size) {
        // A wrong block size is rejected with no status written. A wrong version string is
        // rejected with a status that says so, and that is not worth trying another layout for.
        let status: u32 = block.u32_at(reply + l.alloc_status_offset).unwrap_or(l.status_success);
        return Err(if status == l.status_version_mismatch {
            Attempt::Fatal(BackendError::Unavailable(format!(
                "the NVIDIA kernel module and NVKMS do not agree on the driver version ({version_text}); \
                 reboot if the driver was just updated"
            )))
        } else if status == l.status_no_hardware_available {
            Attempt::Fatal(BackendError::Unsupported(format!("NVKMS has no GPU number {gpu}")))
        } else {
            Attempt::Rejected(format!("ALLOC_DEVICE: {error}"))
        });
    }

    let status: u32 = block.u32_at(reply + l.alloc_status_offset).map_err(layout_error)?;
    let device_handle: u32 = block
        .u32_at(reply + l.alloc_device_handle_offset)
        .map_err(layout_error)?;
    let count: usize =
        usize::try_from(block.u32_at(reply + l.alloc_num_disps_offset).map_err(layout_error)?).unwrap_or(usize::MAX);
    if status != l.status_success || device_handle == 0 || count == 0 || count > l.max_subdevices {
        // The call was accepted but the answer makes no sense: this layout does not describe the
        // driver, and the handle (if any) is not trustworthy enough to free.
        return Err(Attempt::Rejected(format!(
            "ALLOC_DEVICE answered status {status}, handle {device_handle:#x}, {count} disps"
        )));
    }
    let mut disps: Vec<u32> = Vec::with_capacity(count);
    for index in 0..count {
        disps.push(
            block
                .u32_at(reply + l.alloc_disp_handles_offset + 4 * index)
                .map_err(layout_error)?,
        );
    }
    Ok((device_handle, disps))
}

fn kind_name(l: &Layout, raw: u32) -> &'static str {
    match raw {
        t if t == l.connector_type_dp => "DP",
        t if t == l.connector_type_hdmi => "HDMI",
        t if t == l.connector_type_lvds => "LVDS",
        t if t == l.connector_type_vga => "VGA",
        t if t == l.connector_type_dvi_i => "DVI-I",
        t if t == l.connector_type_dvi_d => "DVI-D",
        t if t == l.connector_type_adc => "ADC",
        _ => "Other",
    }
}

fn enumerate<D: NvkmsDevice>(
    device: &mut D,
    block: &mut Block,
    l: &Layout,
    device_handle: u32,
    disps: &[u32],
) -> Result<Vec<Output>, Failure> {
    let mut outputs: Vec<Output> = Vec::new();
    for (disp_index, &disp) in disps.iter().enumerate() {
        block.clear(l.query_disp_size)?;
        block.put_u32(l.query_disp_device_handle_offset, device_handle)?;
        block.put_u32(l.query_disp_disp_handle_offset, disp)?;
        device.call(l.cmd_query_disp, block, l.query_disp_size)?;
        let count: usize = usize::try_from(block.u32_at(l.query_disp_reply_offset + l.disp_num_connectors_offset)?)
            .unwrap_or(usize::MAX);
        if count > l.max_connectors_per_disp {
            return Err(Failure::Implausible(format!("{count} connectors on one disp")));
        }
        let mut connectors: Vec<u32> = Vec::with_capacity(count);
        for index in 0..count {
            connectors.push(block.u32_at(l.query_disp_reply_offset + l.disp_connector_handles_offset + 4 * index)?);
        }

        for connector in connectors {
            block.clear(l.connector_size)?;
            block.put_u32(l.connector_device_handle_offset, device_handle)?;
            block.put_u32(l.connector_disp_handle_offset, disp)?;
            block.put_u32(l.connector_connector_handle_offset, connector)?;
            device.call(l.cmd_query_connector_static_data, block, l.connector_size)?;
            let dpy_id: u32 = block.u32_at(l.connector_reply_offset + l.connector_dpy_id_offset)?;
            let kind: u32 = block.u32_at(l.connector_reply_offset + l.connector_type_offset)?;
            let type_index: u32 = block.u32_at(l.connector_reply_offset + l.connector_type_index_offset)?;

            // The request is zeroed on purpose: its force and override flags must stay false.
            block.clear(l.dynamic_size)?;
            block.put_u32(l.dynamic_device_handle_offset, device_handle)?;
            block.put_u32(l.dynamic_disp_handle_offset, disp)?;
            block.put_u32(l.dynamic_dpy_id_offset, dpy_id)?;
            device.call(l.cmd_query_dpy_dynamic_data, block, l.dynamic_size)?;
            let reply: usize = l.dynamic_reply_offset;
            let connected: bool = block.u8_at(reply + l.dynamic_connected_offset)? != 0;
            let spec: MonitorSpec = if connected {
                read_spec(block, l, reply)?
            } else {
                MonitorSpec::unknown()
            };

            let disp_key: u64 = u64::try_from(disp_index).unwrap_or(0);
            outputs.push(Output {
                handle: OutputHandle((disp_key << 32) | u64::from(dpy_id)),
                disp,
                dpy_id,
                connector: format!("{}-{type_index}", kind_name(l, kind)),
                spec,
                connected,
            });
        }
    }
    Ok(outputs)
}

fn read_spec(block: &Block, l: &Layout, reply: usize) -> Result<MonitorSpec, OutOfBounds> {
    let valid: bool = block.u8_at(reply + l.dynamic_edid_valid_offset)? != 0;
    let size: usize = usize::from(block.u16_at(reply + l.dynamic_edid_size_offset)?).min(l.edid_buffer_size);
    if !valid || size == 0 {
        return Ok(MonitorSpec::unknown());
    }
    let bytes: &[u8] = block.bytes_at(reply + l.dynamic_edid_buffer_offset, size)?;
    Ok(Edid::parse(bytes).map_or_else(|_| MonitorSpec::unknown(), |edid| edid.spec()))
}

fn read_vibrance<D: NvkmsDevice>(
    device: &mut D,
    block: &mut Block,
    l: &Layout,
    device_handle: u32,
    output: &Output,
) -> Result<Vibrance, BackendError> {
    block.clear(l.get_size)?;
    block.put_u32(l.get_device_handle_offset, device_handle)?;
    block.put_u32(l.get_disp_handle_offset, output.disp)?;
    block.put_u32(l.get_dpy_id_offset, output.dpy_id)?;
    block.put_u32(l.get_attribute_offset, l.attribute_digital_vibrance)?;
    device.call(l.cmd_get_dpy_attribute, block, l.get_size)?;
    let raw: i64 = block.i64_at(l.get_reply_offset + l.get_value_offset)?;
    Vibrance::new(raw)
        .map_err(|_| BackendError::Rejected(format!("the driver reported vibrance {raw}, outside the valid range")))
}

fn write_vibrance<D: NvkmsDevice>(
    device: &mut D,
    block: &mut Block,
    l: &Layout,
    device_handle: u32,
    output: &Output,
    value: Vibrance,
) -> Result<(), BackendError> {
    block.clear(l.set_size)?;
    block.put_u32(l.set_device_handle_offset, device_handle)?;
    block.put_u32(l.set_disp_handle_offset, output.disp)?;
    block.put_u32(l.set_dpy_id_offset, output.dpy_id)?;
    block.put_u32(l.set_attribute_offset, l.attribute_digital_vibrance)?;
    block.put_i64(l.set_value_offset, i64::from(value))?;
    device.call(l.cmd_set_dpy_attribute, block, l.set_size)?;
    Ok(())
}

#[cfg(test)]
// Tests unwrap their own fixtures and fail loudly otherwise.
#[allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic)]
mod tests {
    use super::*;
    use crate::{GENERATIONS, Generation, emulator::Emulator};

    /// A valid 128-byte EDID base block: vendor, product code, numeric serial and a name.
    fn edid(vendor: &str, product: u16, serial: u32, name: &str) -> Vec<u8> {
        let mut bytes: Vec<u8> = vec![0; 128];
        bytes[..8].copy_from_slice(&[0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0]);
        let letters: Vec<u16> = vendor.bytes().map(|c| u16::from(c - b'A' + 1)).collect();
        let id: u16 = (letters[0] << 10) | (letters[1] << 5) | letters[2];
        bytes[8..10].copy_from_slice(&id.to_be_bytes());
        bytes[10..12].copy_from_slice(&product.to_le_bytes());
        bytes[12..16].copy_from_slice(&serial.to_le_bytes());
        bytes[72 + 3] = 0xFC;
        let mut text: Vec<u8> = name.bytes().collect();
        text.push(b'\n');
        text.resize(13, b' ');
        bytes[72 + 5..72 + 18].copy_from_slice(&text);
        let sum: u8 = bytes[..127].iter().fold(0, |a, &b| a.wrapping_add(b));
        bytes[127] = 0_u8.wrapping_sub(sum);
        bytes
    }

    fn version_of(generation: &Generation) -> String {
        generation.first.to_string()
    }

    /// An emulator with an HDMI monitor attached and an empty `DisplayPort` connector.
    fn emulator(layout: &Layout, version: &str) -> Emulator {
        Emulator::new(layout, version)
            .with_output(
                layout.connector_type_hdmi,
                0,
                true,
                Some(edid("PHL", 0xC219, 0x8FAC, "PHL 242V8")),
            )
            .with_output(layout.connector_type_dp, 3, false, None)
    }

    fn backend(emulator: &Emulator, version: &str) -> NvkmsBackend<Emulator> {
        NvkmsBackend::new(emulator.clone(), version, 0).expect("a valid version")
    }

    fn hdmi(backend: &mut NvkmsBackend<Emulator>) -> BackendOutput {
        backend
            .outputs()
            .expect("outputs")
            .into_iter()
            .find(|o| o.connected)
            .expect("a connected output")
    }

    #[test]
    fn every_generation_probes_lists_outputs_and_round_trips_a_value() {
        for generation in GENERATIONS {
            let version: String = version_of(generation);
            let emu: Emulator = emulator(&generation.layout, &version);
            let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, &version);

            let capabilities: Capabilities = nvkms.probe().expect("probe");
            assert!(
                capabilities
                    .description
                    .contains(&format!("generation {} (known)", generation.id)),
                "{}",
                capabilities.description
            );
            assert!(capabilities.can_read_back);

            let outputs: Vec<BackendOutput> = nvkms.outputs().expect("outputs");
            let names: Vec<&str> = outputs.iter().map(|o| o.connector.as_str()).collect();
            assert_eq!(names, ["HDMI-0", "DP-3"], "generation {}", generation.id);
            assert_eq!(
                outputs[0].spec,
                MonitorSpec::new("PHL", "PHL 242V8", "0x00008fac"),
                "generation {}",
                generation.id
            );
            assert!(outputs[0].connected && !outputs[1].connected);
            assert_eq!(outputs[1].spec, MonitorSpec::unknown());

            let handle: OutputHandle = outputs[0].handle;
            assert_eq!(nvkms.vibrance(handle), Ok(Vibrance::NEUTRAL));
            assert_eq!(nvkms.set_vibrance(handle, Vibrance::clamped(512)), Ok(()));
            assert_eq!(emu.vibrance_of(0), Some(512), "generation {}", generation.id);
            assert_eq!(nvkms.vibrance(handle), Ok(Vibrance::clamped(512)));
            assert_eq!(nvkms.set_vibrance(handle, Vibrance::MIN), Ok(()));
            assert_eq!(nvkms.reset(handle), Ok(()));
            assert_eq!(emu.vibrance_of(0), Some(0));
        }
    }

    #[test]
    fn a_known_version_is_not_guessed_at() {
        let generation: &Generation = &GENERATIONS[6];
        let version: String = version_of(generation);
        let emu: Emulator = emulator(&generation.layout, &version);
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, &version);
        nvkms.probe().expect("probe");
        // One ALLOC, then the enumeration of two connectors (3 calls each) and the vibrance read.
        assert_eq!(emu.calls(), 1 + 1 + 2 * 2 + 1);
    }

    #[test]
    fn the_device_is_freed_when_the_backend_is_dropped() {
        let generation: &Generation = &GENERATIONS[7];
        let version: String = version_of(generation);
        let emu: Emulator = emulator(&generation.layout, &version);
        {
            let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, &version);
            nvkms.probe().expect("probe");
            assert!(emu.is_open());
        }
        assert!(!emu.is_open());
        assert_eq!(emu.freed(), 1);
    }

    #[test]
    fn nothing_is_sent_until_the_backend_is_used() {
        let generation: &Generation = &GENERATIONS[0];
        let version: String = version_of(generation);
        let emu: Emulator = emulator(&generation.layout, &version);
        drop(backend(&emu, &version));
        assert_eq!(emu.calls(), 0);
    }

    #[test]
    fn a_monitor_plugged_in_later_shows_up_in_the_next_listing() {
        let generation: &Generation = &GENERATIONS[9];
        let version: String = version_of(generation);
        let emu: Emulator = emulator(&generation.layout, &version);
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, &version);
        assert_eq!(nvkms.outputs().expect("outputs").len(), 2);
        emu.add_output(
            generation.layout.connector_type_dp,
            0,
            true,
            Some(edid("DEL", 0x42, 7, "U2720Q")),
        );
        let outputs: Vec<BackendOutput> = nvkms.outputs().expect("outputs");
        assert_eq!(outputs.len(), 3);
        assert_eq!(outputs[2].spec, MonitorSpec::new("DEL", "U2720Q", "0x00000007"));
    }

    #[test]
    fn a_connected_monitor_without_an_edid_has_the_unknown_spec() {
        let generation: &Generation = &GENERATIONS[4];
        let version: String = version_of(generation);
        let emu: Emulator = Emulator::new(&generation.layout, &version).with_output(
            generation.layout.connector_type_lvds,
            0,
            true,
            None,
        );
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, &version);
        let outputs: Vec<BackendOutput> = nvkms.outputs().expect("outputs");
        assert_eq!(outputs[0].spec, MonitorSpec::unknown());
        assert_eq!(outputs[0].connector, "LVDS-0");
        assert!(outputs[0].connected);
    }

    #[test]
    fn a_newer_driver_with_the_newest_layout_is_validated_and_accepted() {
        let newest: &Generation = GENERATIONS.last().expect("a table");
        let emu: Emulator = emulator(&newest.layout, "700.1.2");
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, "700.1.2");
        let description: String = nvkms.probe().expect("probe").description;
        assert!(
            description.contains("generation 10") && description.contains("newer than the table"),
            "{description}"
        );
        let handle: OutputHandle = hdmi(&mut nvkms).handle;
        assert_eq!(nvkms.set_vibrance(handle, Vibrance::clamped(100)), Ok(()));
    }

    #[test]
    fn a_newer_driver_that_kept_an_older_layout_is_found_by_trying_the_generations_in_turn() {
        // The driver claims to be 700.1.2 but has the layout of generation 8 (595). Generations 10 and
        // 9 are tried first and rejected by their block sizes.
        let older: &Generation = &GENERATIONS[7];
        let emu: Emulator = emulator(&older.layout, "700.1.2");
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, "700.1.2");
        let description: String = nvkms.probe().expect("probe").description;
        assert!(description.contains("generation 8"), "{description}");
        assert!(emu.calls() > 3, "the rejected attempts are visible");
        let handle: OutputHandle = hdmi(&mut nvkms).handle;
        assert_eq!(nvkms.set_vibrance(handle, Vibrance::clamped(-200)), Ok(()));
        assert_eq!(emu.vibrance_of(0), Some(-200));
    }

    #[test]
    fn shifted_attribute_command_numbers_are_found_by_trying_the_other_pair() {
        // Sizes of the newest generation, but SET/GET numbered 22/23 instead of 21/22.
        let mut layout: Layout = GENERATIONS.last().expect("a table").layout;
        layout.cmd_set_dpy_attribute = 22;
        layout.cmd_get_dpy_attribute = 23;
        let emu: Emulator = emulator(&layout, "700.1.2");
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, "700.1.2");
        nvkms.probe().expect("the second pair validates");
        let handle: OutputHandle = hdmi(&mut nvkms).handle;
        assert_eq!(nvkms.set_vibrance(handle, Vibrance::clamped(300)), Ok(()));
        assert_eq!(emu.vibrance_of(0), Some(300));
    }

    #[test]
    fn a_layout_nobody_knows_is_refused_with_the_driver_version() {
        let mut layout: Layout = GENERATIONS.last().expect("a table").layout;
        layout.alloc_size = 1234;
        let emu: Emulator = emulator(&layout, "700.1.2");
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, "700.1.2");
        match nvkms.probe() {
            Err(BackendError::Unsupported(message)) => {
                assert!(
                    message.contains("700.1.2") && message.contains("no known NVKMS layout"),
                    "{message}"
                );
            }
            other => panic!("expected Unsupported, got {other:?}"),
        }
        assert!(!emu.is_open());
    }

    #[test]
    fn a_wrong_layout_for_a_known_version_is_refused_not_worked_around() {
        let known: &Generation = &GENERATIONS[6];
        let mut layout: Layout = known.layout;
        layout.dynamic_size += 8;
        let version: String = version_of(known);
        let emu: Emulator = emulator(&layout, &version);
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, &version);
        assert!(matches!(nvkms.probe(), Err(BackendError::Unsupported(_))));
        assert!(!emu.is_open(), "a rejected attempt gives the device back");
        assert_eq!(emu.freed(), 1);
    }

    #[test]
    fn a_driver_that_answers_nonsense_is_refused() {
        let known: &Generation = &GENERATIONS[7];
        let version: String = version_of(known);
        let emu: Emulator = emulator(&known.layout, &version);
        emu.set_bogus_disp_count(Some(99));
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, &version);
        match nvkms.probe() {
            Err(BackendError::Unsupported(message)) => {
                assert!(message.contains("99 disps"), "refused for the wrong reason: {message}");
            }
            other => panic!("expected Unsupported, got {other:?}"),
        }
        assert!(
            !emu.is_open() || emu.freed() == 0,
            "an implausible handle is not trusted enough to free"
        );
    }

    #[test]
    fn a_version_mismatch_says_to_reboot_and_does_not_try_other_layouts() {
        let known: &Generation = &GENERATIONS[7];
        let emu: Emulator = emulator(&known.layout, "595.99.02");
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, "595.99.03");
        match nvkms.probe() {
            Err(BackendError::Unavailable(message)) => assert!(message.contains("reboot"), "{message}"),
            other => panic!("expected Unavailable, got {other:?}"),
        }
        assert_eq!(emu.calls(), 1, "a version mismatch is final");
    }

    #[test]
    fn asking_for_a_gpu_that_does_not_exist_is_unsupported() {
        let known: &Generation = &GENERATIONS[7];
        let version: String = version_of(known);
        let emu: Emulator = emulator(&known.layout, &version);
        let mut nvkms: NvkmsBackend<Emulator> = NvkmsBackend::new(emu.clone(), &version, 3).expect("version");
        assert!(matches!(nvkms.probe(), Err(BackendError::Unsupported(message)) if message.contains("GPU number 3")));
    }

    #[test]
    fn a_pair_that_does_not_validate_stops_the_write_before_the_requested_value_is_sent() {
        let known: &Generation = &GENERATIONS[7];
        let version: String = version_of(known);
        let emu: Emulator = emulator(&known.layout, &version);
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, &version);
        let handle: OutputHandle = hdmi(&mut nvkms).handle;
        emu.set_corrupt_readback(true);
        // The emulator reads back one more than it stored. Validation reads 1, writes 1, reads 2.
        assert_eq!(
            nvkms.set_vibrance(handle, Vibrance::clamped(400)),
            Err(BackendError::ReadbackMismatch {
                expected: Vibrance::clamped(1),
                actual:   Vibrance::clamped(2),
            })
        );
        assert_ne!(
            emu.vibrance_of(0),
            Some(400),
            "the requested value must not be sent when the pair does not validate"
        );
    }

    #[test]
    fn a_readback_that_differs_after_validation_is_reported_with_both_values() {
        let known: &Generation = &GENERATIONS[7];
        let version: String = version_of(known);
        let emu: Emulator = emulator(&known.layout, &version);
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, &version);
        let handle: OutputHandle = hdmi(&mut nvkms).handle;
        assert_eq!(nvkms.set_vibrance(handle, Vibrance::clamped(10)), Ok(()), "validates the pair");
        emu.set_corrupt_readback(true);
        assert_eq!(
            nvkms.set_vibrance(handle, Vibrance::clamped(400)),
            Err(BackendError::ReadbackMismatch {
                expected: Vibrance::clamped(400),
                actual:   Vibrance::clamped(401),
            })
        );
    }

    #[test]
    fn the_first_write_is_preceded_by_a_no_op_write_that_proves_the_command_pair() {
        let known: &Generation = &GENERATIONS[7];
        let version: String = version_of(known);
        let emu: Emulator = emulator(&known.layout, &version);
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, &version);
        let handle: OutputHandle = hdmi(&mut nvkms).handle;
        let before: u32 = emu.calls();
        assert_eq!(nvkms.set_vibrance(handle, Vibrance::clamped(700)), Ok(()));
        // Validation: GET, SET (same value), GET. Then the real write: SET, GET.
        assert_eq!(emu.calls() - before, 5);
        let before_second: u32 = emu.calls();
        assert_eq!(nvkms.set_vibrance(handle, Vibrance::clamped(-5)), Ok(()));
        assert_eq!(emu.calls() - before_second, 2, "the pair is proven once per session");
    }

    #[test]
    fn the_validation_write_never_changes_the_value_even_when_the_real_write_is_rejected_later() {
        let known: &Generation = &GENERATIONS[7];
        let version: String = version_of(known);
        let emu: Emulator = emulator(&known.layout, &version);
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, &version);
        let handle: OutputHandle = hdmi(&mut nvkms).handle;
        assert_eq!(nvkms.set_vibrance(handle, Vibrance::clamped(250)), Ok(()));
        emu.set_reject_set(true);
        assert!(nvkms.set_vibrance(handle, Vibrance::clamped(900)).is_err());
        assert_eq!(emu.vibrance_of(0), Some(250), "a rejected write leaves the previous value");
    }

    #[test]
    fn a_rejected_write_leaves_the_value_alone_and_says_so() {
        let known: &Generation = &GENERATIONS[7];
        let version: String = version_of(known);
        let emu: Emulator = emulator(&known.layout, &version);
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, &version);
        let handle: OutputHandle = hdmi(&mut nvkms).handle;
        emu.set_reject_set(true);
        assert!(matches!(
            nvkms.set_vibrance(handle, Vibrance::clamped(400)),
            Err(BackendError::Rejected(_))
        ));
        assert_eq!(emu.vibrance_of(0), Some(0));
    }

    #[test]
    fn writing_to_an_output_without_a_monitor_is_unsupported() {
        let known: &Generation = &GENERATIONS[7];
        let version: String = version_of(known);
        let emu: Emulator = emulator(&known.layout, &version);
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, &version);
        let empty: OutputHandle = nvkms.outputs().expect("outputs")[1].handle;
        assert!(matches!(
            nvkms.set_vibrance(empty, Vibrance::clamped(1)),
            Err(BackendError::Unsupported(_))
        ));
    }

    #[test]
    fn an_unknown_handle_is_reported_as_such() {
        let known: &Generation = &GENERATIONS[7];
        let version: String = version_of(known);
        let emu: Emulator = emulator(&known.layout, &version);
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, &version);
        assert_eq!(
            nvkms.vibrance(OutputHandle(0xDEAD)),
            Err(BackendError::UnknownOutput(OutputHandle(0xDEAD)))
        );
        assert_eq!(
            nvkms.set_vibrance(OutputHandle(0xDEAD), Vibrance::NEUTRAL),
            Err(BackendError::UnknownOutput(OutputHandle(0xDEAD)))
        );
    }

    #[test]
    fn a_text_that_is_not_a_version_is_refused_up_front() {
        let emu: Emulator = emulator(&GENERATIONS[0].layout, "x");
        assert!(matches!(
            NvkmsBackend::new(emu.clone(), "not-a-version", 0),
            Err(BackendError::Unsupported(_))
        ));
        assert!(matches!(NvkmsBackend::new(emu, "", 0), Err(BackendError::Unsupported(_))));
    }

    #[test]
    fn a_version_string_too_long_for_the_driver_field_is_refused_without_calling_the_driver() {
        let long: &str = "1000000000.1000000000.1000000000";
        assert_eq!(long.len(), 32);
        let emu: Emulator = emulator(&GENERATIONS[0].layout, long);
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, long);
        assert!(matches!(nvkms.probe(), Err(BackendError::Unsupported(message)) if message.contains("does not fit")));
        assert_eq!(emu.calls(), 0);
    }

    #[test]
    fn surrounding_whitespace_in_the_version_is_ignored() {
        let known: &Generation = &GENERATIONS[7];
        let version: String = version_of(known);
        let emu: Emulator = emulator(&known.layout, &version);
        let mut nvkms: NvkmsBackend<Emulator> = backend(&emu, &format!("{version}\n"));
        assert_eq!(nvkms.driver_version(), version);
        assert!(nvkms.probe().is_ok());
    }

    /// Hardware test, read only: the real driver on this machine. Run with
    /// `VIVID_HW_TESTS=1 cargo test -p vivid-nvkms -- --ignored --nocapture`.
    #[test]
    #[ignore = "needs an NVIDIA GPU with the kernel module loaded; run with VIVID_HW_TESTS=1 and --ignored"]
    fn the_real_driver_probes_lists_outputs_and_accepts_a_no_op_write() {
        if env::var_os("VIVID_HW_TESTS").is_none() {
            return;
        }
        let mut nvkms: NvkmsBackend<NvkmsNode> = NvkmsBackend::open_system().expect("the NVIDIA device opens");
        let capabilities: Capabilities = nvkms.probe().expect("the layout validates against the real driver");
        println!("probe: {}", capabilities.description);
        let outputs: Vec<BackendOutput> = nvkms.outputs().expect("outputs");
        for output in &outputs {
            println!("  {:<10} connected={:<5} {}", output.connector, output.connected, output.spec);
        }
        let Some(connected) = outputs.into_iter().find(|o| o.connected) else {
            // Not a failure: on a hybrid laptop the NVIDIA GPU often owns only the HDMI port, and
            // the internal panel belongs to the integrated GPU.
            println!(
                "NOTE: no output driven by this NVIDIA GPU has a monitor attached. The layout was accepted for the \
                 allocation and the listing, but the SET and GET commands could not be validated. Attach a monitor \
                 to a port the NVIDIA GPU drives (on a hybrid laptop usually HDMI) and run again."
            );
            return;
        };
        let current: Vibrance = nvkms.vibrance(connected.handle).expect("a readable vibrance");
        println!("vibrance of {}: {current}", connected.connector);
        // Writing the value it already has changes nothing on screen and proves the SET path.
        assert_eq!(nvkms.set_vibrance(connected.handle, current), Ok(()));
        assert_eq!(nvkms.vibrance(connected.handle), Ok(current));
    }
}
