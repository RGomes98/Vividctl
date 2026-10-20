# Spikes

A spike is a time-boxed investigation. Its deliverable is an ADR in `docs/adr/`, plus raw notes
and measurements in its own folder here: `docs/spikes/sp-NN-<slug>/README.md`, next to any
throwaway code (`Cargo.toml`, `src/`, C probes). Spike code is standalone and is not part of the
product workspace.

Every spike states its go/no-go criteria up front, in the issue. See `ROADMAP.md` for the list.

| Spike | Question | Result | Notes |
|---|---|---|---|
| SP-01 | Can Digital Vibrance be set from pure Rust through NVKMS? | Go, [ADR-0001](../adr/0001-nvkms-pure-rust.md) | [sp-01-nvkms](sp-01-nvkms/README.md) |
| SP-02 | How does the NVKMS ABI change across driver releases? | 10 generations, [ADR-0004](../adr/0004-nvkms-abi-generations.md) | [sp-02-nvkms-abi](sp-02-nvkms-abi/README.md); data in `tests/abi/` |
| SP-04 | Can a GNOME Shell shader do it for the outputs NVIDIA does not drive? | Go with conditions, [ADR-0002](../adr/0002-gnome-shader-backend.md) | [sp-04-gnome-shader](sp-04-gnome-shader/README.md) |
| SP-05 | Can one monitor have one stable identity across backends and ports? | Yes, from the EDID, [ADR-0003](../adr/0003-output-identity.md) | [sp-05-output-identity](sp-05-output-identity/README.md) |
| SP-06 | What do the CLI parser and the GUI toolkit cost? | `clap`, budgets, [ADR-0006](../adr/0006-cli-parser-and-budgets.md) | [sp-06-build-budgets](sp-06-build-budgets/README.md) |
| SP-11 | Which D-Bus client? | The `dbus` crate, [ADR-0007](../adr/0007-dbus-client.md) | [sp-11-dbus-client](sp-11-dbus-client/README.md) |
| SP-03 | Does the value survive suspend, VT switch, hotplug, compositor restart? | Open, #4 (suspend/resume is needed for 0.1.0) | |
| SP-07 | Which laptop configurations work? | Open, #8 | |
| SP-08 | What curve turns vibrance into saturation? | Open, #9 (a provisional curve is in use) | |
| SP-09 | What triggers a re-apply? | Open, #10 | |
| SP-10 | Is a Wayland saturation protocol feasible? | Open, #11 | |

The GNOME extension that began as the SP-04 proof of concept now lives in `extensions/gnome/`; the code in `sp-04-gnome-shader/` is kept as the record of the measurements.
