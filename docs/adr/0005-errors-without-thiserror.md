# ADR-0005: Errors written by hand in vivid-core, no thiserror

- Status: accepted
- Date: 2026-10-19
- Issue: #12

## Context

The plan was to derive error types with `thiserror`, one `error.rs` per crate. The dependency policy (`docs/dependencies.md`) asks every external crate to justify itself against the standard library and to report its compile cost and transitive count.

Measured on this machine (rustc 1.96, clean build of a one-enum crate, `default-features = false`):

| | Transitive crates | Clean build |
|---|---|---|
| `thiserror` 2.0.21 | 6 (`thiserror`, `thiserror-impl`, `proc-macro2`, `quote`, `syn`, `unicode-ident`) | 2.18 s |
| Hand-written `Display` and `Error` | 0 | 0.09 s |

The hand-written version costs about five lines per variant group: a `match` in `Display` and an empty `impl Error`. `vivid-core` has two error types.

## Decision

`vivid-core` writes its errors by hand (`Display` and `std::error::Error`) and has no dependencies. The `thiserror` task of #12 is closed by this ADR instead of by adding the crate.

Other crates decide for themselves under the same policy. If a crate already pulls in `syn` for another reason (for example `serde` derive in `vivid-config`), `thiserror` then costs one extra proc-macro crate and may be worth adopting there, with its own register row.

## Consequences

- `vivid-core` compiles in a fraction of a second and the crate that every other crate depends on adds nothing to the dependency graph.
- Error types need their `Display` written out. With a `#[non_exhaustive]` enum, forgetting a variant is a compile error, not a silent gap.
- There is no `#[from]` conversion. Conversions between error types are written where they are needed.

## Alternatives considered

- **`thiserror`** (the original plan): less boilerplate, at the cost shown above for a few small enums.
- **`anyhow`** or another dynamic error type in the library crates: loses the error kinds that the engine and the frontends react to. It may still suit a binary's `main`.
