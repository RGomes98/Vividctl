# Changelog

All notable changes to this project are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/) and the project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

Nothing is released yet; 0.1.0 is described in the [roadmap](ROADMAP.md#release-plan-v010).

### Added

- **CLI:** `vividctl list [--json]`, `get`, `set` and `reset`. A monitor is named with `--output` by its identity (`VENDOR:PRODUCT:SERIAL`, which follows the monitor from port to port) or by connector. A failure on one output does not stop the others. See the [user guide](docs/user-guide.md).
- **NVIDIA backend:** Digital Vibrance through NVKMS from pure Rust, with no per-driver build. It reads the running driver version, picks the interface layout from a table of 10 ABI generations covering drivers 535 to 615 (generated from NVIDIA's headers by `tests/abi`), validates every reply before writing and reads every write back. For a driver the table does not know it tries every layout and refuses if none validates.
- **GNOME backend:** `vividctl` also drives the outputs a GNOME Shell extension controls, such as the internal panel of a hybrid laptop. A monitor an NVIDIA GPU drives is listed once and handled by NVKMS. The vibrance is converted to a saturation with a provisional curve until the calibration (SP-08).
- **GNOME Shell extension:** a per-monitor saturation shader for Shell 46 to 50, with a small D-Bus interface (`org.vividctl.Shell1`). Fullscreen unredirect is disabled only while a monitor is not neutral. Tested headless in CI on Shell 46 and 50.
- **GNOME:** `vividctl extension install`, `status` and `remove`. The extension is embedded in the binary, so it always matches it; installing adds it to the enabled extensions without disturbing the ones already there, and one log in is enough for GNOME Shell on Wayland to load it.
- **Output identity:** monitors are identified from their EDID in the same form Mutter uses, so the same monitor has the same identity on every backend and port.
- **Build requirements:** building needs `libdbus-1-dev` and `pkg-config`, or `--no-default-features` on `vivid-cli` for the NVKMS backend only.
- **Budgets:** CI fails if the `vividctl` binary passes 1.5 MiB or its dependency tree passes 40 crates (it is 1.09 MiB and 25 crates).
- **Docs:** user guide, architecture, seven decision records (`docs/adr/`) and the measurements behind them (`docs/spikes/`).
- **Repo:** Cargo workspace, CI (format, clippy, tests, `cargo-deny`, `cargo-machete`, MSRV, budgets, extension tests), lint and dependency policy, contribution docs, issue and pull request templates.

### Fixed

- **GNOME:** locking the screen no longer returns the outputs the extension controls to neutral (#58). The extension now stays enabled on the lock screen, which shows the effect too.
- **Install:** the quick start in the README left `vividctl: command not found` on a fresh machine (#60). It copied the binary into `~/.local/bin`, which does not exist on a new Ubuntu install, so the copy failed, and it assumed `git` and Rust were present. It now installs with `cargo install` into `~/.cargo/bin`, adds the missing packages and the Rust installer, and is checked on clean Ubuntu 24.04 and 26.04 in CI.
- **GNOME:** with the effect on and more than one monitor, moving windows left trails and the screenshot UI made the monitors flicker dark (#59). The effect is now applied by one overlay per boosted monitor, a clone of the shell UI clipped to that monitor, instead of one effect over every monitor; a neutral monitor is not touched at all.
