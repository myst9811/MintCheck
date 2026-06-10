//! Audit report aggregation. The finding types (`Severity`,
//! `Vulnerability`) are owned by `auditor-detectors` and re-exported here,
//! so existing `auditor_core::report::*` paths keep working.

use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

pub use auditor_detectors::finding::{Severity, Vulnerability};

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

#[cfg(test)]
mod tests {
    use super::*;
    use auditor_parser::ir::SourceSpan;

    fn vuln(id: &str, severity: Severity, start: usize) -> Vulnerability {
        Vulnerability {
            id: id.to_owned(),
            title: format!("{id} finding"),
            description: "details".to_owned(),
            severity,
            span: SourceSpan::new(start, start + 10),
            file_path: "contracts/Vault.sol".to_owned(),
        }
    }

    #[test]
    fn severity_orders_critical_above_info() {
        assert!(Severity::Critical < Severity::High);
        assert!(Severity::High < Severity::Medium);
        assert!(Severity::Medium < Severity::Low);
        assert!(Severity::Low < Severity::Info);

        let mut levels = [Severity::Info, Severity::Critical, Severity::Medium];
        levels.sort();
        assert_eq!(
            levels,
            [Severity::Critical, Severity::Medium, Severity::Info]
        );
    }

    #[test]
    fn severity_displays_cleanly() {
        assert_eq!(Severity::Critical.to_string(), "Critical");
        assert_eq!(Severity::Info.to_string(), "Info");
        assert_eq!(format!("[{}]", Severity::High), "[High]");
    }

    #[test]
    fn report_accumulates_and_sorts_by_severity() {
        let mut report = AuditReport::new("contracts/", 1_750_000_000);
        report.add_vulnerability(vuln("SWC-116", Severity::Low, 300));
        report.add_vulnerability(vuln("SWC-107", Severity::High, 120));
        report.add_vulnerability(vuln("SWC-112", Severity::Critical, 80));
        report.add_vulnerability(vuln("SWC-101", Severity::High, 40));
        assert_eq!(report.vulnerabilities.len(), 4);

        report.sort_by_severity();
        let order: Vec<&str> = report
            .vulnerabilities
            .iter()
            .map(|v| v.id.as_str())
            .collect();
        // Critical first; the two High findings tie-break by span start.
        assert_eq!(order, ["SWC-112", "SWC-101", "SWC-107", "SWC-116"]);
    }

    #[test]
    fn report_serializes_to_json_and_back() {
        let mut report = AuditReport::new("contracts/Vault.sol", 1_750_000_000);
        report.add_vulnerability(vuln("SWC-107", Severity::High, 120));

        let json = serde_json::to_string_pretty(&report).expect("serialization must succeed");
        assert!(json.contains("\"target_path\": \"contracts/Vault.sol\""));
        assert!(json.contains("\"severity\": \"High\""));
        assert!(json.contains("\"start\": 120"));

        let restored: AuditReport =
            serde_json::from_str(&json).expect("deserialization must succeed");
        assert_eq!(restored, report);
        assert_eq!(restored.vulnerabilities[0].span, SourceSpan::new(120, 130));
    }

    #[test]
    fn started_now_stamps_a_plausible_timestamp() {
        let report = AuditReport::started_now("contracts/");
        // 2025-01-01T00:00:00Z — any sane clock is past this.
        assert!(report.timestamp > 1_735_689_600);
    }
}
