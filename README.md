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

## Planned interface

```
vividctl list                    # outputs and which backend controls each
vividctl set --output DP-1 512   # Digital Vibrance, -1024..=1023
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
