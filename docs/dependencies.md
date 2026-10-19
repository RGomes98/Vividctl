# Dependency register

Every external crate in `[workspace.dependencies]` has a row here before it merges. Defaults:
`default-features = false`, enable only what is used. Numbers come from `cargo build --timings`
and `cargo tree -e normal --prefix none | sort -u | wc -l` on a clean build.

| Crate | Used by | Why | Why not `std` | Clean-build cost | Transitive deps | ADR |
|---|---|---|---|---|---|---|
| `rustix` 1.1 (`default-features = false`, `std`, `fs`) | `vivid-nvkms` (planned) | `open` and the `ioctl` machinery without linking libc | Raw `ioctl` needs `unsafe` libc calls and manual opcode encoding | PoC clean debug build, 3 crates: about 1.6 s | 2 (`bitflags`, `linux-raw-sys`) | ADR-0001 |

Candidates to evaluate in Phase 0: `rustix`, `thiserror`, `serde` + `toml`, `clap` (minimal
features) or `lexopt`, `zbus` (vivid-gnome only), `gtk4` + `libadwaita` (vivid-gui only).
