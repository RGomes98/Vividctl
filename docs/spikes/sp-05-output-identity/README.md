# SP-05: stable output identity

Issue: #6. Status: done, decision in `docs/adr/0003-output-identity.md`.

## Question

How do we identify a monitor so that a saved profile follows the monitor, not an index or a port? The same physical output shows up in NVKMS, in DRM sysfs and in Mutter, under different names. nvibrant addresses outputs by their position in the connector list (`nvibrant 0 0 0 0 512`), which broke on multi-monitor setups (nvibrant issue #14).

## Environment

- Ubuntu 24.04.4, GNOME Shell 46 / mutter 46.2, Wayland, NVIDIA open driver 595.99.02
- One monitor: Philips PHL 242V8 on HDMI. No second monitor, no laptop, no DisplayPort monitor, so no real collision case.

## What the three sources say about the same monitor

`cargo run` in this folder reads all three and joins them.

| Source | Connector name | Connectors listed | EDID |
|---|---|---|---|
| NVKMS | `HDMI-0` (type + 0-based index) | 7 (`DP-0` to `DP-5`, `HDMI-0`) | 256 bytes, in the dynamic-data reply |
| DRM sysfs | `HDMI-A-1` | 4 (`DP-1` to `DP-3`, `HDMI-A-1`) | 256 bytes, `/sys/class/drm/card1-HDMI-A-1/edid` |
| Mutter (`GetCurrentState`) | `HDMI-1` | 1 (connected monitors only) | not exposed, only `(vendor, product, serial)` |

- The three names differ, and NVKMS and DRM list different numbers of connectors. Connector names cannot be the join key.
- **The NVKMS EDID and the DRM EDID are byte-identical.**
- Mutter's tuple for this monitor is `(PHL, PHL 242V8, 0x00008fac)`. Parsing the EDID with Mutter's rule gives the same three strings from NVKMS and from DRM.

### Mutter's rule

`set_output_details_from_edid` in `src/backends/meta-output.c` is identical in mutter 46.2 and 50.1:

- vendor: the 3-letter manufacturer code
- product: the monitor name descriptor (tag `0xFC`) if non-empty and valid UTF-8, otherwise `0x%04x` of the product code
- serial: the serial descriptor (tag `0xFF`) if non-empty and valid UTF-8, otherwise `0x%08x` of the numeric serial
- no EDID at all: `unknown` for all three (`meta_monitor_generate_spec`)

Mutter's own `MetaMonitorSpec` equality also includes the connector name. We leave the connector out on purpose, because it is the part that differs between sources and changes with the port.

This monitor has no serial descriptor, so its serial is the numeric one (`0x00008fac`, usable).

## How reliable are EDID identity fields? (92,054 real EDIDs)

`edid-stats.py` runs over a sample of the public collection `linuxhw/EDID`: 14 vendors (AOC, ASUS, Acer, BenQ, Dell, HP, Lenovo, Philips, Samsung, ViewSonic, LG Display, AU Optronics, BOE, Chimei Innolux). Run: `python3 -I edid-stats.py <clone>/Digital`.

| Measure | Result |
|---|---|
| Numeric serial usable (not zero, not a factory default) | 64.2 % |
| ASCII serial descriptor (`0xFF`) present | 77.0 % |
| **No usable serial of any kind** | **12.0 %** |
| Factory-default numeric serials seen on many unrelated models | `0x0`, `0x1`, `0x01000e00`, `0x01010101`, `0x5445` |
| Triples with a usable numeric serial that show more than one base block | 6.9 % (3,119 of 45,380) |
| Most common differing bytes in those | 16 and 17 (manufacture week and year), and 116 to 122 (the last descriptor) |

Reading:

- About 1 in 8 monitors cannot tell two units of the same model apart. Two identical monitors on one desk will collide for these.
- Even with a usable serial, 6.9 % of triples appear with different manufacture dates, so some serials repeat across units. A serial is not a guarantee of uniqueness.
- The same triple also appears with different descriptor bytes, so a hash of the whole EDID is not a stable identity: it would change with firmware.

Caveats: the collection is a sample of 14 vendors, not all monitors; it is contributed by Linux users, so it skews to desktops and laptops of that crowd; and it may have scrubbed ASCII serials (the numeric field is clearly real: 11.8 % are zero and the factory defaults repeat thousands of times), so the `0xFF` figure is indicative only.

## The PoC

`src/main.rs` (standalone crate, `rustix` only):

- `parse_edid`: base block, vendor, product, numeric serial, week/year, descriptors `0xFC` and `0xFF`, checksum.
- `MonitorSpec::from_edid`: Mutter's rule.
- `serial_quality`: `Usable`, `Sentinel` (factory defaults above) or `Missing` (no EDID).
- `assign_ids`: gives a slot (`#1`, `#2`, ...) to connected outputs that share a spec, in sorted connector-name order.
- Sources: DRM sysfs, NVKMS (EDID at reply offset 217, size at 214, validity at 216; DP GUID at 160 and 161) and Mutter through `gdbus`.

`cargo test` has 10 scenarios, among them: descriptors versus numeric fallback, no EDID, garbage input, factory-default serials, distinct monitors get distinct ids, identical twins get slots in connector order, slots stay stable when an unrelated monitor comes or goes, and the same monitor on another port keeps its id.

`layout.c` is the C probe for the NVKMS offsets used here (compiled against the 595.99.02 headers).

## Not covered

- Real multi-monitor setups and real identical twins: only one monitor was available. Collisions are tested with synthetic EDIDs.
- DisplayPort, MST hubs and docks. NVKMS reports a DP GUID (`dp.guid`) that could identify MST branches; it was not exercised.
- Hybrid laptops: the internal panel is an eDP connector on the iGPU's DRM card. The EDID path is the same, but it was not run (#8).
- KVM switches and adapters that rewrite the EDID.
- Behaviour across a monitor firmware update.
