//! Canonical finding types: the severity scale and individual security
//! findings. Defined here (not in `auditor-core`) so detectors can emit
//! findings without inverting the workspace dependency graph; core
//! re-exports these under `auditor_core::report` for compatibility.

use std::fmt;

use serde::{Deserialize, Serialize};

// Re-exported (not merely imported) so downstream crates such as
// auditor-reporter can name the span type findings carry without taking
// a direct auditor-parser dependency.
pub use auditor_parser::ir::SourceSpan;

/// Severity scale for findings. Declaration order drives `Ord`: `Critical`
/// is the smallest variant so an ascending sort lists it first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum Severity {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let label = match self {
            Severity::Critical => "Critical",
            Severity::High => "High",
            Severity::Medium => "Medium",
            Severity::Low => "Low",
            Severity::Info => "Info",
        };
        f.write_str(label)
    }
}

/// A single security finding produced by a detector.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vulnerability {
    /// Detector or registry identifier, e.g. "SWC-101" or "MC-REENTRANCY-001".
    pub id: String,
    pub title: String,
    pub description: String,
    pub severity: Severity,
    /// Byte range of the offending code, in the normalized IR's span type.
    pub span: SourceSpan,
    pub file_path: String,
}
