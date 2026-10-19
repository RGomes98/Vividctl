//! SP-05 throwaway proof of concept: one stable identity for a monitor across NVKMS, DRM and Mutter.
//!
//! The idea under test: build the identity from the EDID with the same rule Mutter uses for its
//! monitor spec (vendor, product, serial), so all three sources can be joined on it and it matches
//! what GNOME stores. The connector name is deliberately not part of it: the three sources name
//! the same connector differently (NVKMS `HDMI-0`, DRM `HDMI-A-1`, Mutter `HDMI-1`).
//!
//! Commands: `show` (default) joins the three sources on this machine; `test` is the unit tests'
//! scenarios printed for reading (`cargo test` runs them for real).

use std::{env, fs, process::Command};

use rustix::{
    fd::OwnedFd,
    fs::{Mode, OFlags, open},
    ioctl::{Opcode, Updater, ioctl, opcode},
};

// ---------------------------------------------------------------------------------------------
// EDID

/// The few EDID fields that identify a monitor. Parsing follows EDID 1.3/1.4 base block layout.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Edid {
    vendor:            String,
    product_code:      u16,
    serial_number:     u32,
    week:              u8,
    year:              u16,
    /// Monitor name descriptor (tag 0xFC).
    dsc_product_name:  Option<String>,
    /// Serial number descriptor (tag 0xFF).
    dsc_serial_number: Option<String>,
    checksum_ok:       bool,
}

fn descriptor_text(block: &[u8]) -> Option<String> {
    let text: &[u8] = block.get(5..18)?;
    let end: usize = text.iter().position(|&b| b == 0x0A).unwrap_or(text.len());
    let value: String = String::from_utf8_lossy(&text[..end]).trim_end().to_owned();
    if value.is_empty() { None } else { Some(value) }
}

fn parse_edid(bytes: &[u8]) -> Option<Edid> {
    if bytes.len() < 128 || bytes[..8] != [0x00, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x00] {
        return None;
    }
    let id: u16 = u16::from_be_bytes([bytes[8], bytes[9]]);
    let letter = |shift: u16| char::from(((id >> shift) & 0x1F) as u8 + b'A' - 1);
    let mut edid: Edid = Edid {
        vendor:            [letter(10), letter(5), letter(0)].iter().collect(),
        product_code:      u16::from_le_bytes([bytes[10], bytes[11]]),
        serial_number:     u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]),
        week:              bytes[16],
        year:              1990 + u16::from(bytes[17]),
        dsc_product_name:  None,
        dsc_serial_number: None,
        checksum_ok:       bytes[..128].iter().fold(0u8, |sum, &b| sum.wrapping_add(b)) == 0,
    };
    for offset in [54usize, 72, 90, 108] {
        let block: &[u8] = &bytes[offset..offset + 18];
        if block[..3] == [0, 0, 0] {
            match block[3] {
                0xFC => edid.dsc_product_name = descriptor_text(block),
                0xFF => edid.dsc_serial_number = descriptor_text(block),
                _ => {}
            }
        }
    }
    Some(edid)
}

// ---------------------------------------------------------------------------------------------
// Identity

/// Same three strings Mutter keeps in its monitor spec (`set_output_details_from_edid`, identical
/// in mutter 46.2 and 50.1): vendor, product (name or `0x%04x`), serial (descriptor or `0x%08x`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct MonitorSpec {
    vendor:  String,
    product: String,
    serial:  String,
}

impl MonitorSpec {
    fn from_edid(edid: Option<&Edid>) -> Self {
        let Some(e) = edid else {
            let unknown = || "unknown".to_owned();
            return Self {
                vendor:  unknown(),
                product: unknown(),
                serial:  unknown(),
            };
        };
        Self {
            vendor:  e.vendor.clone(),
            product: e
                .dsc_product_name
                .clone()
                .unwrap_or_else(|| format!("0x{:04x}", e.product_code)),
            serial:  e
                .dsc_serial_number
                .clone()
                .unwrap_or_else(|| format!("0x{:08x}", e.serial_number)),
        }
    }
}

/// Numeric serial values that factory-set many unrelated models (measured on 92k real EDIDs).
const SENTINEL_SERIALS: [u32; 5] = [0, 1, 0x0100_0e00, 0x0101_0101, 0x5445];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SerialQuality {
    /// A serial descriptor, or a numeric serial that is not a known factory value.
    Usable,
    /// Only a factory-default numeric serial: identical monitors will collide.
    Sentinel,
    /// No EDID at all.
    Missing,
}

fn serial_quality(edid: Option<&Edid>) -> SerialQuality {
    match edid {
        None => SerialQuality::Missing,
        Some(e) if e.dsc_serial_number.is_some() => SerialQuality::Usable,
        Some(e) if SENTINEL_SERIALS.contains(&e.serial_number) => SerialQuality::Sentinel,
        Some(_) => SerialQuality::Usable,
    }
}

/// What the profile store keys on: the spec, plus a slot when several connected outputs share a
/// spec. Slots follow the sorted connector names, so swapping cables swaps the slots.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OutputId {
    spec: MonitorSpec,
    slot: Option<u32>,
}

impl std::fmt::Display for OutputId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}:{}:{}", self.spec.vendor, self.spec.product, self.spec.serial)?;
        if let Some(slot) = self.slot {
            write!(f, "#{slot}")?;
        }
        Ok(())
    }
}

/// Assigns ids to the connected outputs, given as (stable connector name, spec).
fn assign_ids(outputs: &[(String, MonitorSpec)]) -> Vec<OutputId> {
    let mut ids: Vec<OutputId> = outputs
        .iter()
        .map(|(_, spec)| OutputId {
            spec: spec.clone(),
            slot: None,
        })
        .collect();
    let mut order: Vec<usize> = (0..outputs.len()).collect();
    order.sort_by(|&a, &b| outputs[a].0.cmp(&outputs[b].0));
    for (i, &index) in order.iter().enumerate() {
        let spec: &MonitorSpec = &outputs[index].1;
        let same: usize = outputs.iter().filter(|(_, s)| s == spec).count();
        if same > 1 {
            let slot: usize = order[..i].iter().filter(|&&j| &outputs[j].1 == spec).count() + 1;
            ids[index].slot = Some(slot as u32);
        }
    }
    ids
}

// ---------------------------------------------------------------------------------------------
// Source 1: DRM sysfs

struct DrmOutput {
    name:      String,
    connected: bool,
    edid:      Vec<u8>,
}

fn drm_outputs() -> Vec<DrmOutput> {
    let mut outputs: Vec<DrmOutput> = Vec::new();
    let Ok(entries) = fs::read_dir("/sys/class/drm") else {
        return outputs;
    };
    for entry in entries.flatten() {
        let dir_name: String = entry.file_name().to_string_lossy().into_owned();
        let Some((_, connector)) = dir_name.split_once('-').filter(|_| dir_name.starts_with("card")) else {
            continue;
        };
        let path = entry.path();
        let status: String = fs::read_to_string(path.join("status")).unwrap_or_default();
        outputs.push(DrmOutput {
            name:      connector.to_owned(),
            connected: status.trim() == "connected",
            edid:      fs::read(path.join("edid")).unwrap_or_default(),
        });
    }
    outputs.sort_by(|a, b| a.name.cmp(&b.name));
    outputs
}

// ---------------------------------------------------------------------------------------------
// Source 2: NVKMS (layout offsets from layout.c in docs/spikes/sp-01-nvkms and layout2.c here)

#[repr(C)]
struct IoctlParams {
    cmd:     u32,
    size:    u32,
    address: u64,
}

const NVKMS_IOCTL: Opcode = opcode::read_write::<IoctlParams>(b'm', 0);

#[repr(C, align(8))]
struct Blob<const N: usize>([u8; N]);

impl<const N: usize> Blob<N> {
    fn zeroed() -> Box<Self> {
        Box::new(Self([0; N]))
    }
    fn put_u32(&mut self, offset: usize, value: u32) {
        self.0[offset..offset + 4].copy_from_slice(&value.to_ne_bytes());
    }
    fn u32_at(&self, offset: usize) -> u32 {
        u32::from_ne_bytes(self.0[offset..offset + 4].try_into().unwrap_or_default())
    }
    fn u16_at(&self, offset: usize) -> u16 {
        u16::from_ne_bytes(self.0[offset..offset + 2].try_into().unwrap_or_default())
    }
    fn str_at(&self, offset: usize, capacity: usize) -> String {
        let field: &[u8] = &self.0[offset..offset + capacity];
        let end: usize = field.iter().position(|&b| b == 0).unwrap_or(capacity);
        String::from_utf8_lossy(&field[..end]).into_owned()
    }
}

fn nvkms<const N: usize>(fd: &OwnedFd, cmd: u32, blob: &mut Blob<N>) -> Result<(), String> {
    let mut params: IoctlParams = IoctlParams {
        cmd,
        size: N as u32,
        address: std::ptr::from_mut(blob) as usize as u64,
    };
    // SAFETY: NVKMS_IOCTL is `_IOWR('m', 0, IoctlParams)`; `params` is live and `address` points to a
    // live writable buffer of exactly `size` bytes for the duration of the call.
    unsafe { ioctl(fd, Updater::<{ NVKMS_IOCTL }, IoctlParams>::new(&mut params)) }
        .map_err(|e| format!("ioctl {cmd}: {e}"))
}

struct NvkmsOutput {
    name:      String,
    dpy_id:    u32,
    connected: bool,
    edid:      Vec<u8>,
    dp_guid:   Option<[u8; 16]>,
}

fn nvkms_outputs() -> Result<Vec<NvkmsOutput>, String> {
    const ALLOC: usize = 1512;
    const QUERY_DISP: usize = 172;
    const CONNECTOR: usize = 44;
    const DYNAMIC: usize = 37168;
    let version: String =
        fs::read_to_string("/sys/module/nvidia/version").map_err(|e| format!("nvidia module not loaded: {e}"))?;
    let fd: OwnedFd =
        open("/dev/nvidia-modeset", OFlags::RDWR | OFlags::CLOEXEC, Mode::empty()).map_err(|e| e.to_string())?;

    let mut alloc = Blob::<ALLOC>::zeroed();
    let v: &[u8] = version.trim().as_bytes();
    alloc.0[..v.len().min(31)].copy_from_slice(&v[..v.len().min(31)]);
    alloc.0[40] = 1; // no3d
    nvkms(&fd, 0, &mut alloc)?;
    let device: u32 = alloc.u32_at(624 + 4);
    let num_disps: usize = alloc.u32_at(624 + 16) as usize;
    if num_disps == 0 || num_disps > 8 {
        return Err(format!("implausible numDisps {num_disps}"));
    }

    let mut outputs: Vec<NvkmsOutput> = Vec::new();
    for d in 0..num_disps {
        let disp: u32 = alloc.u32_at(624 + 20 + d * 4);
        let mut q = Blob::<QUERY_DISP>::zeroed();
        q.put_u32(0, device);
        q.put_u32(4, disp);
        nvkms(&fd, 2, &mut q)?;
        let count: usize = q.u32_at(8 + 16) as usize;
        if count > 16 {
            return Err(format!("implausible numConnectors {count}"));
        }
        for c in 0..count {
            let mut st = Blob::<CONNECTOR>::zeroed();
            st.put_u32(0, device);
            st.put_u32(4, disp);
            st.put_u32(8, q.u32_at(8 + 20 + c * 4));
            nvkms(&fd, 3, &mut st)?;
            let dpy_id: u32 = st.u32_at(12);
            let kind: &str = match st.u32_at(12 + 12) {
                0 => "DP",
                6 => "HDMI",
                7 => "USB-C",
                5 => "LVDS",
                _ => "Other",
            };
            let mut dynamic = Blob::<DYNAMIC>::zeroed(); // zeroed on purpose: force/override flags stay false
            dynamic.put_u32(0, device);
            dynamic.put_u32(4, disp);
            dynamic.put_u32(8, dpy_id);
            nvkms(&fd, 6, &mut dynamic)?;
            let reply: usize = 2072;
            let size: usize = usize::from(dynamic.u16_at(reply + 214)).min(2048);
            let edid_valid: bool = dynamic.0[reply + 216] != 0;
            let guid_valid: bool = dynamic.0[reply + 160] != 0;
            let mut guid: [u8; 16] = [0; 16];
            guid.copy_from_slice(&dynamic.0[reply + 161..reply + 177]);
            outputs.push(NvkmsOutput {
                name: format!("{kind}-{}", st.u32_at(12 + 16)),
                dpy_id,
                connected: dynamic.0[reply + 132] != 0,
                edid: if edid_valid {
                    dynamic.0[reply + 217..reply + 217 + size].to_vec()
                } else {
                    Vec::new()
                },
                dp_guid: guid_valid.then_some(guid),
            });
            let _ = dynamic.str_at(reply, 128);
        }
    }
    let mut free = Blob::<8>::zeroed();
    free.put_u32(0, device);
    nvkms(&fd, 1, &mut free)?;
    Ok(outputs)
}

// ---------------------------------------------------------------------------------------------
// Source 3: Mutter (`GetCurrentState` over gdbus, parsed from its text form)

#[derive(Debug)]
struct MutterMonitor {
    connector: String,
    spec:      MonitorSpec,
}

fn mutter_monitors() -> Vec<MutterMonitor> {
    let Ok(out) = Command::new("gdbus")
        .args([
            "call",
            "--session",
            "--dest",
            "org.gnome.Mutter.DisplayConfig",
            "--object-path",
            "/org/gnome/Mutter/DisplayConfig",
            "--method",
            "org.gnome.Mutter.DisplayConfig.GetCurrentState",
        ])
        .output()
    else {
        return Vec::new();
    };
    let text: String = String::from_utf8_lossy(&out.stdout).into_owned();
    // Each monitor starts with a tuple of four quoted strings: (('HDMI-1', 'PHL', 'PHL 242V8', '0x00008fac'), [
    let mut monitors: Vec<MutterMonitor> = Vec::new();
    let mut rest: &str = &text;
    while let Some(at) = rest.find("(('") {
        rest = &rest[at + 2..];
        let mut fields: Vec<String> = Vec::new();
        let mut cursor: &str = rest;
        while fields.len() < 4 {
            let Some(start) = cursor.find('\'') else { break };
            let mut value: String = String::new();
            let mut chars = cursor[start + 1..].char_indices();
            let mut end: usize = cursor.len();
            while let Some((i, ch)) = chars.next() {
                match ch {
                    '\\' => {
                        if let Some((_, next)) = chars.next() {
                            value.push(next);
                        }
                    }
                    '\'' => {
                        end = start + 1 + i + 1;
                        break;
                    }
                    other => value.push(other),
                }
            }
            fields.push(value);
            cursor = &cursor[end..];
        }
        if fields.len() == 4 {
            monitors.push(MutterMonitor {
                connector: fields[0].clone(),
                spec:      MonitorSpec {
                    vendor:  fields[1].clone(),
                    product: fields[2].clone(),
                    serial:  fields[3].clone(),
                },
            });
        }
    }
    monitors
}

// ---------------------------------------------------------------------------------------------

fn describe(edid: &[u8]) -> String {
    match parse_edid(edid) {
        None => format!("no usable EDID ({} bytes)", edid.len()),
        Some(e) => format!(
            "{} product={:#06x} serial#={:#010x} name={:?} serial-desc={:?} made={}/{} checksum={}",
            e.vendor,
            e.product_code,
            e.serial_number,
            e.dsc_product_name,
            e.dsc_serial_number,
            e.week,
            e.year,
            e.checksum_ok
        ),
    }
}

fn show() {
    let drm: Vec<DrmOutput> = drm_outputs();
    let nv: Result<Vec<NvkmsOutput>, String> = nvkms_outputs();
    let mutter: Vec<MutterMonitor> = mutter_monitors();

    println!("== DRM sysfs ({} connectors)", drm.len());
    for o in &drm {
        println!(
            "  {:<12} connected={:<5} edid_bytes={:<4} {}",
            o.name,
            o.connected,
            o.edid.len(),
            if o.connected { describe(&o.edid) } else { String::new() }
        );
    }
    println!("== NVKMS");
    match &nv {
        Err(e) => println!("  unavailable: {e}"),
        Ok(list) => {
            for o in list {
                println!(
                    "  {:<8} dpy={:#06x} connected={:<5} edid_bytes={:<4} dp_guid={}",
                    o.name,
                    o.dpy_id,
                    o.connected,
                    o.edid.len(),
                    o.dp_guid
                        .map_or("-".to_owned(), |g| g.iter().map(|b| format!("{b:02x}")).collect())
                );
                if o.connected {
                    println!("           {}", describe(&o.edid));
                }
            }
        }
    }
    println!("== Mutter ({} monitors)", mutter.len());
    for m in &mutter {
        println!(
            "  {:<8} spec = ({}, {}, {})",
            m.connector, m.spec.vendor, m.spec.product, m.spec.serial
        );
    }

    println!("== Join");
    let nv_list: Vec<NvkmsOutput> = nv.unwrap_or_default();
    for d in drm.iter().filter(|o| o.connected) {
        let edid: Option<Edid> = parse_edid(&d.edid);
        let spec: MonitorSpec = MonitorSpec::from_edid(edid.as_ref());
        let nv_match = nv_list
            .iter()
            .find(|n| n.connected && MonitorSpec::from_edid(parse_edid(&n.edid).as_ref()) == spec);
        let mu_match = mutter.iter().find(|m| m.spec == spec);
        let same_bytes: Option<bool> =
            nv_match.map(|n| n.edid[..128.min(n.edid.len())] == d.edid[..128.min(d.edid.len())]);
        println!("  id = {}", assign_ids(&[(d.name.clone(), spec.clone())])[0]);
        println!(
            "     DRM {}   NVKMS {}   Mutter {}",
            d.name,
            nv_match.map_or("no match", |n| n.name.as_str()),
            mu_match.map_or("no match", |m| m.connector.as_str())
        );
        println!(
            "     serial quality: {:?}   NVKMS and DRM base block identical: {:?}",
            serial_quality(edid.as_ref()),
            same_bytes
        );
    }
}

fn main() {
    match env::args().nth(1).as_deref() {
        None | Some("show") => show(),
        Some(other) => {
            eprintln!("unknown command {other:?}; use `show` (and `cargo test` for the collision scenarios)")
        }
    }
}

// ---------------------------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Builds a valid 128-byte base block.
    fn edid(vendor: &str, product: u16, serial: u32, name: Option<&str>, ser_desc: Option<&str>, year: u8) -> Vec<u8> {
        let mut b: Vec<u8> = vec![0; 128];
        b[..8].copy_from_slice(&[0, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0]);
        let l: Vec<u16> = vendor.bytes().map(|c| u16::from(c - b'A' + 1)).collect();
        let id: u16 = (l[0] << 10) | (l[1] << 5) | l[2];
        b[8..10].copy_from_slice(&id.to_be_bytes());
        b[10..12].copy_from_slice(&product.to_le_bytes());
        b[12..16].copy_from_slice(&serial.to_le_bytes());
        b[16] = 10;
        b[17] = year;
        let mut put = |offset: usize, tag: u8, text: &str| {
            b[offset + 3] = tag;
            let mut t: Vec<u8> = text.bytes().collect();
            t.push(0x0A);
            t.resize(13, b' ');
            b[offset + 5..offset + 18].copy_from_slice(&t);
        };
        if let Some(n) = name {
            put(72, 0xFC, n);
        }
        if let Some(s) = ser_desc {
            put(90, 0xFF, s);
        }
        let sum: u8 = b[..127].iter().fold(0u8, |a, &x| a.wrapping_add(x));
        b[127] = 0u8.wrapping_sub(sum);
        b
    }

    fn spec(bytes: &[u8]) -> MonitorSpec {
        MonitorSpec::from_edid(parse_edid(bytes).as_ref())
    }

    #[test]
    fn spec_uses_descriptors_when_present() {
        let s: MonitorSpec = spec(&edid("PHL", 0x1234, 0x8fac, Some("PHL 242V8"), Some("ABC123"), 30));
        assert_eq!(
            (s.vendor.as_str(), s.product.as_str(), s.serial.as_str()),
            ("PHL", "PHL 242V8", "ABC123")
        );
    }

    #[test]
    fn spec_falls_back_to_numeric_fields_like_mutter() {
        let s: MonitorSpec = spec(&edid("PHL", 0x1234, 0x8fac, None, None, 30));
        assert_eq!((s.product.as_str(), s.serial.as_str()), ("0x1234", "0x00008fac"));
    }

    #[test]
    fn no_edid_is_unknown() {
        let s: MonitorSpec = MonitorSpec::from_edid(None);
        assert_eq!(
            s.to_owned(),
            MonitorSpec {
                vendor:  "unknown".into(),
                product: "unknown".into(),
                serial:  "unknown".into(),
            }
        );
    }

    #[test]
    fn rejects_garbage() {
        assert!(parse_edid(&[0; 128]).is_none());
        assert!(parse_edid(&[1, 2, 3]).is_none());
    }

    #[test]
    fn serial_quality_flags_factory_defaults() {
        let sentinel = edid("DEL", 1, 0x0101_0101, Some("X"), None, 30);
        let usable = edid("DEL", 1, 0x1234_5678, Some("X"), None, 30);
        let described = edid("DEL", 1, 0, Some("X"), Some("SN1"), 30);
        assert_eq!(serial_quality(parse_edid(&sentinel).as_ref()), SerialQuality::Sentinel);
        assert_eq!(serial_quality(parse_edid(&usable).as_ref()), SerialQuality::Usable);
        assert_eq!(serial_quality(parse_edid(&described).as_ref()), SerialQuality::Usable);
        assert_eq!(serial_quality(None), SerialQuality::Missing);
    }

    #[test]
    fn distinct_monitors_get_distinct_ids_without_slots() {
        let a: MonitorSpec = spec(&edid("DEL", 1, 0x11, Some("A"), None, 30));
        let b: MonitorSpec = spec(&edid("DEL", 1, 0x22, Some("A"), None, 30));
        let ids: Vec<OutputId> = assign_ids(&[("DP-1".into(), a), ("HDMI-A-1".into(), b)]);
        assert_ne!(ids[0], ids[1]);
        assert!(ids.iter().all(|i| i.slot.is_none()));
    }

    #[test]
    fn identical_twins_get_slots_in_connector_order() {
        let twin: MonitorSpec = spec(&edid("DEL", 1, 0x0101_0101, Some("U2720Q"), None, 30));
        let ids: Vec<OutputId> = assign_ids(&[("HDMI-A-1".into(), twin.clone()), ("DP-1".into(), twin)]);
        // DP-1 sorts before HDMI-A-1, so it gets slot 1.
        assert_eq!(ids[1].slot, Some(1));
        assert_eq!(ids[0].slot, Some(2));
    }

    #[test]
    fn slots_are_stable_when_an_unrelated_monitor_comes_and_goes() {
        let twin: MonitorSpec = spec(&edid("DEL", 1, 0, Some("U2720Q"), None, 30));
        let other: MonitorSpec = spec(&edid("PHL", 2, 0x77, Some("P"), None, 30));
        let without: Vec<OutputId> = assign_ids(&[("DP-1".into(), twin.clone()), ("DP-2".into(), twin.clone())]);
        let with: Vec<OutputId> = assign_ids(&[
            ("DP-1".into(), twin.clone()),
            ("DP-2".into(), twin),
            ("HDMI-A-1".into(), other),
        ]);
        assert_eq!(without[0], with[0]);
        assert_eq!(without[1], with[1]);
    }

    #[test]
    fn same_monitor_on_another_port_keeps_its_id() {
        let m: MonitorSpec = spec(&edid("PHL", 0x1234, 0x8fac, Some("PHL 242V8"), None, 30));
        let on_hdmi: Vec<OutputId> = assign_ids(&[("HDMI-A-1".into(), m.clone())]);
        let on_dp: Vec<OutputId> = assign_ids(&[("DP-3".into(), m)]);
        assert_eq!(on_hdmi, on_dp);
    }

    #[test]
    fn display_form_is_readable() {
        let m: MonitorSpec = spec(&edid("PHL", 0x1234, 0x8fac, Some("PHL 242V8"), None, 30));
        assert_eq!(assign_ids(&[("X".into(), m)])[0].to_string(), "PHL:PHL 242V8:0x00008fac");
    }
}
