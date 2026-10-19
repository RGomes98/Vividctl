# SP-11: D-Bus client for `vivid-gnome`

Part of #23. Status: done, decision in `docs/adr/0007-dbus-client.md`.

## Question

`vivid-gnome` has to talk to the GNOME Shell extension and to Mutter over the session bus. Which way of speaking D-Bus fits the project's budgets (1.5 MiB and 40 crates for `vividctl`, now 676 KiB and 20 crates) and its goal of few dependencies?

## Method

The same task in each candidate: connect to the session bus, call `org.gnome.Mutter.DisplayConfig.GetCurrentState` and print the connector and identity of each monitor. The result is the same everywhere: `HDMI-1  PHL:PHL 242V8:0x00008fac`. Measured with `../sp-06-build-budgets/measure.sh` (median of 3, same release profile as the product, i5-9400F). The `libdbus` candidate was built in the Ubuntu 24.04 container of SP-06 (`Dockerfile` here adds `libdbus-1-dev`), and the binary was then run on the desktop.

## Results

| Candidate | Clean release | Clean dev | Binary | Crates | Lines |
|---|---|---|---|---|---|
| `gdbus` run as a subprocess, text output scanned by hand | 3.0 s | 0.16 s | 337 KiB | 1 | 49 |
| `dbus` 0.9 (binds the system `libdbus-1`) | 10.1 s | 2.76 s | 614 KiB | 4 | 21 |
| `zbus` 5.x (`async-io`, `blocking-api`) | 43.2 s | 15.6 s | 1546 KiB | 85 | 26 |

Over the bare 294 KiB binary of SP-06: `gdbus` +43 KiB, `dbus` +320 KiB, `zbus` +1252 KiB. On top of today's `vividctl` (676 KiB, 20 crates), `dbus` gives about 1.0 MiB and 24 crates, inside the budgets. `zbus` alone is above the size budget and has twice the crate budget.

## What each one means in practice

- **`gdbus` subprocess.** Zero dependencies, but every call spawns a process (a slider that sends 30 values a second would spawn 30 processes a second), there is no persistent connection, errors arrive as text on stderr, and the reply is the text form of a GVariant that has to be scanned by hand. The scanner in the PoC only looks for the pattern `(('`; any change in the reply's shape is invisible to it.
- **`dbus` crate.** Typed replies: the PoC decodes the whole `GetCurrentState` structure into Rust types, and a mismatch is an error, not a silent misparse. A persistent blocking connection. It binds `libdbus-1`, the reference implementation, which every GNOME desktop already has installed (`libdbus-1-3`); building needs `libdbus-1-dev` and `pkg-config`. The binary links to `libdbus-1.so.3` dynamically.
- **`zbus`.** A complete, pure-Rust, async-first implementation, and the heaviest by far: 85 crates, `zvariant` and the proc macros, 43 s to build.

## Not covered

- `zbus` 6.0: its `blocking-api` feature is gone and the API is async-only, so it was not measured. The 5.x numbers show the order of magnitude.
- A hand-written D-Bus client (authentication, message marshalling, alignment, variants). Not built: it is protocol code to write and maintain, which is a much larger step than "a few lines instead of a crate".
- GTK's own `gio` D-Bus (GDBus) for the GUI. The GUI links GLib anyway, but the engine is shared with the CLI, which must not.
