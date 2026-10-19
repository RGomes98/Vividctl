# ADR-0006: CLI parser and build budgets

- Status: accepted
- Date: 2026-10-19
- Spike: #7 (method and numbers in `docs/spikes/sp-06-build-budgets/README.md`)

## Context

The CLI needs an argument parser and the GUI a toolkit. The project asks for fast compiles and few dependencies, so both choices need numbers. SP-06 built the same CLI surface (`list`, `get`, `set <VALUE>`, `reset`, `doctor`, `--output`, `--json`) five ways, and a GTK4 + libadwaita skeleton on Ubuntu 24.04, with the product's release profile. Reference machine: i5-9400F, 6 cores, rustc 1.99.1.

The numbers that matter:

- The parser alone: `lexopt` costs 2.7 s of clean release build, 305 KiB and 2 crates; `clap` (builder) 8.6 s, 550 KiB, 5 crates; `clap` (derive) 9.1 s, 561 KiB, 13 crates.
- In the realistic graph, where `vivid-config` brings `serde` (derive) and `toml` and so `syn`, the marginal cost over that stack is: `lexopt` +0.15 s and +13 KiB; `clap` builder +3.4 s and +255 KiB; `clap` derive +3.7 s and +268 KiB. Derive over builder costs only +0.3 s release and +13 KiB there.
- The final binary's fat LTO is the single largest compile cost (about 2.5 s even for an empty binary).
- What the parsers give the user: `clap` generates per-subcommand help, readable errors, typo suggestions (`suggestions` feature: +12 KiB, +1 crate), and shell completions and man pages through `clap_complete` and `clap_mangen`. The hand-written parsers need all of that written by hand. `lexopt` also reads `set -300` as the options `-3 -0 -0`, which matters for a tool whose values are often negative.
- The GUI: 54 s clean release, 75 crates, a 333 KiB binary linked dynamically to GTK and libadwaita, which a stock Ubuntu desktop already has. It costs about five times the whole CLI to compile.

## Decision

1. **`vivid-cli` uses `clap` 4 with derive**, `default-features = false` and the features `std`, `derive`, `help`, `usage`, `error-context` and `suggestions`. No colour, environment variables, wrapping or Unicode features.
2. **Shell completions and man pages are generated in `xtask`** with `clap_complete` and `clap_mangen`. They are build-time tools and never linked into `vividctl`.
3. **The GUI toolkit stays GTK4 + libadwaita**, with the feature floors `gtk4` `v4_14` and `libadwaita` `v1_5`, only in `vivid-gui`. Because it dominates compile time and needs system `-dev` packages, `vivid-gui` is left out of `default-members` when it gets real dependencies, and CI builds it in its own job with the packages installed (task of #26).
4. **Budgets**, enforced by the `budgets` CI job (`scripts/check-budgets.sh`) on the stripped release `vividctl`:
   - binary size: at most **1.5 MiB**
   - unique crates in the `vivid-cli` dependency tree, workspace crates included: at most **40**
   Build time is recorded and not enforced, because runners are too noisy: clean release build of `vividctl` should stay under **30 s** on the reference machine, and the GUI's clean release build is recorded at 54 s.
5. **A budget is a ceiling, not a target.** Raising one needs a reason in this ADR (or a new one).

How the ceilings were set: the CLI with `serde`, `toml` and `clap` (derive, suggestions) measures 766 KiB and 23 crates. Adding `rustix` and the workspace crates puts the expected final size at 0.8 to 1.0 MiB and the crate count near 30, so 1.5 MiB and 40 leave headroom and still catch a heavy dependency. Today's empty `vividctl` is 285 KiB with 1 crate.

## Consequences

- The CLI spends about 270 KiB and 3.7 s of clean build, once, on help, errors, suggestions, completions and man pages that would otherwise be hand-written and kept up to date. This is a deliberate trade against the "few dependencies" goal.
- `clap_complete` and `clap_mangen` are dependencies of `xtask`, not of the product.
- The GUI crate needs special handling in the workspace and in CI. Until it has real dependencies the stub does not.
- The size and crate budgets are checked on every PR. A dependency that breaks them shows up in review.

## Alternatives considered

- **`lexopt`**: the leanest (+13 KiB, +0.15 s over the config stack). Rejected because the CLI will have around ten subcommands and presets, and per-subcommand help, typo suggestions, completions and man pages would all be hand-written, with the negative-number pitfall on top. If a future budget is broken by `clap`, `lexopt` is the way back; the CLI crate is thin, so the change is small.
- **`pico-args`** and a **hand-rolled parser**: the same trade as `lexopt`.
- **`clap` builder API**: saves 0.3 s and 13 KiB once `syn` is shared, but derive gives typed command structures that the engine can consume directly.
- **A GUI toolkit other than GTK4 + libadwaita**: not re-measured here. ADR-level reasoning is unchanged: it is native on the target desktop, and its runtime is already installed.
