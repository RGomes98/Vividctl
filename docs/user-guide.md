# User guide

How to install and use `vividctl`: set the Digital Vibrance (colour saturation) of your monitors on Linux with Wayland, from the command line.

> **State of the project:** early development, before 0.1.0. `set`, `get`, `reset` and `list` work. The value is **not saved** by `vividctl` yet: it is lost at logout or reboot, and [applying it at login](#apply-it-at-every-login) needs a small script for now. There is no GUI and no `doctor` command yet. See the [roadmap](../ROADMAP.md).

## What you need

- **Ubuntu 24.04 LTS or 26.04 LTS** (GNOME 46 or 50), in a **Wayland** session. Other GNOME versions from 46 to 50 are expected to work; see [Supported platforms](../README.md#supported-platforms).
- For monitors driven by an **NVIDIA GPU**: the NVIDIA kernel module loaded. `vividctl` reads the running driver version and checks the driver's interface before writing anything. Its table of interface layouts covers drivers 535 to 615; a driver outside that range is validated before use (see [Troubleshooting](#troubleshooting)). It has been run on 595 (desktop) and 580 (laptop).
- For everything else, such as the **internal panel of a hybrid laptop**: GNOME and the Shell extension that comes with `vividctl` (see [The GNOME extension](#the-gnome-extension)).

Ubuntu 24.04 starts an **Xorg** session by default when an NVIDIA driver is installed. Choose **Ubuntu on Wayland** with the gear icon on the login screen. X11 is not supported.

## Install

There are no releases yet. Build from source:

```
sudo apt install build-essential pkg-config libdbus-1-dev
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh     # if you have no Rust toolchain
git clone https://github.com/RGomes98/Vividctl.git && cd Vividctl
cargo build --release -p vivid-cli
install -m 755 target/release/vividctl ~/.local/bin/vividctl
```

`~/.local/bin` is on your `PATH` after the next login if it did not exist before. A binary built on Ubuntu 24.04 runs on 24.04 and newer; it needs `libdbus-1-3`, which every GNOME desktop has.

Without `libdbus-1-dev`, build only the NVIDIA backend: `cargo build --release -p vivid-cli --no-default-features`. The `vividctl extension` commands and the internal-panel support are not in that build.

## First run

```
vividctl list
```

For example, on a hybrid laptop with a monitor on the NVIDIA port:

```
nvkms: NVIDIA driver 595.99.02, NVKMS ABI generation 8 (known)
gnome: GNOME Shell 50.1 extension, interface 1

BACKEND  CONNECTOR  STATE      VIBRANCE  MONITOR
gnome    eDP-1      connected  0         LGD:0x075c:0x00000000
nvkms    HDMI-0     connected  0         PHL:PHL 242V8:0x00008fac
```

The two lines at the top say which backends work, and why one does not. The table lists every output, which backend controls it, its current value and the identity of the monitor.

## Change the vibrance

```
vividctl set 300 --output HDMI-0       # more saturated
vividctl set -300 --output HDMI-0      # less saturated
vividctl get --output HDMI-0
vividctl reset                         # every output back to neutral
```

- The value goes from **-1024** (washed out) to **1023** (most saturated). **0 is neutral.**
- `--output` takes the monitor identity from `list` (`PHL:PHL 242V8:0x00008fac`, which follows the monitor from port to port; add `#2` to tell two identical monitors apart) or a connector name (`HDMI-0`, `eDP-1`). Without it, every connected output is changed.
- If one output fails the others are still changed, and the exit status is 1. Exit status 2 means the command line was wrong. Results go to stdout and diagnostics to stderr, and `list --json` prints one line of JSON for scripts.

A good start is **200 to 300**. Large values saturate quickly.

### Which backend handles which monitor

Each monitor is handled once, by the first backend that sees it:

| Output | Backend | How it works |
|---|---|---|
| Driven by an NVIDIA GPU (a desktop monitor, an external monitor on the NVIDIA port of a laptop) | `nvkms` | NVIDIA's own Digital Vibrance, applied by the display hardware. Works with games, video, fullscreen and VRR, and costs nothing. |
| Anything else GNOME shows, such as the **internal panel of a hybrid laptop** (driven by the Intel or AMD GPU) | `gnome` | A saturation effect inside GNOME Shell, through the extension. |

On the `gnome` backend the value is converted to a saturation factor with a **provisional curve** (0 is 1.0, 1023 is 2.0, linear on each side). It is not calibrated against NVIDIA's, so the same number does not look the same on both kinds of monitor. Adjust each by eye. The calibration is on the roadmap.

## The GNOME extension

The extension is inside the `vividctl` binary. You only need it for outputs the NVIDIA GPU does not drive.

```
vividctl extension install      # copy it to ~/.local/share/gnome-shell/extensions and enable it
vividctl extension status       # installed? current? enabled? running?
vividctl extension remove
```

After the **first** `install`, **log out and back in once**: GNOME on Wayland only loads new extensions at login. The extension then starts by itself. `install` leaves your other extensions alone, and running it again changes nothing. After you update `vividctl`, run `install` again and log in again to load the matching extension.

While a monitor is not neutral on the `gnome` backend:

- Fullscreen windows go through the compositor so the effect reaches games and video. Measured on a GTX 1660 Ti with a fullscreen game, that cost about 1 percentage point of GPU. Latency and VRR were not measured.
- Screenshots and screen recordings taken through GNOME include the effect.

When every monitor is back to neutral, the effect is removed and the cost with it. The extension holds the values in memory only: a logout, a shell restart or `extension remove` returns every monitor to neutral. Locking the screen keeps them, and the lock screen shows the effect too.

## Apply it at every login

`vividctl` does not save your choice yet; a proper `apply` and a saved profile are coming. Until then, a small script and an autostart entry do the job. This one sets 425 on `HDMI-0`, the value of the pictures in the [README](../README.md); change the values, or add one `set` line per monitor:

```
mkdir -p ~/.local/bin ~/.config/autostart

cat > ~/.local/bin/vividctl-apply.sh <<'EOF'
#!/bin/sh
# Retries because the session may still be starting (on GNOME, the extension may still be loading).
for _ in $(seq 1 60); do
    "$HOME/.local/bin/vividctl" set 425 --output HDMI-0 && exit 0
    sleep 1
done
exit 1
EOF
chmod +x ~/.local/bin/vividctl-apply.sh

cat > ~/.config/autostart/vividctl-apply.desktop <<EOF
[Desktop Entry]
Type=Application
Name=vividctl
Comment=Apply the Digital Vibrance
Exec=$HOME/.local/bin/vividctl-apply.sh
X-GNOME-Autostart-enabled=true
EOF
```

It appears in GNOME's startup applications. To remove it, delete both files and run `vividctl reset`.

Whether the value survives suspend and resume has not been tested (SP-03). If it does not, run the script again.

## Troubleshooting

`vividctl list` prints the reason a backend is unavailable. The common ones:

| Message | What to do |
|---|---|
| `nvkms: unavailable: ... the NVIDIA kernel module is not loaded` | Run `lsmod \| grep nvidia` and `cat /sys/module/nvidia/version`. After a kernel update the module for the new kernel may be missing: install the package for the running kernel (its name looks like `linux-modules-nvidia-595-open-$(uname -r)`) and reboot. |
| `nvkms: unavailable: ... do not agree on the driver version` | The driver was updated and the old module is still loaded. Reboot. |
| `nvkms: unavailable: no permission to open /dev/nvidia-modeset` | Your user cannot open the device. Check its permissions. |
| `gnome: unavailable: ... extension is not running` | Run `vividctl extension install` (or `status` to see which step is missing), then log out and back in. Outside a GNOME session this is expected: only NVIDIA outputs can be controlled. |
| `no connected output can be controlled` | No monitor is on an NVIDIA port and the GNOME backend is unavailable. The internal panel of a hybrid laptop needs the extension. |
| `status` says `user extensions: turned off` | Run `gsettings set org.gnome.shell disable-user-extensions false`. |

For a driver newer than the layouts `vividctl` knows, it tries every known layout and checks the reply before writing, and refuses when none fits. Report the driver version if that happens. `VIVID_NVIDIA_VERSION` overrides the version for diagnostics; do not set it otherwise.

When you report a problem, include the output of `vividctl list`, `vividctl extension status`, `uname -r`, `cat /sys/module/nvidia/version`, `gnome-shell --version` and `echo $XDG_SESSION_TYPE`.

## Uninstall

```
vividctl reset
vividctl extension remove
rm -f ~/.local/bin/vividctl ~/.local/bin/vividctl-apply.sh ~/.config/autostart/vividctl-apply.desktop
```
