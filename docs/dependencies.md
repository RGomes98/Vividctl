# Dependency register

Every external crate in `[workspace.dependencies]` has a row here before it merges. Defaults:
`default-features = false`, enable only what is used. Numbers come from `cargo build --timings`
and `cargo tree -e normal --prefix none | sort -u | wc -l` on a clean build.

| Crate | Used by | Why | Why not `std` | Clean-build cost | Transitive deps | ADR |
|---|---|---|---|---|---|---|
| `rustix` 1.1 (`default-features = false`, `std`, `fs`) | `vivid-nvkms` | `open` and the `ioctl` machinery without linking libc | Raw `ioctl` needs `unsafe` libc calls and manual opcode encoding | `vivid-nvkms` clean debug build including `vivid-core` and `rustix`: 1.4 s | 2 (`bitflags`, `linux-raw-sys`) | ADR-0001 |

Decided against: `thiserror` in `vivid-core` (6 crates and 2.2 s for a 0.09 s alternative, ADR-0005).

Candidates to evaluate in Phase 0: `rustix`, `serde` + `toml`, `clap` (minimal
features) or `lexopt`, `zbus` (vivid-gnome only), `gtk4` + `libadwaita` (vivid-gui only).
