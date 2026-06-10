//! End-to-end integration test: raw source text through parse → detect →
//! aggregate, exercising only the public API of `auditor-core`.

use auditor_core::{audit_source, Severity};
use auditor_parser::parser::ParseError;

const VULNERABLE: &str = "contract Phish {\n    address owner;\n\n    function auth() public {\n        require(tx.origin == owner);\n        count = count + 1;\n    }\n}\n";

#[test]
fn audits_vulnerable_contract_end_to_end() {
    let report =
        audit_source("contracts/Phish.sol", VULNERABLE).expect("valid source must audit cleanly");

    assert_eq!(report.target_path, "contracts/Phish.sol");
    assert_eq!(report.vulnerabilities.len(), 1);

    let v = &report.vulnerabilities[0];
    assert_eq!(v.id, "SWC-115");
    assert_eq!(v.severity, Severity::High);
    assert_eq!(v.file_path, "contracts/Phish.sol");

    // The span must point exactly at the offending statement.
    let stmt = "require(tx.origin == owner);";
    let start = VULNERABLE.find(stmt).expect("stmt present");
    assert_eq!(v.span.start, start);
    assert_eq!(v.span.end, start + stmt.len());
    assert_eq!(&VULNERABLE[v.span.start..v.span.end], stmt);
}

#[test]
fn clean_contract_yields_empty_report() {
    let safe = "contract Safe {\n    uint256 count;\n\n    function bump() public {\n        count = count + 1;\n    }\n}\n";
    let report = audit_source("contracts/Safe.sol", safe).expect("valid source must audit");
    assert!(report.vulnerabilities.is_empty());
    assert_eq!(report.target_path, "contracts/Safe.sol");
}

#[test]
fn parse_failures_propagate_to_the_caller() {
    assert_eq!(
        audit_source("contracts/Empty.sol", "   "),
        Err(ParseError::EmptySource)
    );
    assert!(matches!(
        audit_source("contracts/Broken.sol", "contract X { function f() {"),
        Err(ParseError::SyntaxError(_))
    ));
}
