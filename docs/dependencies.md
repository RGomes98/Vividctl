# Dependency register

Every external crate in `[workspace.dependencies]` has a row here before it merges. Defaults:
`default-features = false`, enable only what is used. Numbers come from `cargo build --timings`
and `cargo tree -e normal --prefix none | sort -u | wc -l` on a clean build.

| Crate | Used by | Why | Why not `std` | Clean-build cost | Transitive deps | ADR |
|---|---|---|---|---|---|---|
| `rustix` 1.1 (`default-features = false`, `std`, `fs`) | `vivid-nvkms` | `open` and the `ioctl` machinery without linking libc | Raw `ioctl` needs `unsafe` libc calls and manual opcode encoding | `vivid-nvkms` clean debug build including `vivid-core` and `rustix`: 1.4 s | 2 (`bitflags`, `linux-raw-sys`) | ADR-0001 |
| `clap` 4.6 (`default-features = false`, `std`, `derive`, `help`, `usage`, `error-context`, `suggestions`) | `vivid-cli` | Argument parsing with per-subcommand help, readable errors, typo suggestions | Hand-written help, errors, completions and man pages for about ten subcommands | Measured in the real CLI: `vividctl` is 676 KiB with 20 crates. Spike: +3.7 s clean release, +0.9 s clean dev, +268 KiB over `serde` + `toml`; 766 KiB total in the measured CLI | 23 in the measured CLI (+7 over `serde` + `toml`) | ADR-0006 |
| `serde` 1 (`std`, `derive`) and `toml` 1 (`std`, `parse`, `serde`) | `vivid-config` (planned) | The config file format | A TOML parser and a deserializer | +4.8 s clean release, +190 KiB over a bare binary | 15 in the measured CLI | ADR-0006 (measured) |
| `gtk4` 0.11 (`v4_14`) and `libadwaita` 0.9 (`v1_5`) | `vivid-gui` (planned) | The native GUI on GNOME | Nothing comparable | 54 s clean release, 42 s clean dev; 333 KiB binary linked to the system GTK; needs `libgtk-4-dev` and `libadwaita-1-dev` | 75 | ADR-0006 |
| `dbus` 0.9 (`default-features = false`) | `vivid-gnome` | Talks to the GNOME Shell extension and to Mutter over the session bus, with typed replies | A D-Bus client is protocol code (authentication, marshalling, variants) | +7 s clean release, +320 KiB over a bare binary; needs `libdbus-1-dev` and `pkg-config` to build | 4 (`dbus`, `libdbus-sys`, `libc`, and `pkg-config` at build time only) | ADR-0007 |

Decided against: `thiserror` in `vivid-core` (6 crates and 2.2 s for a 0.09 s alternative, ADR-0005).

Decided against: `lexopt`, `pico-args` and a hand-rolled parser for `vivid-cli` (the lean option, kept as the way back if the budget breaks, ADR-0006).

Decided against: `zbus` for `vivid-gnome` (85 crates, 1.5 MiB and 43 s of clean build, ADR-0007).

## Budgets (ADR-0006)

Checked on every PR by `scripts/check-budgets.sh` (CI job `budgets`) on the stripped release `vividctl`:

| Measure | Ceiling | Today (empty stub) | Expected |
|---|---|---|---|
| Binary size | 1.5 MiB | 1.01 MiB (1,061,504 bytes, with the GNOME backend) | 0.8 to 1.1 MiB |
| Unique crates in the `vivid-cli` tree, workspace crates included | 40 | 25 | about 30 |
| Clean release build of `vividctl` (recorded, not enforced) | 30 s on the reference machine | 2.5 s | 12 to 15 s |

A budget is a ceiling, not a target. Raising one needs a reason in the ADR.
