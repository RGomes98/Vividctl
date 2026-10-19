# Roadmap

Each phase is a GitHub Milestone. Items map to issues; numbers are added once the issues exist.
Priority: `P0` critical, `P1` high, `P2` medium, `P3` low, `Icebox` unscheduled. Size: `XS` to `XL`.

## Phase 0 — Foundation & Discovery

**Goal:** repo, CI and standards ready; critical decisions made with evidence.

Gate: SP-01 and SP-04 decide whether the NVKMS + GNOME strategy holds. If SP-04 fails, fall back to NVKMS-only plus a documented limitation, and revisit the laptop decision.

- [x] **Repository bootstrap:** workspace, toolchain, rustfmt, lints, deny, CI, contribution docs, issue templates — P0 · M (#1)
- [x] **SP-01 NVKMS in pure Rust:** set Digital Vibrance via `rustix` + hand-written structs on driver 595. Go, ADR-0001 — P0 · M (#2)
- [x] **SP-02 ABI map:** NVKMS struct changes across driver tags 535 to 615; readback exists in all. 10 generations, ADR-0004 — P0 · M (#3)
- [ ] **SP-03 Persistence matrix:** suspend/resume, VT switch, hotplug, mode change, compositor restart, fullscreen/VRR, HDR — P1 · M (#4)
- [x] **SP-04 GNOME Shell shader PoC:** shell 46 and 50, fullscreen games/unredirect, per-monitor, cost. Go with conditions, ADR-0002 — P0 · L (#5)
- [x] **SP-05 Stable output identity:** NVKMS dpy to DRM connector to EDID to Mutter monitor. ADR-0003 — P1 · M (#6)
- [ ] **SP-06 Build cost budgets:** GTK4/libadwaita, clap vs lexopt, binary size and clean-build time — P1 · S (#7)
- [ ] **SP-07 Laptop test matrix:** Optimus, MUX, dGPU-only, external ports; recruit testers — P1 · M (#8)
- [ ] **SP-08 DVC to saturation model:** calibrate the curve used by preview and shader — P2 · M (#9)
- [ ] **SP-09 Re-apply trigger:** XDG autostart vs udev `SYSTEMD_USER_WANTS` vs opt-in `vividctl watch` — P1 · S (#10)
- [ ] **SP-10 Wayland saturation protocol feasibility:** who, where, how — P3 · S (#11)

## Phase 1 — Core + NVIDIA backend + CLI (v0.1)

**Goal:** `vividctl set/get/reset/list/apply/doctor` working on NVIDIA desktops and laptop external ports, persistent at login.

- [x] **vivid-core:** domain types, validated `Vibrance`, `Backend` trait, errors — P0 · M (#12)
- [x] **vivid-nvkms transport:** ioctl wrapper and deterministic fake — P0 · M (#13)
- [ ] **vivid-nvkms ABI generations:** per-range layouts, `paramSize` probe, reply validation — P0 · L (#14)
- [ ] **vivid-outputs:** sysfs discovery, owning driver, EDID identity — P0 · M (#15)
- [ ] **vivid-config:** TOML model, XDG paths, atomic writes, versioned migration — P1 · M (#16)
- [ ] **vivid-engine:** apply/verify/fallback, per-output backend selection — P0 · L (#17)
- [ ] **vivid-cli:** `list`, `get`, `set`, `reset`, `apply`, `preset` — P0 · L (#18)
- [ ] **vividctl doctor:** kernel module for the running kernel, device node, version, hybrid detection, per-output verdict — P0 · M (#19)
- [ ] **Login autostart:** XDG autostart entry and `vividctl install --user` — P1 · S (#20)
- [ ] **Hardware tests and usage docs** — P1 · S (#21)

## Phase 2 — GNOME compositor backend (v0.2)

**Goal:** the internal panel of hybrid laptops is controllable; backend chosen per output.

- [ ] **GNOME Shell extension:** saturation shader, shell 46 to 50 — P0 · L (#22)
- [ ] **D-Bus interface and `vivid-gnome` client** — P0 · M (#23)
- [ ] **Per-output backend selection in the engine** — P0 · M (#24)
- [ ] **CI with headless gnome-shell** — P1 · M (#25)

## Phase 3 — GUI (v0.3)

**Goal:** GTK4/libadwaita app with outputs, slider, presets, apply/save/reset, preview and auto-revert.

- [ ] **GUI skeleton:** GApplication single instance, `.desktop`, icon — P0 · M (#26)
- [ ] **Outputs list, slider, live apply with 15 s auto-revert** — P0 · L (#27)
- [ ] **Presets CRUD and settings page** — P1 · M (#28)
- [ ] **Before/after preview on bundled images** — P1 · M (#29)
- [ ] **Split-screen live preview (GNOME backend)** — P3 · L (#30)

## Phase 4 — Robustness & future-proofing (v0.4)

**Goal:** survive new drivers without manual intervention.

- [ ] **Scheduled ABI drift check:** compare NVIDIA tags against Rust layouts, open an issue on drift — P0 · M (#31)
- [ ] **Re-apply on hotplug/resume** (per SP-03 and SP-09) — P1 · M (#32)
- [ ] **`doctor --report`:** paste-ready text, no telemetry — P2 · S (#33)

## Phase 5 — Distribution (v1.0)

**Goal:** one-command install.

- [ ] **`.deb` for 24.04 and 26.04, PPA, musl tarball, AUR, docs site** — P1 · L (#34)

## Phase 6 — Beyond

KWin and Hyprland adapters, Vulkan layer for iGPU games, Wayland saturation protocol proposal, more controls (dithering, per-channel), per-game profiles. Flatpak is Icebox (needs `--device=all`).
