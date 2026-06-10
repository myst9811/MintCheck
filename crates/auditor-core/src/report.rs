//! Compatibility shim. The finding types (`Severity`, `Vulnerability`) are
//! owned by `auditor-detectors`; the report model (`AuditReport`) is owned
//! by `auditor-reporter` (spec §4.3). Re-exported here so established
//! `auditor_core::report::*` paths keep working.

pub use auditor_detectors::finding::{Severity, Vulnerability};
pub use auditor_reporter::model::AuditReport;

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
