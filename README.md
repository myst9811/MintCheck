# MintCheck: Autonomous Smart Contract Auditor

MintCheck is an end-to-end, multi-crate, Rust-based static analysis toolchain for
auditing smart contracts. It ingests Solidity source text, lowers it into a
normalized intermediate representation (IR), sweeps that tree with
visitor-pattern security detectors, and emits deterministic, severity-sorted
audit reports as Markdown (CLI default) or JSON — all in a single pass from the
`mintcheck` binary.

The full architectural blueprint lives in [`PROJECT_SPEC.md`](PROJECT_SPEC.md);
this README documents what is built and working today.

---

## Architectural Layout

The workspace is a strict directed acyclic graph of four crates. Dependencies
flow leaf-to-root in one direction only — the graph is enforced by Cargo and
never inverted or shortcut:

```text
auditor-parser  ──►  auditor-detectors  ──►  auditor-reporter  ──►  auditor-core
    (leaf)                                                         (root, bin)
```

| Crate | Single Responsibility |
|---|---|
| **`auditor-parser`** | Owns the normalized IR (`ASTNode`, `NodeKind`, `SourceSpan` — byte-offset spans) and the `ParserEngine`: a deterministic, single-pass keyword/brace scanner that lowers Solidity-like source into IR trees. It recognizes `contract` definitions, `function` members (with per-statement `Expression` children), and state-variable declarations, mapping each to exact byte offsets. Depends on no internal crate. |
| **`auditor-detectors`** | Owns the `Detector` trait (`name` / `id` / `severity` / `check`) and the canonical finding types (`Severity`, `Vulnerability`), plus the first concrete scanner: `TxOriginDetector` (SWC-115). Depends only on `auditor-parser`. |
| **`auditor-reporter`** | Owns the `AuditReport` model (accumulation + deterministic severity sorting) and the Stage-5 serializers: `report_to_json` (round-trip-safe pretty JSON) and `report_to_markdown` (title, severity summary table, per-finding detail sections). Depends only on `auditor-detectors`. |
| **`auditor-core`** | The orchestration root and the **only** crate permitted to perform I/O. Provides `audit_source()` — the Parse → Detect → Aggregate pipeline — compatibility re-exports under `auditor_core::report`, and the `mintcheck` CLI binary. Depends on all three crates above. |

Workspace-wide lint law, set in the root `Cargo.toml`: `unsafe_code = "forbid"`
and `clippy::unwrap_used = "deny"`.

## Repo as Memory: Developer Guardrails

This repository encodes its own engineering discipline. Two checked-in
documents govern every change, human or agent:

- [`.claude/CLAUDE.md`](.claude/CLAUDE.md) — the operating guide: build/test/lint
  commands, the workspace lint policy, and the dependency-graph rules.
- [`.claude/workflow.md`](.claude/workflow.md) — the mandatory development loop.

The automated quality gates, all of which must pass **before any commit is
created**:

1. **Formatting:** `cargo fmt --check` must be clean (fixes via `cargo fmt`,
   never by hand).
2. **Zero clippy warnings:** `cargo clippy --all-targets -- -D warnings` must
   exit `0`; blanket `#[allow]` suppressions are prohibited.
3. **Workspace-green tests:** the full `cargo test` suite must pass — a green
   run on only the changed crate is insufficient.
4. **Atomic commits, hard 300 LOC cap:** one coherent change per commit;
   oversized work is decomposed along crate boundaries first.
5. **Push-on-commit:** every commit is pushed immediately — no local-only state.
6. **Architecture verification:** before any new detector or core feature, an
   explicit compliance check against `PROJECT_SPEC.md` is printed and any
   deviation is surfaced before code is written.

## Current Capabilities & Detectors

| Detector | ID | Severity | Technique |
|---|---|---|---|
| `TxOriginDetector` | SWC-115 | **High** | Walks the IR in pre-order and, for every `Expression` node, slices the original source by the node's byte span (localized source-slicing validation) to detect `tx.origin` usage — e.g. authorization checks like `require(tx.origin == owner)`. Reports the exact offending span with remediation guidance (`msg.sender`). |

Findings carry a stable identifier, title, description, severity, file path,
and precise byte-span bounds. Reports are deterministic: severity-sorted
(Critical first) with stable tie-breaking by file path, span start, then
detector ID.

Pipeline error handling is explicit end-to-end: empty sources, unnamed
contracts, unbalanced braces, and unterminated declarations surface as typed
`ParseError`s, which the CLI converts to clean stderr messages and exit code 1.

**Current scope, stated honestly:** the parser is a deterministic
keyword/brace scanner purpose-built for the audit pipeline — not a full
Solidity grammar (string-literal braces are treated structurally; expressions
are span-level, not sub-parsed). One detector is live; the `Detector` trait
and the one-line `detector_suite()` registry in `auditor-core` are the
extension points for the SWC catalog to come. CLI ingestion is single-file.

## Quick Start & Usage

### Build & Test

```bash
cargo build                                   # build all four crates + CLI
cargo test                                    # full workspace suite (31 tests)
cargo test --package auditor-parser           # or any single crate
cargo clippy --all-targets -- -D warnings     # the zero-warning gate
cargo fmt --check                             # the formatting gate
```

### Audit a Contract

```bash
cargo run --bin mintcheck -- path/to/Contract.sol
```

Given a vulnerable contract:

```solidity
contract Phish {
    address owner;

    function auth() public {
        require(tx.origin == owner);
    }
}
```

`mintcheck` prints the Markdown report to stdout (exit `0`; missing arguments
or unreadable files print to stderr and exit `1`):

```markdown
# Smart Contract Audit Report

**Target:** `/tmp/Phish.sol`
**Generated (unix):** 1781087614
**Total findings:** 1

## Summary

| Severity | Count |
|----------|-------|
| Critical | 0 |
| High | 1 |
| Medium | 0 |
| Low | 0 |
| Info | 0 |

## Findings

### 1. Authorization via tx.origin [High]

- **ID:** SWC-115
- **Severity:** High
- **File:** `/tmp/Phish.sol`
- **Span:** bytes 74..102

tx.origin is the transaction originator, not the immediate caller; a phishing
contract can relay calls that pass this check. Use msg.sender for authorization.
```

### Library Use

The same pipeline is available programmatically:

```rust
let report = auditor_core::audit_source("contracts/Phish.sol", source)?;
let json = auditor_reporter::report_to_json(&report)?;
let markdown = auditor_reporter::report_to_markdown(&report);
```

## Roadmap

Per `PROJECT_SPEC.md`: additional SWC detectors (reentrancy, unchecked calls,
delegatecall, timestamp dependence), directory-walking ingestion with a
`--json` output flag, Rust frontends (Ink!/Anchor), and the Stage 4b LLM
review seam for validating static findings.
