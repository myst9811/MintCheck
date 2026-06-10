# MintCheck — Agent Operating Guide

MintCheck is a production-grade, Rust-based Autonomous Smart Contract Auditor,
structured as a multi-crate Cargo workspace. The authoritative architecture
reference is `PROJECT_SPEC.md` at the repository root — consult it before any
structural change.

## Workspace Crates

| Crate | Role |
|---|---|
| `auditor-core` | Central orchestration engine + `mintcheck` CLI binary |
| `auditor-parser` | AST generation and normalization (Solidity, Ink!/Anchor) |
| `auditor-detectors` | Static analysis engine (visitor-pattern detectors) |
| `auditor-reporter` | JSON / Markdown report serialization |

## Build Commands

```bash
cargo build                  # build the entire workspace
cargo build --release        # optimized build (use for benchmarking only)
```

## Test Commands

```bash
cargo test                                    # run the full workspace test suite
cargo test --package <crate_name>             # test a single crate, e.g.:
cargo test --package auditor-parser
cargo test --package auditor-detectors
cargo test --package auditor-reporter
cargo test --package auditor-core
```

## Lint Commands

```bash
cargo clippy --all-targets -- -D warnings     # clippy across all targets; warnings are errors
cargo fmt --check                             # verify formatting without modifying files
```

## Strict Rules

1. **Zero clippy warnings before any commit.** `cargo clippy --all-targets -- -D warnings`
   must exit with status `0` before a commit is created. Do not suppress warnings
   with blanket `#[allow(...)]` attributes to satisfy this rule; fix the underlying
   issue. A narrowly-scoped `#[allow]` is acceptable only with a comment explaining
   the constraint that makes the lint wrong.
2. **`cargo fmt --check` must pass before any commit.** Run `cargo fmt` to fix,
   never hand-format.
3. **`cargo test` must pass before any commit.** No commits on a red test suite —
   no exceptions, including "WIP" commits.
4. **Workspace lint policy is law.** `unsafe_code = "forbid"` and
   `clippy::unwrap_used = "deny"` are set at the workspace level. Use proper error
   types (`thiserror` in libraries, `anyhow` only in `auditor-core`'s binary layer).
5. **Respect the dependency graph.** `auditor-parser` depends on no internal crate;
   `auditor-detectors` depends only on `auditor-parser`; `auditor-reporter` depends
   on `auditor-detectors`; only `auditor-core` may perform I/O, read configuration,
   or manage threads. Never invert or shortcut these edges.

## Process

The mandatory development loop — milestone sizing, TDD ordering, push protocol,
and architecture verification — is defined in `.claude/workflow.md`. Follow it
for every change.
