# Contributing

## Setup

```
rustup show                       # installs the pinned stable toolchain
rustup toolchain install nightly --component rustfmt   # formatting only
```

The GNOME backend links the system `libdbus-1`, so building needs its headers:

```
sudo apt install libdbus-1-dev pkg-config
```

Without them, build the NVKMS backend alone with `--no-default-features` on `vivid-cli` or `vivid-engine`.

Set `rust-analyzer.check.command` to `clippy` in your editor; CI fails on clippy warnings.

## Workflow

1. Pick or open an issue. Use the Task, Spike, or Bug template.
2. Branch from `main` as `type/kebab-name` (`feat/nvkms-abi-probe`, `docs/adr-0002`).
3. Commit with Conventional Commits in English, with a scope.
4. Open a PR to `main`. CI runs format, clippy, tests, `cargo-deny`, `cargo-machete`, and an MSRV check.
5. Get a review. You cannot merge your own PR.

`main` is protected: no direct pushes.

## Standards

Code standards: one-way crate dependencies (see [docs/architecture.md](docs/architecture.md)), `unsafe` only in `vivid-nvkms`, no `unwrap`/`expect`/`panic`/indexing in libraries, import ordering, and rustdoc on public items.

Every feature or fix needs tests. `vivid-nvkms` is tested through the `NvkmsDevice` transport trait with a fake; tests that need a real NVIDIA GPU are `#[ignore]` and run with `VIVID_HW_TESTS=1`.

## Dependencies

New external crates need a row in `docs/dependencies.md` before they merge. See the table header for what to measure.

## Decisions

Anything that changes architecture, adds a dependency, or closes a spike gets an ADR (`docs/adr/0000-template.md`).

## Hardware reports

If you test on a laptop or an unusual setup, run `vividctl doctor` and attach its output to the issue. Hybrid-laptop reports are especially valuable (see SP-07 in `ROADMAP.md`).

## License

By contributing you agree that your contributions are licensed under Apache-2.0.
