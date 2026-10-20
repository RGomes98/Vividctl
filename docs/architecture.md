# Architecture

## Crates

| Crate | Role | State | Notes |
|---|---|---|---|
| `vivid-core` | Domain types (`Vibrance`, `OutputId`, `MonitorSpec`, `Profile`), minimal EDID reader, the `Backend` trait, errors | Done | No I/O, no dependencies |
| `vivid-color` | Vibrance to saturation math, shared by the GNOME backend and later by previews | Provisional curve | Only `vivid-core`. The curve is replaced by the SP-08 calibration |
| `vivid-config` | TOML config, XDG paths, atomic writes | Empty crate | Planned (#16) |
| `vivid-outputs` | Output discovery via sysfs/DRM (reads the EDID bytes, `vivid-core` parses them) | Empty crate | Planned (#15) |
| `vivid-nvkms` | NVIDIA NVKMS ioctl backend: transport, ABI generations, driver version probing | Done | The only crate with `unsafe` (one `ioctl` call) |
| `vivid-gnome` | Client of the GNOME Shell extension and of Mutter's monitor list (D-Bus, `dbus` crate). Speaks saturation, not vibrance. Carries the extension's files (embedded) and the `gsettings` switch that enables them | Done | Behind the `gnome` feature |
| `vivid-engine` | Use cases: list outputs of every backend, name a monitor, read and change it, install the extension. Holds the `GnomeBackend` adapter (vibrance to saturation, via `vivid-color`) and keeps each monitor once, under the first backend that sees it | Done, except profiles | Shared by CLI and GUI |
| `vivid-cli` | `vividctl` binary | Done, except `apply`, `preset`, `doctor` | Thin: parses, calls the engine, prints |
| `vivid-gui` | `vivid` binary, GTK4 + libadwaita | Empty crate | Phase 3 |
| `xtask` | Developer automation (`cargo xtask`) | Empty | Planned: completions, man pages, packaging |

Dependency direction: `core` <- `color`, `config`, `outputs`, `nvkms`, `gnome` <- `engine` <- `cli`, `gui`. The reason `vivid-gnome` does not depend on `vivid-color` is that edge: the adapter that joins them lives in the engine.
Internal dependency edges are declared in a crate's `Cargo.toml` when the crate starts using them, because `cargo-machete` rejects unused ones in CI.

Outside the workspace crates:

| Path | What it is |
|---|---|
| `extensions/gnome/` | The GNOME Shell extension (JavaScript, the only language the Shell accepts), its README with the D-Bus interface, and `test/`, the container harness that runs it in a headless Shell |
| `tests/abi/` | The C probe and scripts that read NVIDIA's headers and produce the table of ABI generations; see its README |
| `scripts/check-budgets.sh` | Enforces the binary size and crate budgets |
| `docs/adr/`, `docs/spikes/` | The decisions and the measurements behind them |

## How a `set` flows

1. `vivid-cli` parses the command line and builds an `Engine` with the system backends: `nvkms`, then `gnome`. A backend that cannot start stays in the engine with its reason, so `list` can say why it is missing.
2. The engine asks each backend for its outputs. Connected outputs get an identity (`vendor:product:serial`, with a `#slot` only when two connected outputs are identical) built from the monitor's EDID, with the same rule as Mutter (ADR-0003).
3. A monitor that two backends see (one on an NVIDIA port is seen by NVKMS and by Mutter, under different connector names) is kept once, under the first backend. Matching is by identity and by count (`drop_duplicates` in `engine.rs`).
4. `--output` is resolved against that list, by identity or connector. Each target gets the call in its own backend; a failure on one does not stop the others.
5. A backend that can read the value back does so after every write and reports a mismatch.

## Backends

| Backend | Reaches | How | Cost |
|---|---|---|---|
| `nvkms` | Outputs an NVIDIA GPU scans out | NVIDIA's Digital Vibrance, set through `/dev/nvidia-modeset` | None: it acts in the display hardware, so games, video, fullscreen and VRR are covered |
| `gnome` | Whatever GNOME shows, such as the internal panel of a hybrid laptop | A saturation shader on the Shell's stage, controlled over D-Bus | A little GPU time, and fullscreen unredirect is off while a monitor is not neutral (ADR-0002) |

The order matters: NVKMS first, because it is free and does not touch the compositor. Outputs that neither backend handles are reported with the reason.

### The GNOME extension

`org.vividctl.Shell1` on the session bus is a small interface: `SetSaturation(connector, saturation)`, `GetSaturation(connector)` and the properties `Version`, `ShellVersion` and `UnredirectDisabled` (see `extensions/gnome/README.md`). It is addressed by Mutter's connector name because Shell 46 cannot list monitors; the Rust side maps identity to connector. `Version` is bumped on a breaking change, and `vividctl` refuses an extension whose version it does not know and says to reinstall. The extension is embedded in the binary, so `vividctl extension install` always installs the matching one.

## NVKMS and driver updates

- The kernel checks `versionString` and each ioctl's `paramSize` (`nvkms.c`, `ALLOC_DEVICE` and the dispatch table). A wrong size is rejected without side effects, so layout probing is safe.
- The version comes from `/sys/module/nvidia/version` at runtime, never compiled in.
- Layouts are tables of block sizes and field offsets, not copies of NVIDIA's structs, grouped into ABI generations by driver version range (ADR-0001, ADR-0004). The table is generated from the headers and never edited by hand. There are 10 generations across drivers 535 to 615. No bindgen.
- Replies are validated before any `SET`, and every write is read back.
- For a driver newer than the table, every known layout is tried, nearest first, and the first whose reply validates is used; if none does, `vividctl` refuses.
- **Planned (#31):** a scheduled CI job that compiles `tests/abi` against new open-gpu-kernel-modules tags and opens an issue on drift. Today the probe is run by hand.

## Persistence

**Planned for 0.1.0:** configuration in `~/.config/vividctl/config.toml`, monitors identified by EDID and not by position, applied at login by an XDG autostart entry running `vividctl apply`. There is no resident process. Re-applying on hotplug or resume is decided by SP-03 and SP-09.

**Today:** nothing is saved. The user guide shows a small script and an autostart entry as a stopgap.

## Testing layers

| Layer | What | Where it runs |
|---|---|---|
| Unit tests | Everything above the transport, with fakes (`FakeShell`, `FakeSettings`, `ScriptedDevice`) and an NVKMS emulator that serves the real layout table | `cargo test`, every PR |
| Binary tests | The real `vividctl` against a stub `gsettings` and a temporary data directory | `cargo test`, every PR |
| Extension harness | A headless GNOME Shell with two virtual monitors, the real `vividctl`, pixel checks per monitor | Docker; CI on Ubuntu 24.04 (Shell 46) and 26.04 (Shell 50) |
| ABI probe | `sizeof`/`offsetof` of NVIDIA's headers against the table | By hand today (#31) |
| Hardware tests | A real NVIDIA GPU and monitor | Local, `#[ignore]`, `VIVID_HW_TESTS=1` |

The emulator uses the same layout table as the backend, so it proves the logic, not the table; the table is checked by the ABI probe and by running on real drivers (595 and 580 so far).

## Known risks

- NVKMS is an undocumented NVIDIA interface.
- The GNOME Shell extension needs maintenance per Shell release: in Shell 46 to 50 two public APIs changed. Fullscreen unredirect has to be off while the effect is on, which costs some performance.
- Hybrid-laptop support depends on testers with real hardware: one laptop so far.
- The vibrance to saturation curve on the GNOME backend is a guess until SP-08.
- NVKMS-derived layouts carry the MIT notice in `NOTICE`.
