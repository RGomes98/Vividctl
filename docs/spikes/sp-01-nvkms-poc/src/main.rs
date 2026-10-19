//! SP-01 throwaway proof of concept: set NVIDIA Digital Vibrance from pure Rust.
//!
//! Talks to `/dev/nvidia-modeset` (NVKMS) with `rustix`, no bindgen and no C++.
//! Layouts are "sparse": each ioctl parameter block is a zeroed byte buffer of the exact
//! size the kernel expects, and only the fields we need are read or written at known
//! offsets. Sizes and offsets come from `layout.c`, compiled against the driver headers.
//!
//! Driver tested: 595.99.02 (open-gpu-kernel-modules tag 595.99.02).

use std::{env, fs, process, thread, time::Duration};

use rustix::{
    fd::OwnedFd,
    fs::{Mode, OFlags, open},
    ioctl::{Opcode, Updater, ioctl, opcode},
};

/// Layout of one NVKMS ioctl: sizes and field offsets, generated from the C headers.
mod abi {
    pub const CMD_ALLOC_DEVICE: u32 = 0;
    pub const CMD_FREE_DEVICE: u32 = 1;
    pub const CMD_QUERY_DISP: u32 = 2;
    pub const CMD_QUERY_CONNECTOR_STATIC_DATA: u32 = 3;
    pub const CMD_QUERY_DPY_DYNAMIC_DATA: u32 = 6;
    pub const CMD_SET_DPY_ATTRIBUTE: u32 = 22;
    pub const CMD_GET_DPY_ATTRIBUTE: u32 = 23;

    pub const ATTRIBUTE_DIGITAL_VIBRANCE: u32 = 10;
    pub const VERSION_STRING_LEN: usize = 32;
    pub const MAX_CONNECTORS_PER_DISP: usize = 16;
    pub const GPU_STRING_LEN: usize = 80;
    pub const DPY_NAME_LEN: usize = 128;

    pub const STATUS_SUCCESS: u32 = 0;
    pub const STATUS_VERSION_MISMATCH: u32 = 1;

    pub mod alloc {
        pub const SIZE: usize = 1512;
        pub const REQ_VERSION: usize = 0;
        pub const REQ_RM_DEVICE_ID: usize = 32;
        pub const REQ_NO_3D: usize = 40;
        pub const REP_STATUS: usize = 624;
        pub const REP_DEVICE_HANDLE: usize = 624 + 4;
        pub const REP_NUM_DISPS: usize = 624 + 16;
        pub const REP_DISP_HANDLES: usize = 624 + 20;
    }

    pub mod free {
        pub const SIZE: usize = 8;
        pub const REQ_DEVICE: usize = 0;
    }

    pub mod query_disp {
        pub const SIZE: usize = 172;
        pub const REQ_DEVICE: usize = 0;
        pub const REQ_DISP: usize = 4;
        pub const REP_NUM_CONNECTORS: usize = 8 + 16;
        pub const REP_CONNECTOR_HANDLES: usize = 8 + 20;
        pub const REP_GPU_STRING: usize = 8 + 84;
    }

    pub mod connector_static {
        pub const SIZE: usize = 44;
        pub const REQ_DEVICE: usize = 0;
        pub const REQ_DISP: usize = 4;
        pub const REQ_CONNECTOR: usize = 8;
        pub const REP_DPY_ID: usize = 12;
        pub const REP_TYPE: usize = 12 + 12;
        pub const REP_TYPE_INDEX: usize = 12 + 16;
    }

    pub mod dpy_dynamic {
        pub const SIZE: usize = 37168;
        pub const REQ_DEVICE: usize = 0;
        pub const REQ_DISP: usize = 4;
        pub const REQ_DPY_ID: usize = 8;
        pub const REP_NAME: usize = 2072;
        pub const REP_CONNECTED: usize = 2072 + 132;
    }

    pub mod set_attr {
        pub const SIZE: usize = 32;
        pub const REQ_DEVICE: usize = 0;
        pub const REQ_DISP: usize = 4;
        pub const REQ_DPY_ID: usize = 8;
        pub const REQ_ATTRIBUTE: usize = 12;
        pub const REQ_VALUE: usize = 16;
    }

    pub mod get_attr {
        pub const SIZE: usize = 24;
        pub const REQ_DEVICE: usize = 0;
        pub const REQ_DISP: usize = 4;
        pub const REQ_DPY_ID: usize = 8;
        pub const REQ_ATTRIBUTE: usize = 12;
        pub const REP_VALUE: usize = 16;
    }

    pub fn connector_type_name(value: u32) -> &'static str {
        match value {
            0 => "DP",
            1 => "VGA",
            2 => "DVI-I",
            3 => "DVI-D",
            4 => "ADC",
            5 => "LVDS",
            6 => "HDMI",
            7 => "USB-C",
            8 => "DSI",
            9 => "DP-SERIALIZER",
            _ => "Unknown",
        }
    }
}

/// `struct NvKmsIoctlParams`: the envelope every NVKMS ioctl goes through.
#[repr(C)]
struct IoctlParams {
    cmd:     u32,
    size:    u32,
    address: u64,
}

/// `_IOWR('m', 0, struct NvKmsIoctlParams)`.
const NVKMS_IOCTL: Opcode = opcode::read_write::<IoctlParams>(b'm', 0);

/// Zeroed parameter block, 8-byte aligned like the C structs.
#[repr(C, align(8))]
struct Blob<const N: usize>([u8; N]);

impl<const N: usize> Blob<N> {
    fn zeroed() -> Box<Self> {
        Box::new(Self([0; N]))
    }

    fn put_u32(&mut self, offset: usize, value: u32) {
        self.0[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }

    fn put_i64(&mut self, offset: usize, value: i64) {
        self.0[offset..offset + 8].copy_from_slice(&value.to_ne_bytes());
    }

    fn put_str(&mut self, offset: usize, capacity: usize, text: &str) {
        let bytes: &[u8] = text.as_bytes();
        let len: usize = bytes.len().min(capacity - 1);
        self.0[offset..offset + len].copy_from_slice(&bytes[..len]);
    }

    fn u32_at(&self, offset: usize) -> u32 {
        u32::from_ne_bytes(self.0[offset..offset + 4].try_into().unwrap_or_default())
    }

    fn i64_at(&self, offset: usize) -> i64 {
        i64::from_ne_bytes(self.0[offset..offset + 8].try_into().unwrap_or_default())
    }

    fn str_at(&self, offset: usize, capacity: usize) -> String {
        let field: &[u8] = &self.0[offset..offset + capacity];
        let end: usize = field.iter().position(|&b| b == 0).unwrap_or(capacity);
        String::from_utf8_lossy(&field[..end]).into_owned()
    }
}

/// Issues one NVKMS ioctl. `size` is what we tell the kernel the block size is; normally
/// it equals `N`, and the wrong-size probes deliberately pass something else (never above `N`).
fn nvkms<const N: usize>(fd: &OwnedFd, cmd: u32, size: usize, blob: &mut Blob<N>) -> rustix::io::Result<()> {
    assert!(size <= N, "claimed size must not exceed the buffer");
    let mut params: IoctlParams = IoctlParams {
        cmd,
        size: size as u32,
        address: std::ptr::from_mut(blob) as usize as u64,
    };
    // SAFETY: NVKMS_IOCTL is `_IOWR('m', 0, IoctlParams)`, the opcode nvidia-modeset implements.
    // `params` is a live, correctly laid out `IoctlParams` and `address` points to a live,
    // writable buffer of at least `size` bytes for the duration of the call.
    unsafe { ioctl(fd, Updater::<{ NVKMS_IOCTL }, IoctlParams>::new(&mut params)) }
}

struct Output {
    disp:       u32,
    dpy_id:     u32,
    kind:       &'static str,
    type_index: u32,
    connected:  bool,
    name:       String,
}

struct Device {
    fd:          OwnedFd,
    handle:      u32,
    disps:       Vec<u32>,
    gpu_strings: Vec<String>,
}

fn driver_version() -> Result<String, String> {
    let raw: String = fs::read_to_string("/sys/module/nvidia/version")
        .map_err(|e| format!("cannot read /sys/module/nvidia/version ({e}); is the nvidia module loaded?"))?;
    Ok(raw.trim().to_owned())
}

fn open_modeset() -> Result<OwnedFd, String> {
    open("/dev/nvidia-modeset", OFlags::RDWR | OFlags::CLOEXEC, Mode::empty())
        .map_err(|e| format!("cannot open /dev/nvidia-modeset: {e}"))
}

impl Device {
    fn open(gpu: u32, version: &str) -> Result<Self, String> {
        let fd: OwnedFd = open_modeset()?;
        let mut alloc = Blob::<{ abi::alloc::SIZE }>::zeroed();
        alloc.put_str(abi::alloc::REQ_VERSION, abi::VERSION_STRING_LEN, version);
        alloc.put_u32(abi::alloc::REQ_RM_DEVICE_ID, gpu);
        alloc.0[abi::alloc::REQ_NO_3D] = 1;
        nvkms(&fd, abi::CMD_ALLOC_DEVICE, abi::alloc::SIZE, &mut alloc).map_err(|e| {
            format!(
                "ALLOC_DEVICE failed: {e} (reply status {})",
                alloc.u32_at(abi::alloc::REP_STATUS)
            )
        })?;
        let status: u32 = alloc.u32_at(abi::alloc::REP_STATUS);
        if status != abi::STATUS_SUCCESS {
            return Err(format!("ALLOC_DEVICE status {status}"));
        }
        let num_disps: u32 = alloc.u32_at(abi::alloc::REP_NUM_DISPS);
        if num_disps == 0 || num_disps > 8 {
            return Err(format!("implausible numDisps {num_disps}; refusing to continue"));
        }
        let handle: u32 = alloc.u32_at(abi::alloc::REP_DEVICE_HANDLE);
        let disps: Vec<u32> = (0..num_disps as usize)
            .map(|i| alloc.u32_at(abi::alloc::REP_DISP_HANDLES + i * 4))
            .collect();
        Ok(Self {
            fd,
            handle,
            disps,
            gpu_strings: Vec::new(),
        })
    }

    fn outputs(&mut self) -> Result<Vec<Output>, String> {
        let mut outputs: Vec<Output> = Vec::new();
        for &disp in &self.disps.clone() {
            let mut query = Blob::<{ abi::query_disp::SIZE }>::zeroed();
            query.put_u32(abi::query_disp::REQ_DEVICE, self.handle);
            query.put_u32(abi::query_disp::REQ_DISP, disp);
            nvkms(&self.fd, abi::CMD_QUERY_DISP, abi::query_disp::SIZE, &mut query)
                .map_err(|e| format!("QUERY_DISP failed: {e}"))?;
            self.gpu_strings
                .push(query.str_at(abi::query_disp::REP_GPU_STRING, abi::GPU_STRING_LEN));
            let count: usize = query.u32_at(abi::query_disp::REP_NUM_CONNECTORS) as usize;
            if count > abi::MAX_CONNECTORS_PER_DISP {
                return Err(format!("implausible numConnectors {count}; refusing to continue"));
            }
            for index in 0..count {
                let connector: u32 = query.u32_at(abi::query_disp::REP_CONNECTOR_HANDLES + index * 4);

                let mut stat = Blob::<{ abi::connector_static::SIZE }>::zeroed();
                stat.put_u32(abi::connector_static::REQ_DEVICE, self.handle);
                stat.put_u32(abi::connector_static::REQ_DISP, disp);
                stat.put_u32(abi::connector_static::REQ_CONNECTOR, connector);
                nvkms(
                    &self.fd,
                    abi::CMD_QUERY_CONNECTOR_STATIC_DATA,
                    abi::connector_static::SIZE,
                    &mut stat,
                )
                .map_err(|e| format!("QUERY_CONNECTOR_STATIC_DATA failed: {e}"))?;
                let dpy_id: u32 = stat.u32_at(abi::connector_static::REP_DPY_ID);

                // The request block is zeroed on purpose: its force/override flags must stay false.
                let mut dynamic = Blob::<{ abi::dpy_dynamic::SIZE }>::zeroed();
                dynamic.put_u32(abi::dpy_dynamic::REQ_DEVICE, self.handle);
                dynamic.put_u32(abi::dpy_dynamic::REQ_DISP, disp);
                dynamic.put_u32(abi::dpy_dynamic::REQ_DPY_ID, dpy_id);
                nvkms(&self.fd, abi::CMD_QUERY_DPY_DYNAMIC_DATA, abi::dpy_dynamic::SIZE, &mut dynamic)
                    .map_err(|e| format!("QUERY_DPY_DYNAMIC_DATA failed: {e}"))?;

                outputs.push(Output {
                    disp,
                    dpy_id,
                    kind: abi::connector_type_name(stat.u32_at(abi::connector_static::REP_TYPE)),
                    type_index: stat.u32_at(abi::connector_static::REP_TYPE_INDEX),
                    connected: dynamic.0[abi::dpy_dynamic::REP_CONNECTED] != 0,
                    name: dynamic.str_at(abi::dpy_dynamic::REP_NAME, abi::DPY_NAME_LEN),
                });
            }
        }
        Ok(outputs)
    }

    fn get_vibrance(&self, output: &Output) -> Result<i64, String> {
        let mut get = Blob::<{ abi::get_attr::SIZE }>::zeroed();
        get.put_u32(abi::get_attr::REQ_DEVICE, self.handle);
        get.put_u32(abi::get_attr::REQ_DISP, output.disp);
        get.put_u32(abi::get_attr::REQ_DPY_ID, output.dpy_id);
        get.put_u32(abi::get_attr::REQ_ATTRIBUTE, abi::ATTRIBUTE_DIGITAL_VIBRANCE);
        nvkms(&self.fd, abi::CMD_GET_DPY_ATTRIBUTE, abi::get_attr::SIZE, &mut get)
            .map_err(|e| format!("GET_DPY_ATTRIBUTE failed: {e}"))?;
        Ok(get.i64_at(abi::get_attr::REP_VALUE))
    }

    fn set_vibrance(&self, output: &Output, value: i64) -> Result<(), String> {
        let mut set = Blob::<{ abi::set_attr::SIZE }>::zeroed();
        set.put_u32(abi::set_attr::REQ_DEVICE, self.handle);
        set.put_u32(abi::set_attr::REQ_DISP, output.disp);
        set.put_u32(abi::set_attr::REQ_DPY_ID, output.dpy_id);
        set.put_u32(abi::set_attr::REQ_ATTRIBUTE, abi::ATTRIBUTE_DIGITAL_VIBRANCE);
        set.put_i64(abi::set_attr::REQ_VALUE, value);
        nvkms(&self.fd, abi::CMD_SET_DPY_ATTRIBUTE, abi::set_attr::SIZE, &mut set)
            .map_err(|e| format!("SET_DPY_ATTRIBUTE failed: {e}"))
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        let mut free = Blob::<{ abi::free::SIZE }>::zeroed();
        free.put_u32(abi::free::REQ_DEVICE, self.handle);
        if let Err(e) = nvkms(&self.fd, abi::CMD_FREE_DEVICE, abi::free::SIZE, &mut free) {
            eprintln!("warning: FREE_DEVICE failed: {e}");
        }
    }
}

fn print_outputs(device: &Device, outputs: &[Output]) {
    for (i, gpu) in device.gpu_strings.iter().enumerate() {
        println!("disp {i}: {gpu}");
    }
    for out in outputs {
        let vibrance: String = if out.connected {
            device.get_vibrance(out).map_or_else(|e| e, |v| v.to_string())
        } else {
            "-".to_owned()
        };
        println!(
            "  dpy {:#010x}  {}-{}  connected={}  vibrance={}  name={:?}",
            out.dpy_id, out.kind, out.type_index, out.connected, vibrance, out.name
        );
    }
}

/// Documents what the kernel does with a wrong version string, wrong paramSize and a bad handle.
fn probe_errors(version: &str) -> Result<(), String> {
    println!("driver version from sysfs: {version}");

    {
        let fd: OwnedFd = open_modeset()?;
        let mut alloc = Blob::<{ abi::alloc::SIZE }>::zeroed();
        alloc.put_str(abi::alloc::REQ_VERSION, abi::VERSION_STRING_LEN, "0.0.0");
        let result = nvkms(&fd, abi::CMD_ALLOC_DEVICE, abi::alloc::SIZE, &mut alloc);
        println!(
            "wrong version string  -> ioctl={result:?}  reply.status={} (VERSION_MISMATCH={})",
            alloc.u32_at(abi::alloc::REP_STATUS),
            abi::STATUS_VERSION_MISMATCH
        );
    }

    for (label, size) in [
        ("paramSize - 4", abi::alloc::SIZE - 4),
        ("paramSize - 1", abi::alloc::SIZE - 1),
    ] {
        let fd: OwnedFd = open_modeset()?;
        let mut alloc = Blob::<{ abi::alloc::SIZE + 16 }>::zeroed();
        alloc.put_str(abi::alloc::REQ_VERSION, abi::VERSION_STRING_LEN, version);
        let result = nvkms(&fd, abi::CMD_ALLOC_DEVICE, size, &mut alloc);
        println!(
            "{label:<21} -> ioctl={result:?}  reply.status={}",
            alloc.u32_at(abi::alloc::REP_STATUS)
        );
    }

    {
        let fd: OwnedFd = open_modeset()?;
        let mut alloc = Blob::<{ abi::alloc::SIZE + 16 }>::zeroed();
        alloc.put_str(abi::alloc::REQ_VERSION, abi::VERSION_STRING_LEN, version);
        let result = nvkms(&fd, abi::CMD_ALLOC_DEVICE, abi::alloc::SIZE + 4, &mut alloc);
        println!(
            "paramSize + 4         -> ioctl={result:?}  reply.status={}",
            alloc.u32_at(abi::alloc::REP_STATUS)
        );
    }

    {
        let fd: OwnedFd = open_modeset()?;
        let mut query = Blob::<{ abi::query_disp::SIZE }>::zeroed();
        query.put_u32(abi::query_disp::REQ_DEVICE, 0xdead_beef);
        let result = nvkms(&fd, abi::CMD_QUERY_DISP, abi::query_disp::SIZE, &mut query);
        println!("bad device handle     -> ioctl={result:?}");
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let args: Vec<String> = env::args().skip(1).collect();
    let version: String = driver_version()?;
    let command: &str = args.first().map_or("list", String::as_str);

    if command == "probe-errors" {
        return probe_errors(&version);
    }

    println!("driver version: {version}");
    let mut device: Device = Device::open(0, &version)?;
    let outputs: Vec<Output> = device.outputs()?;
    let parse = |index: usize| -> Result<i64, String> {
        let value: i64 = args
            .get(index)
            .ok_or("missing value")?
            .parse()
            .map_err(|e| format!("bad number: {e}"))?;
        if !(-1024..=1023).contains(&value) {
            return Err(format!("{value} is outside -1024..=1023"));
        }
        Ok(value)
    };

    match command {
        "list" | "get" => print_outputs(&device, &outputs),
        "set" | "reset" => {
            let value: i64 = if command == "reset" { 0 } else { parse(1)? };
            for out in outputs.iter().filter(|o| o.connected) {
                device.set_vibrance(out, value)?;
                let read_back: i64 = device.get_vibrance(out)?;
                println!("{}-{}: set {value}, read back {read_back}", out.kind, out.type_index);
            }
        }
        "pulse" => {
            let value: i64 = parse(1)?;
            let seconds: u64 = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(8);
            let mut originals: Vec<(&Output, i64)> = Vec::new();
            for out in outputs.iter().filter(|o| o.connected) {
                originals.push((out, device.get_vibrance(out)?));
            }
            for (out, _) in &originals {
                device.set_vibrance(out, value)?;
                println!("{}-{}: set {value}", out.kind, out.type_index);
            }
            println!("holding for {seconds}s, then restoring the original values...");
            thread::sleep(Duration::from_secs(seconds));
            for (out, original) in &originals {
                device.set_vibrance(out, *original)?;
                println!(
                    "{}-{}: restored {original} (read back {})",
                    out.kind,
                    out.type_index,
                    device.get_vibrance(out)?
                );
            }
        }
        other => {
            return Err(format!(
                "unknown command {other:?}; use list|get|set N|reset|pulse N [SECS]|probe-errors"
            ));
        }
    }
    Ok(())
}

fn main() {
    if let Err(message) = run() {
        eprintln!("error: {message}");
        process::exit(1);
    }
}
