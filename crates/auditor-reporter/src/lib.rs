//! JSON / Markdown serialization of audit findings (pipeline Stage 5).
//!
//! Depends only on `auditor-detectors` (see `PROJECT_SPEC.md` §2.2).

pub mod model;

use auditor_detectors::Severity;

pub use model::AuditReport;

const SEVERITY_LEVELS: [Severity; 5] = [
    Severity::Critical,
    Severity::High,
    Severity::Medium,
    Severity::Low,
    Severity::Info,
];

/// Serializes the report to pretty-printed, machine-consumable JSON.
pub fn report_to_json(report: &AuditReport) -> Result<String, serde_json::Error> {
    serde_json::to_string_pretty(report)
}

/// Renders the report as scannable Markdown: title, severity summary table,
/// then one detailed section per finding in the report's order.
pub fn report_to_markdown(report: &AuditReport) -> String {
    let mut out = String::new();
    out.push_str("# Smart Contract Audit Report\n\n");
    out.push_str(&format!("**Target:** `{}`  \n", report.target_path));
    out.push_str(&format!("**Generated (unix):** {}  \n", report.timestamp));
    out.push_str(&format!(
        "**Total findings:** {}\n\n",
        report.vulnerabilities.len()
    ));

    out.push_str("## Summary\n\n| Severity | Count |\n|----------|-------|\n");
    for level in SEVERITY_LEVELS {
        let count = report
            .vulnerabilities
            .iter()
            .filter(|v| v.severity == level)
            .count();
        out.push_str(&format!("| {level} | {count} |\n"));
    }

    out.push_str("\n## Findings\n\n");
    if report.vulnerabilities.is_empty() {
        out.push_str("No vulnerabilities were identified.\n");
        return out;
    }
    for (i, v) in report.vulnerabilities.iter().enumerate() {
        out.push_str(&format!("### {}. {} [{}]\n\n", i + 1, v.title, v.severity));
        out.push_str(&format!(
            "- **ID:** {}\n- **Severity:** {}\n- **File:** `{}`\n- **Span:** bytes {}..{}\n\n",
            v.id, v.severity, v.file_path, v.span.start, v.span.end
        ));
        out.push_str(&format!("{}\n\n", v.description));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use auditor_detectors::finding::{SourceSpan, Vulnerability};

    fn mock_report() -> AuditReport {
        let mut report = AuditReport::new("contracts/Phish.sol", 1_750_000_000);
        report.add_vulnerability(Vulnerability {
            id: "SWC-116".to_owned(),
            title: "Block timestamp dependence".to_owned(),
            description: "Timestamp used in critical logic.".to_owned(),
            severity: Severity::Low,
            span: SourceSpan::new(120, 145),
            file_path: "contracts/Phish.sol".to_owned(),
        });
        report.add_vulnerability(Vulnerability {
            id: "SWC-115".to_owned(),
            title: "Authorization via tx.origin".to_owned(),
            description: "Use msg.sender for authorization.".to_owned(),
            severity: Severity::High,
            span: SourceSpan::new(57, 85),
            file_path: "contracts/Phish.sol".to_owned(),
        });
        report.sort_by_severity();
        report
    }

    #[test]
    fn json_output_is_valid_and_round_trips() {
        let report = mock_report();
        let json = report_to_json(&report).expect("serialization must succeed");

        assert!(json.contains("contracts/Phish.sol"));
        assert!(json.contains("SWC-115"));
        assert!(json.contains("\"vulnerabilities\""));

        let restored: AuditReport = serde_json::from_str(&json).expect("output must be valid JSON");
        assert_eq!(restored, report);
    }

    #[test]
    fn markdown_contains_title_summary_table_and_finding_sections() {
        let md = report_to_markdown(&mock_report());

        assert!(md.starts_with("# Smart Contract Audit Report\n"));
        assert!(md.contains("| Severity | Count |"));
        assert!(md.contains("| Critical | 0 |"));
        assert!(md.contains("| High | 1 |"));
        assert!(md.contains("| Low | 1 |"));

        // Findings are severity-sorted: High first, with full detail rows.
        assert!(md.contains("### 1. Authorization via tx.origin [High]"));
        assert!(md.contains("- **ID:** SWC-115"));
        assert!(md.contains("- **File:** `contracts/Phish.sol`"));
        assert!(md.contains("- **Span:** bytes 57..85"));
        assert!(md.contains("### 2. Block timestamp dependence [Low]"));
        assert!(md.contains("- **Span:** bytes 120..145"));
    }

    #[test]
    fn markdown_for_clean_report_says_so() {
        let md = report_to_markdown(&AuditReport::new("contracts/Safe.sol", 1));
        assert!(md.contains("No vulnerabilities were identified."));
        assert!(md.contains("| Critical | 0 |"));
    }
}
