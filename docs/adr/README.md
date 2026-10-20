# Architecture decision records

An ADR records a decision that is expensive to reverse, with the evidence behind it. Write one when a change affects the architecture, adds a dependency or closes a spike. Copy [0000-template.md](0000-template.md), number it in order, and add a row here.

| ADR | Decision | From |
|---|---|---|
| [0001](0001-nvkms-pure-rust.md) | Control Digital Vibrance through NVKMS ioctls from pure Rust, with layouts as data and a readback after every write | SP-01, #2 |
| [0002](0002-gnome-shader-backend.md) | A GNOME Shell saturation shader backend for the outputs the NVIDIA GPU does not drive, with unredirect disabled only while it is active | SP-04, #5 |
| [0003](0003-output-identity.md) | Identify monitors by the EDID in the form Mutter uses, with a slot only where two connected monitors are identical | SP-05, #6 |
| [0004](0004-nvkms-abi-generations.md) | Group NVKMS layouts into ABI generations generated from NVIDIA's headers; try every one, validated, for an unknown driver | SP-02, #3 |
| [0005](0005-errors-without-thiserror.md) | Write errors by hand in the core crate instead of using `thiserror` | #12 |
| [0006](0006-cli-parser-and-budgets.md) | `clap` with derive for the CLI; CI-enforced budgets on binary size and crate count; the GUI stays out of the default build | SP-06, #7 |
| [0007](0007-dbus-client.md) | The `dbus` crate (system libdbus) as the D-Bus client, behind the `gnome` feature | SP-11, #23 |

The measurements behind the spike decisions are in [../spikes/](../spikes/README.md).
