# ADR-0007: The D-Bus client is the `dbus` crate

- Status: accepted
- Date: 2026-10-19
- Spike: SP-11, tracked under #23 (notes and numbers in `docs/spikes/sp-11-dbus-client/README.md`)

## Context

`vivid-gnome` calls the GNOME Shell extension, and reads the monitors from Mutter, over the session bus. The same read, in three candidates, on the reference machine:

| Candidate | Clean release build | Binary | Crates |
|---|---|---|---|
| `gdbus` run as a subprocess | 3.0 s | 337 KiB | 1 |
| `dbus` 0.9 (system `libdbus-1`) | 10.1 s | 614 KiB | 4 |
| `zbus` 5.x | 43.2 s | 1546 KiB | 85 |

`zbus` alone is above the 1.5 MiB budget of ADR-0006 and has twice its crate budget. `gdbus` is the lightest but spawns a process per call and returns text that has to be scanned by hand, with no typed decoding and no persistent connection. The `dbus` crate decodes the real `GetCurrentState` reply into Rust types, keeps one blocking connection, and puts `vividctl` at about 1.0 MiB and 24 crates.

## Decision

1. **`vivid-gnome` uses the `dbus` crate 0.9, blocking API.** It is the only crate that uses it.
2. **It binds the system `libdbus-1`.** Building needs `pkg-config` and `libdbus-1-dev`; running needs `libdbus-1-3`, which every GNOME desktop has.
3. **The GNOME backend is behind a cargo feature** (`gnome`, on by default in `vivid-engine` and `vivid-cli`). A system without `libdbus` can still build and run the NVKMS backend alone.
4. **The extension's interface uses only simple D-Bus types** (strings, doubles, unsigned and signed integers, booleans, and arrays and structs of those). That keeps the client small and a mismatch a clear error.
5. **`zbus` is not used.** `gdbus` as a subprocess is the fallback if linking `libdbus` ever becomes a problem.

## Consequences

- `vividctl` grows by about 320 KiB and 4 crates, to about 1.0 MiB and 24, inside the budgets. Measured with the GNOME backend in: 1.01 MiB and 25 crates.
- CI and contributors need `pkg-config` and `libdbus-1-dev` now that `vivid-gnome` depends on `dbus` (CI installs them; `CONTRIBUTING.md` says how to get them).
- The binary now links a system library dynamically. The `.deb` depends on `libdbus-1-3`.
- A decoding mismatch against a different Mutter version is an error with the offending type, not a silent misparse.

## Not covered

`zbus` 6 (async only, not measured), a hand-written client, and GDBus through GTK for the GUI.

## Alternatives considered

- **`zbus`**: pure Rust and complete, at 85 crates, 1.5 MiB and 43 s. Too heavy for a tool that sends one number to a shell extension.
- **`gdbus` as a subprocess**: the lightest, but a process per call, no persistent connection and hand-scanned text. Kept as the fallback.
- **A hand-written D-Bus client**: no dependency, but authentication, marshalling, alignment and variants are protocol code that this project would own and have to keep correct.
