# vividctl

Digital Vibrance (color saturation) control for Linux on Wayland. Fast, small, and built to survive driver updates.

> **Status:** planning / pre-alpha. Nothing is released yet. See [ROADMAP.md](ROADMAP.md).

## Goals

- Control NVIDIA Digital Vibrance on Wayland, where `nvidia-settings` cannot.
- Work on hybrid laptops, where the internal panel is driven by the iGPU (GNOME first).
- Apply everywhere: games, fullscreen apps, video, desktop.
- No resident process. Apply at login; open the GUI only when you want it.
- Persist across restarts. Scriptable through a CLI.
- Break less when a new NVIDIA driver ships: version probing, ABI conformance CI, and a `doctor` command that explains failures.

## Usage

Works today for outputs an NVIDIA GPU drives (through the display hardware) and, on GNOME with the Shell extension installed, for every other output, including the internal panel of a hybrid laptop. Each output goes to the first backend that sees it, NVKMS before GNOME. Install the extension with `vividctl extension install` and log in again once (see [extensions/gnome/README.md](extensions/gnome/README.md)); on GNOME the value is converted to a saturation with a provisional curve until the calibration (SP-08).

```
vividctl list [--json]                       # outputs, their state, vibrance and monitor identity
vividctl get [--output MONITOR]              # current vibrance
vividctl set 512 [--output MONITOR]          # Digital Vibrance, -1024 to 1023, 0 is neutral
vividctl set -300 --output HDMI-0
vividctl reset [--output MONITOR]            # back to neutral
vividctl extension install                   # GNOME: install and enable the Shell extension (then log in again)
vividctl extension status                    # installed? enabled? running?
vividctl extension remove
```

`MONITOR` is the identity `list` shows, such as `PHL:PHL 242V8:0x00008fac` (it follows the monitor from port to port; add `#2` to tell identical monitors apart), or a connector name such as `HDMI-0`. Without `--output`, every connected output is used. A failure on one output does not stop the others and the exit status is 1. Results go to stdout, diagnostics to stderr.

Planned:

```
vividctl apply                   # apply the saved profile (used at login)
vividctl preset save gaming
vividctl doctor                  # diagnose driver, kernel module, hybrid setups
vivid                            # GUI (GTK4 + libadwaita), started on demand
```

## Supported platforms (target)

|                    | Ubuntu 24.04 LTS | Ubuntu 26.04 LTS |
| ------------------ | ---------------- | ---------------- |
| GNOME              | 46               | 50               |
| NVIDIA open driver | 595              | current          |
| Session            | Wayland          | Wayland          |

X11 is out of scope.

## Repository layout

See [docs/architecture.md](docs/architecture.md). Contributing rules are in [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
