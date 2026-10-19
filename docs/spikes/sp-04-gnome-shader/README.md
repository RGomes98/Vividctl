# SP-04: GNOME Shell saturation shader

Issue: #5. Status: done, decision is go with conditions. The ADR is `docs/adr/0002-gnome-shader-backend.md`.

## Question

Can a GNOME Shell extension apply a saturation shader to everything on screen, as a backend for outputs the NVIDIA display engine does not drive (hybrid laptop panels)? In particular: fullscreen games and video, per-monitor control, cost, screenshots and recordings, and differences between Shell 46 and 50.

## Environment

- Ubuntu 24.04.4 LTS, GNOME Shell 46.0, mutter 46.2, Wayland session ("Ubuntu on Wayland")
- NVIDIA open driver 595.99.02, GeForce GTX 1660 Ti, one monitor (Philips PHL 242V8, 1920x1080, HDMI, 75 Hz, no VRR)
- Test game: Left 4 Dead 2 (native Linux, `hl2_linux`, runs through Xwayland), fullscreen
- Not available: a second monitor, a hybrid laptop, Shell 47 to 50

Ubuntu 24.04 starts Xorg by default on NVIDIA >= 470 (`61-gdm.rules`), even with `nvidia_drm modeset=1`. A Wayland session has to be picked at the login screen. This matters for documentation.

## The PoC

`extension/` is a GNOME Shell extension (`vivid-spike@vividctl.dev`, ES modules):

- A `Clutter.ShaderEffect` with a generated GLSL fragment shader is attached to `Main.uiGroup`, so it covers the panel, windows, overlays and fullscreen windows.
- Saturation is `mix(luma, rgb, s)`. It is set per monitor: the shader receives each monitor's rectangle (x, y, w, h in stage pixels) and picks the saturation by fragment position.
- While any monitor is not neutral, fullscreen unredirect is disabled so fullscreen windows go through the compositor and the shader.
- Control is over D-Bus: `org.vividctl.Spike.SetSaturation(monitor, value)`, `Describe()` and `Reload()`. Saturation 1.0 is neutral.
- `extension.js` is a fixed loader. The code lives in `impl.js` and can be reloaded without logging out (see lessons below).

`measure.sh` samples GPU and compositor load. `results/` has the raw runs.

## Results

### What the effect covers (GNOME 46, Wayland)

| Case | Result |
|---|---|
| Desktop, panel, dock, windows | Saturation changes everywhere |
| Fullscreen game (L4D2, through Xwayland) | Visibly more saturated at 1.5 |
| Fullscreen video (VLC) | Visibly more saturated at 2.0 |
| Screenshot (PrtSc) | Includes the effect. Mean HSV saturation 0.082 before and 0.154 during a 2.0 boost (1.9x) |
| Screen recording (Ctrl+Shift+Alt+R) | The maintainer confirmed by eye that the recording is saturated. A frame measured 0.117 against 0.082 neutral, lower than the screenshot, probably from video compression |
| Half-screen region (debug mode) | Only the left half changed. The per-monitor rectangle test works |

The effect is applied to the final composited image, so screenshots and recordings carry it. Nothing in the PoC excludes them.

### Per-monitor control

The shader picks saturation by rectangle. The half-screen test proves the rectangle logic and the coordinate mapping on one monitor. It does not prove behaviour with two real monitors, mixed scales or fractional scaling: the stage coordinates are logical pixels, and that was not exercised.

### Cost (single runs, 30 s each, L4D2 fullscreen, 1080p, GTX 1660 Ti)

| Phase | GPU util | Power | gnome-shell GPU (`pmon` sm) | gnome-shell CPU | Game GPU (sm) |
|---|---|---|---|---|---|
| 1. Extension off | 38.3 % | 32.2 W | 0.0 % | 6.6 % | 31.6 % |
| 2. On, neutral | 33.8 % | 30.3 W | 0.5 % | 5.0 % | 27.3 % |
| 3. On, saturation 1.5 | 38.8 % | 32.2 W | 1.3 % | 6.4 % | 33.8 % |

Reading: the compositor's own GPU share rises from 0.0 to 0.5 to 1.3 percentage points. Whole-GPU utilisation and power move by amounts comparable to the game's own run-to-run variation (the game alone went from 27 to 34 % between phases), so the shader cost is below what this setup can resolve, roughly 1 point of GPU on a 1080p panel. CPU use of gnome-shell did not change measurably.

Not measured:

- **Frame rate.** The game is old and light, and the counter was too coarse to read.
- **Input and display latency.** No tool for it here.
- **VRR.** The monitor does not support it.
- **Heavy GPU-bound games, higher resolutions and refresh rates.** A fixed full-screen pass costs the same per frame, so its share grows at higher frame rates and resolutions.
- **Whether disabling unredirect adds latency** in the fullscreen path. It forces composition of the game's frames, which is the likely cost, but was not isolated.

### Reading the 0.0 % in phase 1

With the extension off, gnome-shell used no GPU while the game ran. That is consistent with fullscreen unredirect or direct scanout, which is the path disabled when the effect is active. It is an inference from `pmon`, not a direct observation of scanout.

## API compatibility (checked in the mutter sources, run only on 46)

Two public APIs the extension depends on change in GNOME 48:

| API | 46, 47 | 48 to 50 |
|---|---|---|
| `ClutterShaderEffect:shader-type` enum | `Clutter.ShaderType` (`FRAGMENT_SHADER`) | `Cogl.ShaderType` (`FRAGMENT`) |
| Unredirect control | `Meta.disable_unredirect_for_display(display)` | `Meta.Compositor.disable_unredirect()` |

`clutter_shader_effect_set_shader_source` and `clutter_shader_effect_set_uniform_value` are unchanged in 50.1. `impl.js` has a compatibility layer (version switch for the enum, feature detection for unredirect). It was not run on 48 or later.

Verified against tags 46.2, 47.5, 48.4, 49.2 and 50.1 of mutter. Shell 50 was not run: the dev machine is Ubuntu 24.04.

## Lessons for the real extension

- **gjs ignores query strings when caching modules.** `import(uri + '?v=2')` returns the cached module. The loader copies `impl.js` to a uniquely named file in `$XDG_RUNTIME_DIR` and imports that, which makes `Reload` work.
- **gjs turns whole JS numbers into integers, and `ShaderEffect` silently ignores an integer for a float uniform.** The effect did nothing until every value was nudged to a non-integer (`v + 1e-6`).
- **`ShaderEffect` cannot take vec or array uniforms from JS.** The shader source is generated per monitor count and uses scalar uniforms.
- **A new extension is only discovered at login on Wayland.** After that, enable, disable and reload work live.
- **`org.gnome.Shell.Screenshot` is denied** to external callers, so screenshots cannot be taken from a script. Tests that need them are manual.
- **Nested shell (`gnome-shell --nested`) did not work** from a VS Code snap terminal: snap-injected `XDG_DATA_DIRS` hide the system schemas, and with that fixed the portal still failed. Dev loop here is the real session.
- **Monitor names differ.** Mutter calls the connector `HDMI-1` where NVKMS calls it `HDMI-0`. Output identity needs the EDID (SP-05, #6).

## Not done

- Shell 47 to 50 runtime tests (tracked with the CI work in #25).
- Two real monitors, fractional scaling, mixed refresh rates.
- A hybrid laptop (#8), where the game renders on the dGPU and is copied to the iGPU.
- Excluding screenshots and recordings from the effect.
- Latency and VRR.

## Cleanup on the dev machine

The extension is installed as a symlink in `~/.local/share/gnome-shell/extensions/vivid-spike@vividctl.dev` and enabled in the user's dconf settings. Remove both when the spike work is over:

```
gnome-extensions disable vivid-spike@vividctl.dev
rm ~/.local/share/gnome-shell/extensions/vivid-spike@vividctl.dev
```
