# Architecture

## Crates

| Crate | Role | Notes |
|---|---|---|
| `vivid-core` | Domain types (`Vibrance`, `OutputId`, `Profile`), minimal EDID reader, `Backend` trait, errors | No I/O, no dependencies |
| `vivid-color` | DVC to saturation math, shared by previews and shaders | Only `vivid-core`. The curve is provisional until SP-08 |
| `vivid-config` | TOML config, XDG paths, atomic writes | |
| `vivid-outputs` | Output discovery via sysfs/DRM (reads the EDID bytes, `vivid-core` parses them) | |
| `vivid-nvkms` | NVIDIA NVKMS ioctl backend | Only crate with `unsafe` |
| `vivid-gnome` | Client of the GNOME Shell extension and of Mutter's monitor list (D-Bus, `dbus` crate). Speaks saturation, not vibrance | Behind the `gnome` feature |
| `vivid-engine` | Use cases: resolve, apply, verify, fall back, diagnose. Holds the `GnomeBackend` adapter (vibrance to saturation, via `vivid-color`) and keeps each monitor once, under the first backend that sees it | Shared by CLI and GUI |
| `vivid-cli` | `vividctl` binary | |
| `vivid-gui` | `vivid` binary, GTK4 + libadwaita | |
| `xtask` | abi-check, completions, man pages, packaging | |

Dependency direction: `core` <- `color`, `config`, `outputs`, `nvkms`, `gnome` <- `engine` <- `cli`, `gui`.
Internal dependency edges are declared in a crate's `Cargo.toml` when the crate starts using them, because `cargo-machete` rejects unused ones in CI.

## Backend selection

The engine picks a backend per output:

1. Output scanned out by an NVIDIA GPU: `vivid-nvkms` (acts in the display engine, covers games, fullscreen, video, VRR).
2. Output owned by another GPU (hybrid laptop panel) with a supported compositor: `vivid-gnome`.
3. Otherwise: report "unsupported" with the reason and the remedy.

## NVKMS and driver updates

- The kernel checks `versionString` and each ioctl's `paramSize` (`nvkms.c`, `ALLOC_DEVICE` and the dispatch table). A wrong size is rejected without side effects, so layout probing is safe.
- Version comes from `/sys/module/nvidia/version` at runtime, never compiled in.
- Layouts are hand-written `#[repr(C)]` structs grouped into ABI generations by driver version range. No bindgen.
- Replies are validated before any `SET`.
- A scheduled CI job compiles `tests/abi` against new open-gpu-kernel-modules tags and compares `sizeof`/`offsetof` with the Rust layouts; drift opens an issue.

## Persistence

Config lives in `~/.config/vividctl/config.toml`; monitors are identified by EDID, not by position. The profile is applied at login by an XDG autostart entry running `vividctl apply`. There is no resident process. Re-applying on hotplug/resume is decided by SP-03 and SP-09.

## Known risks

- NVKMS is an undocumented NVIDIA interface.
- The GNOME Shell extension needs maintenance per Shell release; fullscreen games require disabling unredirect, which has a performance cost.
- Hybrid-laptop support depends on testers with real hardware.
- NVKMS-derived layouts carry the MIT notice in `NOTICE`.
