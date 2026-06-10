# MintCheck — Project Specification

**Autonomous Smart Contract Auditor (Rust)**

| | |
|---|---|
| **Version** | 0.1.0 |
| **Status** | Draft — Foundational Architecture |
| **Language** | Rust (MSRV 1.78, Edition 2021) |
| **Targets** | Solidity (EVM), Rust smart contracts (Ink! / Anchor) |
| **License** | Apache-2.0 |

---

## 1. Overview

MintCheck is a production-grade, autonomous smart contract auditing engine. It ingests
Solidity and Rust-based (Ink!/Anchor) contract source code, normalizes it into a unified
Abstract Syntax Tree (AST), runs a battery of static-analysis detectors over that tree
via a visitor-pattern traversal engine, aggregates findings, and emits machine-readable
(JSON) and human-readable (Markdown) audit reports.

The system is designed from day one to accommodate an **LLM Reviewer** stage that
validates, deduplicates, and enriches static-analysis findings before report generation
(see §5.3).

### Design Principles

1. **Strict separation of concerns** — each pipeline stage lives in its own crate with a
   narrow, versioned public API. Parsers never know about detectors; detectors never
   know about report formats.
2. **Language-agnostic core** — all analysis operates on the normalized `ASTNode` IR.
   Adding a new source language means adding a frontend, not touching detectors.
3. **Deterministic, reproducible output** — identical inputs produce byte-identical
   reports (stable ordering, content-addressed finding IDs).
4. **Fail-soft analysis** — a panicking detector or unparseable file degrades to a
   diagnostic in the report; it never aborts the audit run.

---

## 2. Workspace Architecture

### 2.1 Crate Layout

```text
mintcheck/
├── Cargo.toml                      # [workspace] virtual manifest
├── PROJECT_SPEC.md
├── crates/
│   ├── auditor-core/               # Central orchestration engine (lib + bin)
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # AuditEngine, pipeline driver
│   │       ├── config.rs           # AuditConfig, detector registry config (TOML)
│   │       ├── pipeline.rs         # Stage scheduling, parallelism (rayon)
│   │       ├── review.rs           # ReviewStage trait — LLM hook point (§5.3)
│   │       └── main.rs             # `mintcheck` CLI binary (clap)
│   ├── auditor-parser/             # AST generation & normalization
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # Parser trait, LanguageFrontend dispatch
│   │       ├── ir.rs               # ASTNode, NodeKind, SourceSpan (canonical IR)
│   │       ├── solidity/
│   │       │   ├── mod.rs          # Solang-driven Solidity frontend
│   │       │   └── lower.rs        # solang_parser::pt::* → ASTNode lowering
│   │       └── rust/
│   │           ├── mod.rs          # syn-driven Rust frontend (Ink!/Anchor)
│   │           ├── ink.rs          # #[ink::contract] attribute resolution
│   │           └── anchor.rs       # #[program] / Accounts<'info> resolution
│   ├── auditor-detectors/          # Static analysis & linting engine
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs              # Detector trait, DetectorRegistry
│   │       ├── visitor.rs          # AstVisitor trait, walk_* free functions
│   │       ├── context.rs          # AnalysisContext (symbol table, call graph)
│   │       └── rules/
│   │           ├── mod.rs
│   │           ├── reentrancy.rs        # SWC-107
│   │           ├── unchecked_call.rs    # SWC-104
│   │           ├── integer_overflow.rs  # SWC-101 (pre-0.8 / unchecked blocks)
│   │           ├── access_control.rs    # SWC-105/106, missing signer checks
│   │           ├── tx_origin.rs         # SWC-115
│   │           ├── delegatecall.rs      # SWC-112
│   │           ├── timestamp_dep.rs     # SWC-116
│   │           └── anchor_owner_check.rs # Anchor: missing owner/discriminator check
│   └── auditor-reporter/           # JSON / Markdown serialization
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs              # Reporter trait, ReportFormat enum
│           ├── model.rs            # AuditReport, Vulnerability, Severity
│           ├── json.rs             # serde_json emitter (schema-versioned)
│           ├── markdown.rs         # Tera-templated Markdown emitter
│           └── templates/
│               └── report.md.tera
├── fixtures/                       # Known-vulnerable corpus for integration tests
│   ├── solidity/
│   └── rust/
└── xtask/                          # cargo-xtask: codegen, fixture refresh, dist
```

### 2.2 Dependency Graph (acyclic, enforced by `cargo deny`)

```text
                    ┌────────────────────┐
                    │   auditor-core     │  (bin: mintcheck)
                    └──┬───────┬──────┬──┘
                       │       │      │
          ┌────────────▼─┐ ┌───▼──────────────┐ ┌─▼──────────────────┐
          │auditor-parser│ │auditor-detectors │ │  auditor-reporter  │
          └──────────────┘ └───┬──────────────┘ └─┬──────────────────┘
                               │ (ASTNode IR)     │ (Vulnerability model)
                               ▼                  ▼
                         auditor-parser     auditor-detectors
```

- `auditor-parser` is the **leaf** crate: zero internal dependencies; owns the IR.
- `auditor-detectors` depends only on `auditor-parser` (consumes `ASTNode`).
- `auditor-reporter` depends on `auditor-detectors` (consumes `Vulnerability`).
- `auditor-core` depends on all three and is the only crate allowed to perform I/O,
  spawn threads, or read configuration.

### 2.3 Workspace Manifest

```toml
[workspace]
resolver = "2"
members = ["crates/auditor-core", "crates/auditor-parser",
           "crates/auditor-detectors", "crates/auditor-reporter", "xtask"]

[workspace.package]
version = "0.1.0"
edition = "2021"
rust-version = "1.78"
license = "Apache-2.0"
repository = "https://github.com/myst9811/mintcheck"

[workspace.dependencies]
# --- parsing frontends ---
solang-parser = "0.3"          # Solidity → parse tree (pt::SourceUnit)
syn = { version = "2", features = ["full", "extra-traits", "visit"] }
proc-macro2 = { version = "1", features = ["span-locations"] }
tree-sitter = "0.22"           # error-tolerant fallback grammar host
tree-sitter-solidity = "1"     # recovery parsing of malformed Solidity

# --- serialization & reporting ---
serde = { version = "1", features = ["derive"] }
serde_json = "1"
schemars = "0.8"               # JSON Schema generation for AuditReport
tera = "1"                     # Markdown report templating
semver = "1"

# --- engine plumbing ---
rayon = "1"                    # per-file parallel detector execution
thiserror = "1"                # library error types
anyhow = "1"                   # binary-level error context
clap = { version = "4", features = ["derive"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
walkdir = "2"
camino = { version = "1", features = ["serde1"] }   # UTF-8 paths
blake3 = "1"                   # content-addressed finding IDs
indexmap = { version = "2", features = ["serde"] }  # stable iteration order
toml = "0.8"                   # mintcheck.toml configuration

# --- future: LLM reviewer (feature-gated, see §5.3) ---
async-trait = "0.1"
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }

[workspace.lints.rust]
unsafe_code = "forbid"
[workspace.lints.clippy]
unwrap_used = "deny"
pedantic = { level = "warn", priority = -1 }
```

---

## 3. Technical Stack & Dependencies

### 3.1 Per-Crate Dependency Assignments

| Crate | Dependencies (from workspace) | Rationale |
|---|---|---|
| `auditor-parser` | `solang-parser`, `syn`, `proc-macro2`, `tree-sitter`, `tree-sitter-solidity`, `serde`, `thiserror`, `camino`, `indexmap` | `solang-parser` provides a pure-Rust, battle-tested Solidity grammar (used by Hyperledger Solang) producing `pt::SourceUnit` parse trees with byte-offset locations. `syn` (with `full` + `visit`) parses arbitrary Rust source including Ink!/Anchor attribute macros; `proc-macro2`'s `span-locations` feature recovers line/column data. `tree-sitter-solidity` is the **error-recovery fallback**: when `solang-parser` rejects a file, we re-parse with tree-sitter to still extract partial structure rather than skipping the file. |
| `auditor-detectors` | `auditor-parser`, `serde`, `thiserror`, `indexmap`, `blake3`, `tracing` | Pure analysis logic over the IR. `blake3` hashes `(detector_id, file, span, code_snippet)` into stable finding fingerprints used for deduplication and baseline suppression. |
| `auditor-reporter` | `auditor-detectors`, `serde`, `serde_json`, `schemars`, `tera`, `semver`, `thiserror` | `schemars` derives a published JSON Schema so CI consumers can validate report payloads. `tera` renders the Markdown template; `semver` stamps `report_schema_version`. |
| `auditor-core` | all three internal crates + `clap`, `rayon`, `anyhow`, `walkdir`, `toml`, `tracing-subscriber`, `tokio`/`async-trait` (behind `llm-review` feature) | Sole I/O boundary: file discovery, config loading, thread-pool management, exit codes (`0` clean, `1` findings ≥ configured fail-level, `2` engine error). |

### 3.2 Parsing Strategy Detail

- **Solidity:** primary path is `solang_parser::parse(&source, file_id)` →
  `pt::SourceUnit`. The `solidity::lower` module performs a single-pass lowering of
  `pt::ContractDefinition`, `pt::FunctionDefinition`, `pt::Statement`, and
  `pt::Expression` into the canonical `ASTNode` IR, preserving `pt::Loc` byte ranges
  as `SourceSpan`s. Pragma versions are captured into node metadata so detectors can
  gate on compiler semantics (e.g., checked arithmetic in `>=0.8.0`).
- **Rust (Ink!):** `syn::parse_file` → walk `ItemMod` items carrying
  `#[ink::contract]`; classify `#[ink(message)]`, `#[ink(storage)]`,
  `#[ink(constructor)]` items into IR `NodeKind::Function { visibility: External }`,
  `NodeKind::StateVariable`, etc.
- **Rust (Anchor):** detect `#[program]` modules and `#[derive(Accounts)]` structs;
  account constraints (`#[account(mut, has_one = owner)]`) are lowered into
  `NodeKind::Modifier` nodes so access-control detectors treat Solidity modifiers and
  Anchor constraints uniformly.
- **Macro limitation (documented, accepted):** `syn` does not expand macros. Ink!/Anchor
  analysis operates on pre-expansion source, which is sufficient for attribute-driven
  structural detectors. Post-expansion analysis via `cargo expand` integration is
  deferred to v0.3 and isolated behind the `Parser` trait so it requires no detector
  changes.

---

## 4. Core Data Structures

All three types below are the **frozen contract** between crates. Changes require a
minor version bump of the owning crate and a `report_schema_version` bump where
externally visible.

### 4.1 `ASTNode` — Normalized IR (`auditor-parser/src/ir.rs`)

```rust
use camino::Utf8PathBuf;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// Identifies which frontend produced a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SourceLanguage {
    Solidity,
    RustInk,
    RustAnchor,
}

/// Byte- and line-addressed location of a node in its source file.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SourceSpan {
    pub file: Utf8PathBuf,
    /// Byte offsets into the original source (half-open: [start, end)).
    pub start_byte: usize,
    pub end_byte: usize,
    /// 1-based line/column for human-facing reports.
    pub start_line: u32,
    pub start_col: u32,
    pub end_line: u32,
    pub end_col: u32,
}

/// Stable, language-agnostic node identifier (index into the owning `Ast` arena).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct NodeId(pub u32);

/// Function/state mutability, unified across languages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Mutability {
    Pure,
    View,
    Payable,
    NonPayable,
}

/// Unified visibility lattice (Anchor instruction handlers map to `External`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Visibility {
    External,
    Public,
    Internal,
    Private,
}

/// The normalized node taxonomy. Every frontend lowers into exactly these kinds;
/// language-specific detail that detectors may need goes into `ASTNode::attributes`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum NodeKind {
    SourceUnit,
    Contract { name: String, is_abstract: bool, bases: Vec<String> },
    Interface { name: String },
    Library { name: String },
    Function {
        name: String,
        visibility: Visibility,
        mutability: Mutability,
        is_constructor: bool,
        modifiers: Vec<String>,
    },
    Modifier { name: String },
    StateVariable {
        name: String,
        type_name: String,
        visibility: Visibility,
        is_immutable: bool,
        is_constant: bool,
    },
    LocalVariable { name: String, type_name: String },
    Parameter { name: Option<String>, type_name: String },
    Block,
    If,
    Loop { kind: LoopKind },
    Return,
    Emit { event: String },
    Require { message: Option<String> },
    Assignment { operator: String },
    BinaryOp { operator: String },
    UnaryOp { operator: String },
    /// Any call: internal, external, low-level, or CPI. The single most
    /// important node for security analysis.
    Call {
        callee: String,
        call_kind: CallKind,
        value_forwarded: bool,
        gas_forwarded: bool,
    },
    MemberAccess { member: String },
    Identifier { name: String },
    Literal { value: String, type_hint: Option<String> },
    InlineAssembly,
    /// Constructs with no unified meaning (e.g. `unchecked {}` blocks, Rust
    /// `unsafe`, Anchor `remaining_accounts`). Preserved, never dropped.
    LanguageSpecific { tag: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LoopKind {
    For,
    While,
    DoWhile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CallKind {
    Internal,
    External,
    /// Solidity `.call` / `.delegatecall` / `.staticcall` / `.send` / `.transfer`.
    LowLevel,
    Delegatecall,
    /// Solana cross-program invocation (`invoke` / `invoke_signed` / CPI builders).
    CrossProgram,
    EventEmission,
}

/// A single node in the normalized AST. Nodes live in `Ast::nodes` (an arena);
/// tree shape is expressed through `parent` / `children` indices, which keeps
/// the IR serializable, cache-friendly, and cycle-free by construction.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ASTNode {
    pub id: NodeId,
    pub kind: NodeKind,
    pub span: SourceSpan,
    pub language: SourceLanguage,
    pub parent: Option<NodeId>,
    pub children: Vec<NodeId>,
    /// Escape hatch for frontend-specific facts detectors may consult,
    /// e.g. {"pragma": ">=0.8.0"}, {"ink_selector": "0xCAFEBABE"},
    /// {"anchor_constraints": "mut,has_one=owner"}.
    pub attributes: IndexMap<String, String>,
}

/// One fully-parsed source file: arena + root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ast {
    pub root: NodeId,
    pub nodes: Vec<ASTNode>,
    pub source_file: Utf8PathBuf,
    pub language: SourceLanguage,
    /// blake3 hash of raw source bytes; ties findings to exact inputs.
    pub source_hash: String,
}

impl Ast {
    pub fn node(&self, id: NodeId) -> &ASTNode {
        &self.nodes[id.0 as usize]
    }
    pub fn children(&self, id: NodeId) -> impl Iterator<Item = &ASTNode> {
        self.node(id).children.iter().map(|c| self.node(*c))
    }
    /// Walk ancestors toward the root (used by e.g. "is this call inside a loop?").
    pub fn ancestors(&self, id: NodeId) -> impl Iterator<Item = &ASTNode> {
        std::iter::successors(self.node(id).parent, |p| self.node(*p).parent)
            .map(|p| self.node(p))
    }
}
```

### 4.2 `Vulnerability` (`auditor-reporter/src/model.rs`, re-exported by `auditor-detectors`)

```rust
use auditor_parser::ir::SourceSpan;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Ordered severity scale. `Ord` is derived so findings sort Critical-first
/// and `--fail-on <level>` threshold checks are simple comparisons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash,
         Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

/// Detector's confidence that the finding is a true positive. Consumed by the
/// LLM Reviewer stage (§5.3) to prioritize validation effort.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord,
         Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Confidence {
    Low,
    Medium,
    High,
    /// Structurally certain (e.g. `tx.origin` used in a require).
    Certain,
}

/// Verdict attached by the (future) LLM Reviewer stage. Defaults to
/// `Unreviewed` so reports are schema-stable before that crate exists.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum ReviewVerdict {
    #[default]
    Unreviewed,
    Confirmed { rationale: String },
    LikelyFalsePositive { rationale: String },
    NeedsHumanReview { rationale: String },
}

/// A single security finding. The unit that flows from detectors,
/// through review, into reports.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Vulnerability {
    /// Content-addressed fingerprint:
    /// blake3(detector_id || file || start_byte || end_byte || source_hash),
    /// hex-encoded. Stable across runs; used for dedup and baselines.
    pub id: String,
    /// Registry identifier of the producing detector, e.g. "MC-REENTRANCY-001".
    pub detector_id: String,
    /// Short human title, e.g. "Reentrancy via external call before state write".
    pub title: String,
    pub severity: Severity,
    pub confidence: Confidence,
    /// Primary code location.
    pub span: SourceSpan,
    /// Supporting locations (e.g. the state write that follows the call).
    pub related_spans: Vec<SourceSpan>,
    /// Exact source excerpt for `span` (verbatim, for report rendering).
    pub code_snippet: String,
    /// What is wrong and why it matters, in complete sentences.
    pub description: String,
    /// Concrete, actionable fix guidance (checks-effects-interactions, use
    /// ReentrancyGuard, add signer constraint, ...).
    pub recommendation: String,
    /// Cross-references: SWC IDs, CWE IDs, Anchor security advisories.
    pub references: Vec<String>,
    /// Populated by the LLM Reviewer stage; `Unreviewed` until then.
    #[serde(default)]
    pub review: ReviewVerdict,
}
```

### 4.3 `AuditReport` (`auditor-reporter/src/model.rs`)

```rust
use camino::Utf8PathBuf;
use indexmap::IndexMap;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Outcome of analyzing one source file (fail-soft: errors are data).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum FileStatus {
    Analyzed { node_count: u32, detectors_run: u32 },
    PartiallyParsed { error: String },   // tree-sitter fallback path
    ParseFailed { error: String },
    Skipped { reason: String },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FileSummary {
    pub path: Utf8PathBuf,
    pub language: String,
    pub source_hash: String,
    pub status: FileStatus,
}

/// Counts per severity, in fixed order. Keys: critical/high/medium/low/info.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SeverityTally {
    pub critical: u32,
    pub high: u32,
    pub medium: u32,
    pub low: u32,
    pub info: u32,
}

/// Top-level audit artifact. Serialized to JSON (canonical) and rendered
/// to Markdown (derived). JSON Schema is generated via `schemars` and
/// published with each release.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AuditReport {
    /// Semver of this report format, independent of tool version.
    pub report_schema_version: String,            // e.g. "1.0.0"
    pub tool_version: String,                     // env!("CARGO_PKG_VERSION")
    /// RFC 3339 UTC timestamp of run start.
    pub generated_at: String,
    /// Audited project root (as given on the CLI).
    pub project_root: Utf8PathBuf,
    /// Git commit of the audited tree, when available.
    pub revision: Option<String>,
    pub files: Vec<FileSummary>,
    pub tally: SeverityTally,
    /// All findings, sorted by (severity desc, file, start_byte, detector_id).
    pub findings: Vec<Vulnerability>,
    /// Detector IDs that ran, mapped to finding counts (stable order).
    pub detectors_executed: IndexMap<String, u32>,
    /// Non-fatal engine diagnostics (panicking detector, timeout, etc.).
    pub diagnostics: Vec<String>,
    /// True iff the LLM Reviewer stage processed this report (§5.3).
    pub reviewed: bool,
}
```

---

## 5. Analysis Pipeline

### 5.1 Stage Sequence

```text
 ┌──────────────┐   ┌──────────────┐   ┌────────────────┐   ┌──────────────────┐   ┌───────────────────┐
 │ 1. Source    │──▶│ 2. AST       │──▶│ 3. Detector    │──▶│ 4. Finding       │──▶│ 5. Report         │
 │   Discovery  │   │   Parsing    │   │   Traversal    │   │   Aggregation    │   │   Generation      │
 │ (core)       │   │ (parser)     │   │ (detectors)    │   │ (core)           │   │ (reporter)        │
 └──────────────┘   └──────────────┘   └────────────────┘   └────────┬─────────┘   └───────────────────┘
                                                                     │      ▲
                                                            ┌────────▼──────┴────────┐
                                                            │ 4b. LLM REVIEW STAGE   │
                                                            │ (auditor-llm-reviewer, │
                                                            │  future crate — §5.3)  │
                                                            └────────────────────────┘
```

**Stage 1 — Source Discovery (`auditor-core`).**
`walkdir` enumerates the project root; files are classified by extension and content
(`.sol` → Solidity; `.rs` containing `#[ink::contract]` or `#[program]` → Ink!/Anchor).
Respects `mintcheck.toml` `exclude` globs (default: `node_modules/`, `target/`,
`lib/forge-std/`, test directories unless `--include-tests`). Output: `Vec<SourceFile>`.

**Stage 2 — AST Parsing (`auditor-parser`).**
Each file is dispatched to its `LanguageFrontend` implementation of:

```rust
pub trait Parser: Send + Sync {
    fn language(&self) -> SourceLanguage;
    fn parse(&self, source: &str, path: &Utf8Path) -> Result<Ast, ParseOutcome>;
}
```

Solidity parse failures fall back to tree-sitter recovery parsing
(`ParseOutcome::Partial`). Output: `Vec<(Ast, FileStatus)>`. Parsing is parallelized
per-file with `rayon::par_iter`.

**Stage 3 — Detector Traversal (`auditor-detectors`).**
The engine performs **one** depth-first walk per AST and multiplexes every enabled
detector over it (detectors are visitors; the tree is walked once, not N times):

```rust
/// Visitor over the normalized IR. Default impls are no-ops, so detectors
/// override only the hooks they need.
pub trait AstVisitor {
    fn enter_node(&mut self, ast: &Ast, node: &ASTNode, cx: &AnalysisContext) {}
    fn exit_node(&mut self, ast: &Ast, node: &ASTNode, cx: &AnalysisContext) {}
    fn enter_function(&mut self, ast: &Ast, node: &ASTNode, cx: &AnalysisContext) {}
    fn exit_function(&mut self, ast: &Ast, node: &ASTNode, cx: &AnalysisContext) {}
    fn visit_call(&mut self, ast: &Ast, node: &ASTNode, cx: &AnalysisContext) {}
    fn visit_assignment(&mut self, ast: &Ast, node: &ASTNode, cx: &AnalysisContext) {}
}

pub trait Detector: AstVisitor + Send {
    /// Stable registry ID, e.g. "MC-REENTRANCY-001".
    fn id(&self) -> &'static str;
    fn default_severity(&self) -> Severity;
    /// Languages this detector applies to; engine skips it otherwise.
    fn languages(&self) -> &'static [SourceLanguage];
    /// Drain findings accumulated during the walk.
    fn take_findings(&mut self) -> Vec<Vulnerability>;
}
```

Before the walk, `AnalysisContext` is built per file: a symbol table of state
variables, an intra-file call graph, and per-function flags (`has_external_call`,
`writes_state_after_call`) that pattern detectors consume. Detector panics are caught
with `catch_unwind`, logged into `AuditReport::diagnostics`, and the run continues.

**Stage 4 — Finding Aggregation (`auditor-core`).**
Per-file finding vectors are merged; duplicates are dropped by `Vulnerability::id`
fingerprint; baseline suppressions (`mintcheck-baseline.json`) and inline
`// mintcheck-ignore: <detector_id>` annotations are applied; findings are sorted
`(severity desc, file, start_byte, detector_id)`; the `SeverityTally` is computed.
Output: a complete, **unreviewed** `AuditReport`.

**Stage 5 — Report Generation (`auditor-reporter`).**
The `AuditReport` is serialized to canonical JSON (`json.rs`, schema published via
`schemars`) and/or rendered to Markdown (`markdown.rs` via the `report.md.tera`
template: executive summary table, severity tally, per-finding sections with code
snippets and recommendations). Exit code is derived from the tally vs. the
`--fail-on` threshold.

### 5.2 Pipeline Driver (in `auditor-core/src/pipeline.rs`)

```rust
pub fn run_audit(config: &AuditConfig) -> anyhow::Result<AuditReport> {
    let files = discover_sources(config)?;                       // Stage 1
    let parsed = parse_all(&files)?;                             // Stage 2 (rayon)
    let raw_findings = run_detectors(&parsed, &config.registry); // Stage 3 (rayon)
    let report = aggregate(raw_findings, &parsed, config)?;      // Stage 4
    let report = config.review_stage.review(report)?;            // Stage 4b (§5.3)
    emit(&report, &config.output)?;                              // Stage 5
    Ok(report)
}
```

### 5.3 LLM Reviewer Hook (Stage 4b — future `auditor-llm-reviewer` crate)

The review stage is a **first-class seam in v0.1**, even though its real
implementation ships later. `auditor-core` defines the trait and always invokes it
between aggregation and report generation:

```rust
// auditor-core/src/review.rs
pub trait ReviewStage: Send + Sync {
    /// Receives the fully aggregated (unreviewed) report; returns it with
    /// `Vulnerability::review` verdicts and `AuditReport::reviewed` populated.
    /// MUST NOT add or delete findings — only annotate them — so static
    /// results remain auditable even after review.
    fn review(&self, report: AuditReport) -> anyhow::Result<AuditReport>;
}

/// v0.1 default: identity pass-through, leaves every finding `Unreviewed`.
pub struct NoopReview;
impl ReviewStage for NoopReview {
    fn review(&self, report: AuditReport) -> anyhow::Result<AuditReport> {
        Ok(report)
    }
}
```

The future `auditor-llm-reviewer` crate (workspace member, enabled via the
`llm-review` cargo feature on `auditor-core` and `--review` on the CLI) will:

1. **Batch findings by file**, attach the full function source surrounding each
   `Vulnerability::span` (the snippet alone is insufficient context for validation).
2. **Validate each finding** with an LLM (Claude via the `anthropic` REST API over
   `reqwest`/`tokio`), prioritizing `Confidence::Low`/`Medium` findings where static
   analysis false-positive rates are highest.
3. **Write back verdicts only**: `ReviewVerdict::Confirmed`,
   `LikelyFalsePositive`, or `NeedsHumanReview`, each with a rationale string. The
   contract that the reviewer may annotate but never add/remove findings is what
   keeps reports reproducible and the static layer independently trustworthy.
4. **Degrade gracefully**: API failure or timeout leaves findings `Unreviewed` and
   appends a diagnostic; the audit still completes.

Because `ReviewVerdict` and `AuditReport::reviewed` already exist in the v0.1
schema (defaulting to `Unreviewed`/`false`), enabling the LLM stage later is **not**
a breaking schema change for downstream consumers.

### 5.4 Concurrency & Failure Model

| Concern | Decision |
|---|---|
| Parallelism | Per-file via `rayon`; detectors for one file run sequentially over a single walk (they're cheap; the walk dominates). |
| Detector panic | `catch_unwind` → diagnostic entry; other detectors unaffected. |
| Parse failure | tree-sitter partial parse → `FileStatus::PartiallyParsed`; detectors run on whatever IR was recovered. |
| Determinism | `indexmap` everywhere ordering leaks into output; final sort in Stage 4; blake3 fingerprints; no wall-clock data except `generated_at`. |
| Exit codes | `0` no findings ≥ fail-level · `1` findings ≥ fail-level · `2` engine error. |

---

## 6. Initial Detector Set (v0.1)

| Detector ID | Severity | Languages | Reference |
|---|---|---|---|
| `MC-REENTRANCY-001` — external call before state write | High | Solidity | SWC-107 |
| `MC-UNCHECKED-CALL-001` — unchecked low-level call return | Medium | Solidity | SWC-104 |
| `MC-OVERFLOW-001` — arithmetic in `unchecked {}` / pragma <0.8 | High | Solidity | SWC-101 |
| `MC-TXORIGIN-001` — `tx.origin` authorization | High | Solidity | SWC-115 |
| `MC-DELEGATECALL-001` — delegatecall to user-influenced target | Critical | Solidity | SWC-112 |
| `MC-TIMESTAMP-001` — block timestamp in critical logic | Low | Solidity | SWC-116 |
| `MC-ACCESS-001` — state-mutating external fn without auth modifier/constraint | Medium | All | SWC-105 |
| `MC-ANCHOR-OWNER-001` — `AccountInfo` used without owner check | Critical | Anchor | Anchor docs: account validation |
| `MC-INK-SELECTOR-001` — colliding/missing ink! selectors | Medium | Ink! | ink! docs: selectors |

Each detector ships with positive and negative fixtures under `fixtures/` and a
snapshot test (`insta`) of its findings.

---

## 7. Milestones

| Version | Scope |
|---|---|
| **0.1** | Workspace scaffolding, Solidity frontend (solang + tree-sitter fallback), IR, visitor engine, 7 Solidity detectors, JSON + Markdown reports, `NoopReview` seam, CI (fmt, clippy, deny, snapshot tests). |
| **0.2** | Rust frontends (Ink!, Anchor), cross-language detectors, baseline/suppression files, JSON Schema publication. |
| **0.3** | `auditor-llm-reviewer` crate (Stage 4b live), `cargo expand` integration for post-macro analysis, SARIF output for GitHub code scanning. |
| **0.4** | Inter-file call graph, inheritance-aware analysis, taint tracking for `DELEGATECALL`/`CrossProgram` targets. |
