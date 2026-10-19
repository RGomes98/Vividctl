# GNOME Shell extension

Applies a saturation boost to a monitor from inside the compositor. This is how the internal panel of a hybrid laptop is controlled: the panel is scanned out by the iGPU, which NVKMS cannot reach.

It is a small JavaScript module, because GNOME Shell extensions can only be written in JavaScript. All logic that can live outside the compositor (monitor identity, the mapping from vibrance to saturation, persistence) lives in the Rust crates; the extension only draws.

Supported: GNOME Shell 46 to 50 (tested in CI on 46 and 50, and locally on 48 and 49).

## Install

```
extensions/gnome/install.sh
```

On Wayland, log out and back in once so the shell discovers the extension, then run `gnome-extensions enable vividctl@rgomes98.github.io`. The script enables it right away only when the running shell already knows it, and says so when it does not.

## How it works

- A `Clutter.ShaderEffect` on `Main.uiGroup` adjusts saturation inside one rectangle per monitor with a non-neutral value. With every monitor neutral the effect is removed and costs nothing.
- Fullscreen windows are normally scanned out without passing through the compositor, so the effect would not reach games or fullscreen video. While any monitor is non-neutral the extension disables unredirect (a real performance cost, which is why a neutral monitor removes it again).
- Screenshots and screen recordings taken through the shell include the effect.

## D-Bus interface

Session bus, name `org.vividctl.Shell`, object `/org/vividctl/Shell`, interface `org.vividctl.Shell1`.

| Member | Signature | Meaning |
|---|---|---|
| `SetSaturation(connector, saturation)` | `sd` | Saturation for the monitor on that connector, `0.0` to `4.0`, `1.0` is neutral |
| `GetSaturation(connector)` | `s` → `d` | Current value, `1.0` for a monitor never set |
| `Version` (property) | `u` | Interface version, `1` today; bumped on a breaking change |
| `ShellVersion` (property) | `s` | Running GNOME Shell version |
| `UnredirectDisabled` (property) | `b` | Whether fullscreen unredirect is currently off |

Monitors are addressed by the connector name Mutter uses (`Meta-0`, `eDP-1`, `HDMI-1`), the same name as in `org.gnome.Mutter.DisplayConfig`. Identity across ports is resolved on the Rust side from the EDID (ADR-0003), because the Shell 46 API cannot list monitors.

Errors are D-Bus `InvalidArgs`: an unknown connector, or a saturation outside `0.0..=4.0`. A refused call changes nothing. Values live in the shell process only: a shell restart, a disable, or a logout returns every monitor to neutral, and the caller re-applies them.

```
gdbus call --session --dest org.vividctl.Shell --object-path /org/vividctl/Shell \
    --method org.vividctl.Shell1.SetSaturation eDP-1 1.4
```

## Tests

```
extensions/gnome/test/run.sh [24.04|25.04|25.10|26.04]
```

Needs Docker. Builds an Ubuntu image with GNOME Shell, starts a headless shell with two virtual monitors and checks the interface, the error cases, the pixels on each monitor (only the boosted one changes), unredirect handling, and disable/enable. A headless Shell older than 48 repaints late; if a pixel check cannot be told apart from that, it is reported as `XFAIL` instead of a failure.
