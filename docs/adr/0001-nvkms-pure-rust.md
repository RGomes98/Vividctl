# ADR-0001: Control Digital Vibrance through NVKMS ioctls from pure Rust

- Status: accepted
- Date: 2026-10-19
- Spike: #2 (notes in `docs/spikes/sp-01-nvkms/README.md`)

## Context

On Wayland, `nvidia-settings` cannot change Digital Vibrance. nvibrant does it by calling ioctls on `/dev/nvidia-modeset` (NVKMS), compiled in C++ against the headers of one specific driver tag. Its releases break when a driver changes a struct, and its output addressing is positional.

The question for this spike: can the same ioctls be driven from Rust alone, with hand-written layouts, no bindgen, no C++ and no NVIDIA libraries?

Evidence, on driver 595.99.02, GeForce GTX 1660 Ti, GNOME on Wayland:

- A PoC of about 400 lines using `rustix` enumerated the display, read the current vibrance with `GET_DPY_ATTRIBUTE`, set a new one with `SET_DPY_ATTRIBUTE`, and restored it. Reading back after each change returned the value we set.
- A visual test on the real monitor (Philips PHL 242V8, HDMI-0): vibrance 1000 for 10 seconds made the image visibly more saturated, and restoring 0 brought it back.
- The kernel rejects a wrong `versionString` (reply status `VERSION_MISMATCH`) and any wrong `paramSize`, in both directions, with `EPERM` and no side effect. Details in the spike notes.
- The driver version is available at runtime from `/sys/module/nvidia/version`.

## Decision

Go. The NVIDIA backend (`vivid-nvkms`) talks to NVKMS directly from Rust, with these rules:

1. **Sparse layouts as data.** Each ioctl parameter block is a zeroed buffer of the exact size the kernel expects. Only the fields we use are read or written, at offsets taken from the C headers. A layout is a table of sizes and offsets, grouped into ABI generations by driver version range. We do not copy the full structs.
2. **Generated, checked numbers.** The offsets come from a C program compiled against the NVIDIA headers (`layout.c`, to become `tests/abi`). A scheduled CI job re-runs it on new driver tags and fails on drift (#31).
3. **Runtime version.** The `versionString` is read from `/sys/module/nvidia/version` on every run, never compiled in, with an explicit override for diagnostics.
4. **Zeroed request blocks.** Request structs are zero-initialised, in particular the force/override flags of `QUERY_DPY_DYNAMIC_DATA`. nvibrant leaves them uninitialised.
5. **Validate before writing.** After `ALLOC_DEVICE`, check the reply (status, `numDisps`, `numConnectors` within bounds) before any `SET`.
6. **Verify with readback.** After a `SET`, read the value back with `GET_DPY_ATTRIBUTE` and report a mismatch.
7. **Do not trust errno.** Every rejection is `EPERM`. Distinguish causes through the reply status where the kernel writes one, and through our own probing (try the newest known layout, then fall back).
8. **Dependency.** `rustix` with `default-features = false` and features `std`, `fs` (transitive: `bitflags`, `linux-raw-sys`). `unsafe` stays confined to `vivid-nvkms`. A row in `docs/dependencies.md` records it.

## Consequences

- No libclang, no C++ toolchain and no per-driver build. The backend compiles in about a second on top of its dependencies.
- A new driver that changes a struct we use is detected by CI before users hit it, and by the runtime probe if it slips through.
- NVKMS stays an internal, undocumented NVIDIA interface. This ADR reduces the cost of a break, it does not remove the risk.
- Output identity cannot rely on connector order, which is what nvibrant does (SP-05, #6).

## Not covered by this spike

Tested on one GPU, one monitor on HDMI and one driver version. Still open:

- DisplayPort outputs, several monitors, several GPUs, and laptops (#8).
- Whether the value survives suspend/resume, VT switches, hotplug and compositor restarts (#4).
- The ABI differences between driver versions (#3).
- Behaviour with HDR and in fullscreen games. NVKMS acts in the display engine, so it should apply everywhere, but that was not measured here.

## Alternatives considered

- **bindgen over the NVIDIA headers.** Needs libclang at build time and a full struct copy per driver version, which makes the diff between versions large and hard to audit.
- **Vendoring nvibrant's C++.** Keeps the per-driver build and the manual releases that this project exists to avoid.
- **NVML.** Not investigated here. nvibrant's author lists it as a future direction, but it is not known to expose Digital Vibrance.
- **Compositor-only saturation.** Mutter has no user-adjustable saturation, and it cannot reach what the display engine does. It remains the plan for outputs the NVIDIA GPU does not drive (hybrid laptop panels), tracked in #5.
