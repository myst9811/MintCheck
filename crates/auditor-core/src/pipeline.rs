//! Central orchestration: drives one source file through the full audit
//! lifecycle — Parsing (Stage 2) → Detector Traversal (Stage 3) → Finding
//! Aggregation (Stage 4). Report serialization (Stage 5) and the LLM review
//! seam (Stage 4b) attach downstream of the report returned here.

use auditor_detectors::{Detector, TxOriginDetector};
use auditor_parser::parser::{ParseError, ParserEngine};

use crate::report::AuditReport;

/// The active detector suite for one file's audit. Detectors that need the
/// raw source (to resolve IR spans back to text) receive it here.
fn detector_suite(source: &str) -> Vec<Box<dyn Detector>> {
    vec![Box::new(TxOriginDetector::new(source))]
}

/// Runs the complete audit pipeline over an in-memory source file and
/// returns the aggregated, severity-sorted report. Parse failures propagate
/// to the caller; detector results are deterministic for identical inputs.
pub fn audit_source(file_path: &str, source: &str) -> Result<AuditReport, ParseError> {
    let ast = ParserEngine::new().parse_solidity(source)?;
    let mut report = AuditReport::started_now(file_path);
    for detector in detector_suite(source) {
        for finding in detector.check(&ast, file_path) {
            report.add_vulnerability(finding);
        }
    }
    report.sort_by_severity();
    Ok(report)
}
