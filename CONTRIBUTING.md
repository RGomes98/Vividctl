# Contributing

Start with the [architecture](docs/architecture.md) for how the crates fit together, and the [roadmap](ROADMAP.md) for what is being worked on.

## Setup

```
rustup show                       # installs the pinned stable toolchain
rustup toolchain install nightly --component rustfmt   # formatting only
sudo apt install build-essential pkg-config libdbus-1-dev
```

The GNOME backend links the system `libdbus-1`, which is why the headers are needed. Without them, work on the NVKMS backend alone with `--no-default-features` on `vivid-cli` or `vivid-engine`.

Formatting uses options that only exist in nightly `rustfmt` (`rustfmt.toml`), so run `cargo +nightly fmt --all`; CI pins a nightly for it. Set `rust-analyzer.check.command` to `clippy` in your editor; CI fails on clippy warnings.

## Running the checks

These are what CI runs. Run them before you push.

```
cargo +nightly fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo deny check && cargo machete          # licences, advisories, unused dependencies
cargo +1.88 check --workspace              # the MSRV in Cargo.toml
scripts/check-budgets.sh                   # binary size and crate count
```

Three more layers need extra setup:

- **The GNOME extension in a headless Shell**, in Docker. Build `vividctl` first so the harness can also run the real command line against the extension:
  ```
  cargo build --release -p vivid-cli
  VIVIDCTL=target/release/vividctl extensions/gnome/test/run.sh 26.04      # or 24.04, 25.04, 25.10
  ```
  It starts a Shell with two virtual monitors in a container, then checks the D-Bus interface, the pixels on each monitor, unredirect, the installer and `vividctl set`/`get`/`reset`. It ends with `failures: 0`. CI runs it on Ubuntu 24.04 (Shell 46) and 26.04 (Shell 50). The binary is built on 24.04 so it runs in both containers.
- **Hardware tests**, which need a local NVIDIA GPU and talk to `/dev/nvidia-modeset`:
  ```
  VIVID_HW_TESTS=1 cargo test --workspace -- --ignored
  ```
  They may change and restore the vibrance of a monitor. Without a monitor on an NVIDIA port they say so and pass.

- **The README quick start on a clean machine**, in Docker: `scripts/check-quick-start.sh 24.04` (or `26.04`) runs the install part of the quick start, as written in the README, as a new user on a clean Ubuntu, then checks that `vividctl` is found in new bash and zsh shells. It builds the committed code, so commit first. CI runs it when the README, the user guide or the CLI changes, and weekly.

When the NVKMS layout table changes, regenerate it from NVIDIA's headers as described in [tests/abi/README.md](tests/abi/README.md); never edit `generations.rs` by hand.

## Workflow

1. Pick or open an issue. Use the Task, Spike, or Bug template.
2. Branch from `main` as `type/kebab-name` (`feat/nvkms-abi-probe`, `docs/adr-0002`).
3. Commit with [Conventional Commits](https://www.conventionalcommits.org/) in English, with a scope (`feat(nvkms): probe ABI generation`). Types: feat, fix, refactor, perf, docs, chore, build, ci, test, spike.
4. Update `CHANGELOG.md` for anything a user would notice, and the docs the change touches (`docs/user-guide.md`, `README.md`, `ROADMAP.md`).
5. Open a PR to `main` with the template. CI runs format, clippy, tests, `cargo-deny`, `cargo-machete`, the MSRV check, the budgets and the extension tests.
6. Get a review. You cannot merge your own PR.

`main` is protected: no direct pushes. A PR says `Closes #N` only when it finishes the issue; otherwise `Part of #N`, and the roadmap line says what is still pending.

## Standards

- One-way crate dependencies (see [docs/architecture.md](docs/architecture.md)); business logic in `vivid-engine`, with `vivid-cli` and `vivid-gui` as thin frontends.
- `unsafe` only in `vivid-nvkms`, each block with a `// SAFETY:` comment. Every other crate starts with `#![forbid(unsafe_code)]`.
- Libraries never print or call `exit`. No `unwrap`, `expect`, `panic` or indexing in libraries (tests may allow them locally).
- Errors are written by hand (`Display` + `Error`), one `error.rs` per crate (ADR-0005).
- Import order: `mod` declarations, `std`, external crates, then `super::` and `crate::`. No fully qualified paths in signatures. Public items have rustdoc in English. Comments say why, not what.
- NVKMS: pass the running driver's version string, never a compiled-in one; validate every reply before a `SET`; a layout change needs a new ABI generation and an update to `tests/abi`.
- No resident process by default. Anything long-running is opt-in and blocks on events instead of polling.

Every feature or fix needs tests. `vivid-nvkms` is tested through the `NvkmsDevice` transport trait, with a scripted fake and an emulator. Injecting a bug on purpose and checking that a test fails is a good way to find a test that checks nothing; this project has found real gaps that way.

## Dependencies

New external crates need a row in `docs/dependencies.md` before they merge: reason, why not `std`, compile cost, number of transitive crates. Use `default-features = false`. The binary has a size and a crate budget (ADR-0006) that CI enforces.

## Decisions

Anything that changes architecture, adds a dependency, or closes a spike gets an ADR (`docs/adr/0000-template.md`, listed in [docs/adr/README.md](docs/adr/README.md)). A spike keeps its raw notes and measurements in `docs/spikes/`.

## Hardware reports

If you test on a laptop or an unusual setup, attach this to the issue:

```
vividctl list
vividctl extension status
uname -r
cat /sys/module/nvidia/version
gnome-shell --version
echo $XDG_SESSION_TYPE
```

Hybrid-laptop reports are especially valuable (see SP-07 in `ROADMAP.md`). A `vividctl doctor` that collects this is planned.

## License

By contributing you agree that your contributions are licensed under Apache-2.0.
