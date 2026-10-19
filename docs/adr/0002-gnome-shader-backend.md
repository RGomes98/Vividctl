# ADR-0002: GNOME Shell shader backend for outputs the NVIDIA GPU does not drive

- Status: accepted
- Date: 2026-10-19
- Spike: #5 (notes and data in `docs/spikes/sp-04-gnome-shader/README.md`)

## Context

ADR-0001 covers outputs scanned out by an NVIDIA GPU. On a hybrid laptop the internal panel is driven by the iGPU, so NVKMS cannot reach it. No vendor-agnostic path exists on Wayland: changing the KMS color matrix needs DRM master (the compositor), and Mutter has no saturation control. The remaining option is to do it inside the compositor, per compositor. GNOME comes first.

The spike built a GNOME Shell extension that attaches a saturation shader (`Clutter.ShaderEffect`) to `Main.uiGroup`, with per-monitor rectangles and a D-Bus control interface.

Evidence, on GNOME Shell 46 / Wayland / NVIDIA 595 desktop:

- The effect covers the whole stage: desktop, windows, a fullscreen game (L4D2 through Xwayland) and fullscreen video (VLC), as long as fullscreen unredirect is disabled while the effect is active.
- Screenshots and screen recordings include the effect.
- A half-screen debug region changed only that half, so the per-monitor rectangle logic works.
- Cost on a 1080p GTX 1660 Ti with a fullscreen game: about 1 percentage point of GPU for gnome-shell, no measurable CPU change, whole-GPU and power differences within the game's own variation.
- Two APIs the extension uses change in GNOME 48 (shader type enum, unredirect control). Both are handled by a compatibility layer that was written from the mutter sources and has not been run on 48 or later.

## Decision

Go, as a secondary backend with conditions.

1. **Scope.** `vivid-gnome` serves outputs that NVKMS cannot drive. NVKMS stays the primary backend, because it costs nothing and does not touch the compositor (ADR-0001). The engine chooses per output.
2. **Shape.** A GNOME Shell extension plus a D-Bus interface. The Rust side (`vivid-gnome`) is a D-Bus client. No code runs in the shell beyond the extension.
3. **Minimal extension, version-checked.** The extension stays as small as the PoC: effect, per-monitor rectangles, unredirect control, D-Bus. Anything that can live in Rust does.
4. **Compatibility layer from day one.** Version switch for the shader type enum (GNOME 48), feature detection for the unredirect API. Each supported Shell version is listed explicitly in `metadata.json`.
5. **CI on real shells.** The extension is loaded and exercised on every supported Shell version (24.04 GNOME 46 and 26.04 GNOME 50) in CI (#25). A new Shell version is not supported until that passes.
6. **Distribution.** Shipped in the `.deb` (system extension directory) and optionally on extensions.gnome.org. Not enabled behind the user's back: `vividctl doctor` says when it is needed and how to enable it.
7. **Unredirect only when needed.** Fullscreen unredirect is disabled only while some monitor is not neutral, and restored at neutral.

Per-monitor identity is not decided here. Mutter and NVKMS name the same connector differently (`HDMI-1` and `HDMI-0`), so output identity depends on SP-05 (#6).

## Consequences

- Hybrid laptop panels become controllable without disabling the iGPU, which was the goal of the laptop strategy.
- The backend is a maintenance commitment. In four GNOME versions (46 to 50) two public APIs broke; expect that rate. CI per Shell version is what keeps it honest.
- The effect is applied to the composited image, so it is global to the monitor: notifications, screenshots and recordings carry it. Excluding captures would need an extra hook and was not explored.
- Disabling fullscreen unredirect while the effect is active forces composition of fullscreen game frames. The measured cost is small on one setup, but latency and VRR behaviour are unknown and must be tested on real hardware before claiming parity with the NVKMS path.
- Tests that need screenshots are manual, because `org.gnome.Shell.Screenshot` is denied to external callers.
- On Ubuntu 24.04 with NVIDIA the default login is Xorg; users need to pick the Wayland session. This is a documentation item, not a backend issue.

## Not covered by this spike

Shell 47 to 50 runtime, two or more real monitors, fractional scaling and mixed refresh rates, an actual hybrid laptop (#8), latency, VRR, and excluding captures from the effect. These stay open and are tracked by #25, #6 and #8.

## Alternatives considered

- **No compositor backend (documentation only).** This is nvibrant's position (disable the iGPU in the BIOS). It does not serve the stated goal of working on laptops.
- **KMS color matrix from a client.** Needs DRM master. Not available to anything but the compositor.
- **A Vulkan or OpenGL layer for games.** Does not cover the desktop and video, only apps that load the layer. Kept as a later option for games on hybrid laptops (#36).
- **A different compositor first (KWin, Hyprland).** Ubuntu's default is GNOME, and the project targets Ubuntu LTS first. Other compositors are phase 6 (#35).
- **A Wayland protocol.** The right long-term answer but depends on upstream (#37). It does not exist today.
