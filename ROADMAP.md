# Roadmap

Each phase is a GitHub Milestone. Items map to issues; numbers are added once the issues exist.
Priority: `P0` critical, `P1` high, `P2` medium, `P3` low, `Icebox` unscheduled. Size: `XS` to `XL`.

**Where we are:** the NVIDIA backend, the GNOME backend (internal panels of hybrid laptops) and the `list`, `get`, `set`, `reset` and `extension` commands work. What is missing for a first release is everything that makes a choice stick: saved configuration, `apply` at login, and `doctor`. The GNOME backend (Phase 2) was built before the end of Phase 1 because the internal panel was the priority, and it ships with v0.1.0.

## Release plan v0.1.0

The goal of v0.1.0 is the goal of Phase 1: the tool sets the vibrance, remembers it, applies it at login and explains failures, on NVIDIA monitors and on the internal panel of a GNOME laptop.

Remaining work, in the order it should land:

1. **`vivid-config` (#16):** TOML file in `~/.config/vividctl/`, atomic writes, format version.
2. **Profile and `apply` (#17, #18):** the engine stores "monitor identity to vibrance"; `vividctl set 300 --save` writes it and `vividctl apply` reapplies it. Full presets (CRUD) can wait.
3. **Login autostart (#20, SP-09 #10):** an XDG autostart entry running `vividctl apply`, installed by `vividctl install --user`. The temporary script in the [user guide](docs/user-guide.md#apply-it-at-every-login) goes away.
4. **`vividctl doctor` (#19):** kernel module for the running kernel, device node, driver version, hybrid detection, a verdict per output. May pull in `vivid-outputs` (#15) for the hybrid detection.
5. **Release preparation:** version bump from `0.0.0`, a `0.1.0` section in the changelog, a workflow that builds the binary on Ubuntu 24.04 when a tag is pushed and attaches it to a GitHub release, and the status line of the README.

Checks before tagging:

- **Suspend and resume (SP-03 #4):** if NVKMS loses the value across suspend, `apply` has to run on resume too, or the limitation is documented. A five-minute manual test on the desktop decides it; do it before step 2.
- **NVKMS on the laptop driver (generation 7):** `set` and `get` have not run through it, because that needs a monitor on the NVIDIA port. Ship stating that, or test it.
- The `.deb`, PPA and GUI are not part of v0.1.0 (Phases 3 and 5).

## Phase 0 — Foundation & Discovery

**Goal:** repo, CI and standards ready; critical decisions made with evidence.

Gate: SP-01 and SP-04 decide whether the NVKMS + GNOME strategy holds. If SP-04 fails, fall back to NVKMS-only plus a documented limitation, and revisit the laptop decision.

- [x] **Repository bootstrap:** workspace, toolchain, rustfmt, lints, deny, CI, contribution docs, issue templates — P0 · M (#1)
- [x] **SP-01 NVKMS in pure Rust:** set Digital Vibrance via `rustix` + hand-written structs on driver 595. Go, ADR-0001 — P0 · M (#2)
- [x] **SP-02 ABI map:** NVKMS struct changes across driver tags 535 to 615; readback exists in all. 10 generations, ADR-0004 — P0 · M (#3)
- [ ] **SP-03 Persistence matrix:** suspend/resume, VT switch, hotplug, mode change, compositor restart, fullscreen/VRR, HDR. Suspend/resume is needed before v0.1.0 — P1 · M (#4)
- [x] **SP-04 GNOME Shell shader PoC:** shell 46 and 50, fullscreen games/unredirect, per-monitor, cost. Go with conditions, ADR-0002 — P0 · L (#5)
- [x] **SP-05 Stable output identity:** NVKMS dpy to DRM connector to EDID to Mutter monitor. ADR-0003 — P1 · M (#6)
- [x] **SP-06 Build cost budgets:** GTK4/libadwaita, clap vs lexopt, binary size and clean-build time. `clap` with derive, budgets enforced in CI, ADR-0006 — P1 · S (#7)
- [ ] **SP-07 Laptop test matrix:** Optimus, MUX, dGPU-only, external ports; recruit testers. One hybrid laptop (Dell Inspiron 16 Plus 7640, Intel iGPU panel) works with the GNOME backend — P1 · M (#8)
- [ ] **SP-08 DVC to saturation model:** calibrate the curve used by preview and shader. Until then `vivid-color` uses a provisional linear curve — P2 · M (#9)
- [x] **SP-11 D-Bus client:** `gdbus` vs the `dbus` crate vs `zbus`. The `dbus` crate, ADR-0007 — P1 · S (tracked under #23)
- [ ] **SP-09 Re-apply trigger:** XDG autostart vs udev `SYSTEMD_USER_WANTS` vs opt-in `vividctl watch` — P1 · S (#10)
- [ ] **SP-10 Wayland saturation protocol feasibility:** who, where, how — P3 · S (#11)

## Phase 1 — Core + NVIDIA backend + CLI (v0.1)

**Goal:** `vividctl set/get/reset/list/apply/doctor` working on NVIDIA desktops and laptop external ports, persistent at login.

- [x] **vivid-core:** domain types, validated `Vibrance`, `Backend` trait, errors — P0 · M (#12)
- [x] **vivid-nvkms transport:** ioctl wrapper and deterministic fake — P0 · M (#13)
- [x] **vivid-nvkms ABI generations:** per-range layouts, `paramSize` probe, reply validation — P0 · L (#14)
- [ ] **vivid-outputs:** sysfs discovery, owning driver, EDID identity. The backends report identity today; this crate is needed for `doctor`'s hybrid detection — P0 · M (#15)
- [ ] **vivid-config:** TOML model, XDG paths, atomic writes, versioned migration — P1 · M (#16)
- [ ] **vivid-engine:** apply/verify/fallback, per-output backend selection. Done: listing with identity, monitor selection, set and get that continue past a failure, per-output backend selection with each monitor listed once, installing the Shell extension. Pending: profiles, auto-revert — P0 · L (#17)
- [ ] **vivid-cli:** `list`, `get`, `set`, `reset`, `apply`, `preset`. Done: `list`, `get`, `set`, `reset`, `extension install|status|remove`. Pending: `apply`, `preset`, completions and man page — P0 · L (#18)
- [ ] **vividctl doctor:** kernel module for the running kernel, device node, version, hybrid detection, per-output verdict — P0 · M (#19)
- [ ] **Login autostart:** XDG autostart entry and `vividctl install --user`. Done: `vividctl extension install`, which enables the extension for the next login. Pending: the autostart entry — P1 · S (#20)
- [ ] **Hardware tests and usage docs.** Done: NVKMS hardware tests (`VIVID_HW_TESTS`), the user guide. Pending: the GNOME backend on real hardware as a repeatable test, and NVKMS generation 7 `set`/`get` — P1 · S (#21)

## Phase 2 — GNOME compositor backend (done, ships in v0.1.0)

**Goal:** the internal panel of hybrid laptops is controllable; backend chosen per output. Done and checked on a real hybrid laptop. It was finished before Phase 1, so its GitHub milestone should move to v0.1.

- [x] **GNOME Shell extension:** saturation shader, shell 46 to 50. Tested headless on 46, 48, 49 and 50 (CI runs 46 and 50) and on a real session on 46 and 50 — P0 · L (#22)
- [x] **D-Bus interface and `vivid-gnome` client** — P0 · M (#23)
- [x] **Per-output backend selection in the engine** — P0 · M (#24)
- [x] **CI with headless gnome-shell** — P1 · M (#25)

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
