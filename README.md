# vividctl

Digital Vibrance (colour saturation) control for Linux on Wayland. Fast, small, and built to survive driver updates.

> **Status:** early development, before 0.1.0. You can set, read and reset the vibrance of NVIDIA monitors and of the internal panel of a hybrid laptop (GNOME). `vividctl` does not save your choice yet, so it does not survive a logout; see [what is left for 0.1.0](ROADMAP.md#release-plan-v010). No releases have been published: build from source.

## Why

- NVIDIA's Digital Vibrance cannot be changed from `nvidia-settings` on Wayland.
- On a hybrid laptop the internal panel is driven by the Intel or AMD GPU, which NVIDIA's control cannot reach. The GNOME backend covers it.
- It should work in games, fullscreen apps and video, without a resident process, and keep working when a new NVIDIA driver ships (version probing and validation instead of a build per driver).

## Quick start

```
sudo apt install build-essential pkg-config libdbus-1-dev
git clone https://github.com/RGomes98/Vividctl.git && cd Vividctl
cargo build --release -p vivid-cli
install -m 755 target/release/vividctl ~/.local/bin/vividctl

vividctl list                          # what it found, and which backend controls each output
vividctl set 300 --output HDMI-0       # -1024 (washed out) to 1023 (saturated), 0 is neutral
vividctl reset
```

For the internal panel of a laptop (or any output the NVIDIA GPU does not drive), also run `vividctl extension install` and log in again once.

The [user guide](docs/user-guide.md) has the details: requirements, the GNOME extension, applying a value at every login, troubleshooting, uninstalling.

## Commands

```
vividctl list [--json]                       # outputs, their state, vibrance and monitor identity
vividctl get [--output MONITOR]              # current vibrance
vividctl set VALUE [--output MONITOR]        # -1024 to 1023, 0 is neutral
vividctl reset [--output MONITOR]            # back to neutral
vividctl extension install | status | remove # the GNOME Shell extension (then log in again)
```

`MONITOR` is the identity `list` shows, such as `PHL:PHL 242V8:0x00008fac` (it follows the monitor from port to port), or a connector name such as `HDMI-0`. Without `--output`, every connected output is used.

Planned:

```
vividctl apply                   # apply the saved profile (used at login)
vividctl preset save gaming
vividctl doctor                  # diagnose driver, kernel module, hybrid setups
vivid                            # GUI (GTK4 + libadwaita), started on demand
```

## Supported platforms

|               | Ubuntu 24.04 LTS | Ubuntu 26.04 LTS |
| ------------- | ---------------- | ---------------- |
| GNOME         | 46               | 50               |
| NVIDIA driver | 595 (tested)     | 580 (tested)     |
| Session       | Wayland          | Wayland          |

X11 is out of scope. The GNOME extension also runs in headless tests on Shell 48 and 49. KDE and other compositors are not supported yet: NVIDIA monitors work wherever the NVIDIA module is loaded, but the internal panel of a hybrid laptop needs the GNOME backend.

### What has been tested on real hardware

| Machine | What was exercised |
|---|---|
| Desktop: GTX 1660 Ti, driver 595.99.02, Ubuntu 24.04, GNOME 46, monitor on HDMI | NVKMS `list`, `get`, `set`, `reset` with a visual check; the Shell effect on a fullscreen game and on fullscreen video |
| Hybrid laptop: Dell Inspiron 16 Plus 7640 (internal panel on the Intel GPU), driver 580.178.04, Ubuntu 26.04, GNOME 50.1 | The GNOME backend on the internal panel: `list`, `set` and `get`, with a visual check. The NVKMS layout for this driver is recognised, but `set` and `get` through it have not run: that needs a monitor on the NVIDIA port |

Everything else is covered by automated tests, including an emulated NVKMS driver and headless GNOME Shell 46 and 50 in CI. Reports from other hardware are welcome; see [CONTRIBUTING.md](CONTRIBUTING.md#hardware-reports).

## Documentation

| For | Read |
|---|---|
| Users | [docs/user-guide.md](docs/user-guide.md), [extensions/gnome/README.md](extensions/gnome/README.md) |
| Contributors | [CONTRIBUTING.md](CONTRIBUTING.md), [docs/architecture.md](docs/architecture.md) |
| Why it is built this way | [docs/adr/](docs/adr/README.md) (decisions) and [docs/spikes/](docs/spikes/README.md) (the measurements behind them) |
| What is next | [ROADMAP.md](ROADMAP.md), [CHANGELOG.md](CHANGELOG.md) |

## License

Apache-2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).
