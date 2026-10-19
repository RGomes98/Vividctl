# SP-06: build cost budgets

Issue: #7. Status: done, decision and budgets in `docs/adr/0006-cli-parser-and-budgets.md`.

## Question

What do the CLI argument parser and the GUI toolkit cost in compile time, binary size and dependencies, so the choice and the CI budgets rest on numbers?

## Method

Reference machine: Intel i5-9400F (6 cores), 15 GB RAM, rustc 1.99.1. The GUI ran in an Ubuntu 24.04 container (GTK 4.14.5, libadwaita 1.5.0, 6 CPUs), the compatibility floor of the project.

- Every candidate is a standalone crate in `cli/` (or `gui/`) with the **same** release profile as the product: `opt-level = 3`, `lto = true`, `codegen-units = 1`, `panic = "abort"`, `strip = true`.
- All CLI candidates implement the same surface: `list [--json]`, `get`, `set <VALUE>`, `reset`, `doctor`, `--output <ID>`, `--help`, `--version`. `cli/check.sh` runs 20 invocations against all five and compares what they print and their exit codes: they agree.
- `measure.sh` fetches dependencies first (compile time only, no download), then takes three runs of each measure from an empty target directory and reports the **median**: clean release build, clean dev build, incremental dev rebuild after touching `main.rs`. Binary size is the stripped release binary. Crates are the unique crates in `cargo tree -e normal`, including the crate itself.

## Results

### The parser alone

| Candidate | Clean release | Clean dev | Incremental dev | Binary | Crates | Lines |
|---|---|---|---|---|---|---|
| `std` only, by hand | 2.5 s | 0.13 s | 0.08 s | 294 KiB | 1 | 79 |
| `lexopt` 0.3 | 2.7 s | 0.24 s | 0.08 s | 305 KiB | 2 | 88 |
| `pico-args` 0.5 | 2.6 s | 0.20 s | 0.08 s | 297 KiB | 2 | 73 |
| `clap` 4.6, builder API | 8.6 s | 1.85 s | 0.11 s | 550 KiB | 5 | 37 |
| `clap` 4.6, derive | 9.1 s | 3.11 s | 0.13 s | 561 KiB | 13 | 44 |

`clap` features: `std`, `help`, `usage`, `error-context`, plus `derive` for the derive row. The line counts include the help text the hand-written versions must carry.

### The parser inside a realistic graph

The `vivid-config` crate will use `serde` with derive and `toml`, which already pulls in `syn`. So the cost that matters is the marginal one, measured with `serde` (derive) and `toml` in every row:

| Candidate | Clean release | Clean dev | Binary | Crates |
|---|---|---|---|---|
| `serde` + `toml` only | 7.3 s | 3.40 s | 486 KiB | 15 |
| + `lexopt` | 7.5 s | 3.41 s | 499 KiB | 16 |
| + `clap`, builder | 10.7 s | 3.70 s | 741 KiB | 19 |
| + `clap`, derive | 11.0 s | 4.32 s | 754 KiB | 22 |
| + `clap`, derive, `suggestions` | not timed | not timed | 766 KiB | 23 |

Marginal cost over `serde` + `toml`:

| Parser | Clean release | Binary | Crates |
|---|---|---|---|
| `lexopt` | +0.15 s | +13 KiB | +1 |
| `clap`, builder | +3.4 s | +255 KiB | +4 |
| `clap`, derive | +3.7 s | +268 KiB | +7 |

Derive over builder, once `syn` is shared: +0.3 s release, +0.6 s clean dev, +13 KiB, +3 crates. The `serde` + `toml` stack itself costs about +190 KiB and +4.8 s over the bare binary.

### `cargo build --timings` (serde + toml + clap derive, clean release, 11.0 s wall)

| Unit | Time |
|---|---|
| the binary itself (fat LTO, one codegen unit) | 6.3 s |
| `clap_builder` | 4.5 s |
| `syn` | 1.8 s |
| `serde_core` | 1.6 s |
| `serde_derive` | 1.4 s |
| `toml` | 1.3 s |
| `clap_derive` | 0.9 s |

The release profile's LTO is the largest single cost, about 2.5 s even for the empty binary. `clap` adds one big unit (`clap_builder`).

### `cargo bloat` (same build, unstripped, `.text` 602 KiB)

| Crate | Size |
|---|---|
| `std` | 270 KiB |
| `clap_builder` | 195 KiB |
| `toml` | 45 KiB |
| the binary's own code | 38 KiB |
| `toml_parser` | 29 KiB |

### What the parsers do for the user

| | `clap` | `lexopt` / `pico-args` / by hand |
|---|---|---|
| Help per subcommand (`vividctl set --help`) | generated | written by hand; mine prints only the global help |
| Error text | `invalid value 'abc' for '<VALUE>': invalid digit found in string` | what the code writes (`invalid <VALUE>`) |
| Typo suggestions (`lst`, `--outptu`) | with the `suggestions` feature (+12 KiB, +1 crate): `a similar subcommand exists: 'list'` | none |
| Shell completions and man pages | `clap_complete` and `clap_mangen`, run in `xtask`, not in the binary | by hand |
| Negative numbers as arguments (`set -300`) | `allow_hyphen_values` | `lexopt` reads `-300` as the options `-3 -0 -0`; it needed 6 extra lines (see `cli/lexopt`). `pico-args` and the by-hand parser accept it |

A negative vibrance is a normal argument for this tool, so the `lexopt` pitfall is not hypothetical.

### The GUI (GTK4 + libadwaita, Ubuntu 24.04 container)

`gui/` is a window with a header bar and a slider, with the feature floors `gtk4` `v4_14` and `libadwaita` `v1_5`. `gtk4` 0.11 and `libadwaita` 0.9.

| Clean release | Clean dev | Incremental dev | Binary | Crates |
|---|---|---|---|---|
| 54.1 s | 42.1 s | 0.30 s | 333 KiB | 75 |

The binary links dynamically: 7 direct shared libraries (`libadwaita-1`, `libgtk-4`, `libgio`, `libgobject`, `libglib`, `libgcc_s`, `libc`), 104 resolved transitively. The GTK and libadwaita runtime are installed on a stock Ubuntu GNOME desktop (24.04 ships GTK 4.14.5 and libadwaita 1.5.0). Building needs `libgtk-4-dev` and `libadwaita-1-dev`.

The GUI costs about five times the whole CLI to compile and has seven times more crates than the CLI with `serde` and `toml`. It must stay in its own crate, and the day-to-day build of everything else must not compile it.

## Not covered

- Runtime memory and startup time of the GUI (no display in the container).
- GTK 4.20 and libadwaita 1.8 (Ubuntu 26.04): only the 24.04 floor was built.
- Other CLI parsers (`argh`, `bpaf`): not measured. The candidates above cover the range from nothing to the full-featured option.
- Build times on CI runners: the numbers are for one desktop CPU and are for comparison, not for budgets.

## Reproducing

```
cd docs/spikes/sp-06-build-budgets
cli/check.sh <dir with the five binaries>     # behaviour equivalence
./measure.sh cli/clap-derive sp06-clap-derive 3
docker build -t sp06-gui gui && docker run --rm -v "$PWD:/src:ro" sp06-gui bash -c 'cp -r /src /spike/work && cd /spike/work && ./measure.sh gui sp06-gui 3'
```
