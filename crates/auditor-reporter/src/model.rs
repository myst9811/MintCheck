//! The aggregated audit report model (spec §4.3). Owned here so the
//! reporter can serialize it without inverting the dependency graph;
//! `auditor-core` re-exports it under `auditor_core::report`.

use std::time::{SystemTime, UNIX_EPOCH};

use auditor_detectors::finding::Vulnerability;
use serde::{Deserialize, Serialize};

/// Aggregated result of one audit run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuditReport {
    /// Project root or file that was audited.
    pub target_path: String,
    pub vulnerabilities: Vec<Vulnerability>,
    /// Seconds since the Unix epoch at run start.
    pub timestamp: u64,
}

impl AuditReport {
    /// Creates an empty report with an explicit timestamp (deterministic;
    /// preferred in tests and reproducible pipelines).
    pub fn new(target_path: impl Into<String>, timestamp: u64) -> Self {
        Self {
            target_path: target_path.into(),
            vulnerabilities: Vec::new(),
            timestamp,
        }
    }

    /// Creates an empty report stamped with the current wall-clock time.
    pub fn started_now(target_path: impl Into<String>) -> Self {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or_default();
        Self::new(target_path, timestamp)
    }

    pub fn add_vulnerability(&mut self, vulnerability: Vulnerability) {
        self.vulnerabilities.push(vulnerability);
    }

    /// Sorts findings most-severe-first; ties keep a stable, reproducible
    /// order by file path, then span start, then detector id.
    pub fn sort_by_severity(&mut self) {
        self.vulnerabilities.sort_by(|a, b| {
            a.severity
                .cmp(&b.severity)
                .then_with(|| a.file_path.cmp(&b.file_path))
                .then_with(|| a.span.start.cmp(&b.span.start))
                .then_with(|| a.id.cmp(&b.id))
        });
    }
}
