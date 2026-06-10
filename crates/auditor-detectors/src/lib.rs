//! Static analysis engine: visitor-pattern detectors over the normalized IR.
//!
//! Depends only on `auditor-parser` (see `PROJECT_SPEC.md` §2.2).

pub mod finding;
pub mod tx_origin;

use auditor_parser::ir::ASTNode;

pub use finding::{Severity, Vulnerability};
pub use tx_origin::TxOriginDetector;

/// A modular static-analysis routine. Each detector scans a normalized AST
/// and reports zero or more findings; detectors are stateless across files
/// and must be deterministic for identical inputs.
pub trait Detector {
    /// Human-readable detector name, e.g. "Authorization via tx.origin".
    fn name(&self) -> &'static str;

    /// Stable registry identifier, e.g. "SWC-115".
    fn id(&self) -> &'static str;

    /// Severity assigned to every finding this detector emits.
    fn severity(&self) -> Severity;

    /// Scans the subtree rooted at `node` and returns all findings,
    /// attributing each to `file_path`.
    fn check(&self, node: &ASTNode, file_path: &str) -> Vec<Vulnerability>;
}
