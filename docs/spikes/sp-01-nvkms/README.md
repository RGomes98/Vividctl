# SP-01: NVKMS Digital Vibrance in pure Rust

Issue: #2. Status: done, decision is go. The ADR is `docs/adr/0001-nvkms-pure-rust.md`.

## Environment

- Ubuntu 24.04.4 LTS, kernel 7.0.0-38-generic, GNOME on Wayland
- NVIDIA open kernel module, driver 595.99.02 (`/sys/module/nvidia/version`)
- GeForce GTX 1660 Ti (TU116), one monitor: Philips PHL 242V8 on HDMI-0
- Headers: open-gpu-kernel-modules tag `595.99.02`

## Method

1. Compile ``layout.c` (this folder)` against the tag's headers. It prints `sizeof`, `offsetof` and the enum values of the six ioctls we need, plus `GET_DPY_ATTRIBUTE`.
2. The PoC (`src/main.rs`, this folder) uses those numbers as constants. Each ioctl parameter block is a zeroed byte buffer of the exact size the kernel expects, and only the fields we need are read or written at their offsets. No bindgen, no C++, no full struct copies.
3. The only dependency is `rustix` 1.1.5 (`default-features = false`, features `std` and `fs`). Transitive: `bitflags`, `linux-raw-sys`.

Build: `cargo build` inside the PoC directory. Commands: `list`, `get`, `set N`, `reset`, `pulse N [SECS]`, `probe-errors`.

## Layout facts (595.99.02)

| Item                                    | Value                                                                                                                                                                |
| --------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `NVKMS_IOCTL_IOWR`                      | `_IOWR('m', 0, struct NvKmsIoctlParams)` = `0xC0106D00` (envelope is 16 bytes: `cmd`, `size`, `address`)                                                             |
| Commands used                           | ALLOC_DEVICE 0, FREE_DEVICE 1, QUERY_DISP 2, QUERY_CONNECTOR_STATIC_DATA 3, QUERY_DPY_DYNAMIC_DATA 6, SET_DPY_ATTRIBUTE 22, GET_DPY_ATTRIBUTE 23                     |
| `NV_KMS_DPY_ATTRIBUTE_DIGITAL_VIBRANCE` | 10                                                                                                                                                                   |
| `AllocDeviceParams`                     | size 1512; request 620 bytes (`versionString[32]` @0, `deviceId` @32, `no3d` @40); reply @624 (`status` @0, `deviceHandle` @4, `numDisps` @16, `dispHandles[8]` @20) |
| `FreeDeviceParams`                      | 8                                                                                                                                                                    |
| `QueryDispParams`                       | 172; reply @8 (`numConnectors` @16, `connectorHandles[16]` @20, `gpuString[80]` @84)                                                                                 |
| `QueryConnectorStaticDataParams`        | 44; reply @12 (`dpyId` @0, `type` @12, `typeIndex` @16)                                                                                                              |
| `QueryDpyDynamicDataParams`             | 37168; request 2072 bytes (`dpyId` @8, then override/force flags that must stay zero); reply @2072 (`name[128]` @0, `connected` @132)                                |
| `SetDpyAttributeParams`                 | 32; `dpyId` @8, `attribute` @12, `value` (i64) @16                                                                                                                   |
| `GetDpyAttributeParams`                 | 24; request 16 bytes; reply @16 (`value` i64)                                                                                                                        |

`GET_DPY_ATTRIBUTE` exists, so the value can be read back after a `SET`. nvibrant never reads it.

## Results so far

`list` (read only) enumerated one disp (`GPU:0 ... PCI:0000:01:00.0`) with seven connectors: `DP-0..DP-5` disconnected and `HDMI-0` connected (`Philips PHL 242V8`). `GET_DPY_ATTRIBUTE` returned 512 for HDMI-0. That value came from the nvibrant autostart entry that was installed on this machine (`nvibrant 0 0 0 0 512`), since removed.

### Behaviour on wrong input (`probe-errors`)

| Input                                      | ioctl result | Reply                             |
| ------------------------------------------ | ------------ | --------------------------------- |
| `versionString` = `"0.0.0"`                | `EPERM`      | `status` = 1 (`VERSION_MISMATCH`) |
| `size` = expected - 4                      | `EPERM`      | untouched (`status` 0)            |
| `size` = expected - 1                      | `EPERM`      | untouched                         |
| `size` = expected + 4                      | `EPERM`      | untouched                         |
| `deviceHandle` = `0xdeadbeef` (QUERY_DISP) | `EPERM`      | n/a                               |

Every rejection is `EPERM`, so errno alone cannot tell a wrong version from a wrong size or a bad handle. Only the version mismatch writes a reply status. The size check is an exact match, in both directions, and rejected calls have no visible side effect. This confirms that probing layouts by size is safe, as assumed in the plan.

## Observations for later issues

- nvibrant's `QueryDpyDynamicData` request block is not zero-initialised in its C++ (`NvKmsQueryDpyDynamicDataParams dynamicData;`), so its force/override flags are whatever was on the stack. The PoC zeroes the whole block.
- nvibrant takes per-output values as positional arguments in connector order (`nvibrant 0 0 0 0 512` meant "the fifth connector"). Output identity needs a better key (SP-05, #6).
- Sparse layouts keep the Rust side tiny: the per-driver data is a table of sizes and offsets, which `tests/abi` can compare mechanically (SP-02, #3).

## Visual test

Run after a fresh login, with the starting value at 0 (read back before the test). `pulse 1000 10` set 1000 on HDMI-0, held it for 10 seconds and restored the original value; the readback after the restore was 0. The maintainer confirmed by eye that the Philips PHL 242V8 became visibly more saturated and then returned to normal.
