//! `mintcheck` CLI entry point: audits one smart contract file and prints
//! the Markdown audit report to stdout. Errors go to stderr with exit code 1.

use std::env;
use std::fs;
use std::process::ExitCode;

use auditor_core::audit_source;
use auditor_reporter::report_to_markdown;

const USAGE: &str = "usage: mintcheck <contract-file>";

/// Pure CLI driver: maps argv to the rendered report or an error message.
/// Kept free of process concerns so every branch is unit-testable.
fn run(args: &[String]) -> Result<String, String> {
    let [_, file_path] = args else {
        return Err(USAGE.to_owned());
    };
    let source = fs::read_to_string(file_path)
        .map_err(|e| format!("mintcheck: cannot read '{file_path}': {e}"))?;
    let report = audit_source(file_path, &source)
        .map_err(|e| format!("mintcheck: audit failed for '{file_path}': {e}"))?;
    Ok(report_to_markdown(&report))
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();
    match run(&args) {
        Ok(markdown) => {
            println!("{markdown}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(1)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn missing_argument_yields_usage() {
        assert_eq!(run(&args(&["mintcheck"])), Err(USAGE.to_owned()));
        assert_eq!(
            run(&args(&["mintcheck", "a.sol", "b.sol"])),
            Err(USAGE.to_owned())
        );
    }

    #[test]
    fn unreadable_file_yields_clean_error() {
        let err = run(&args(&["mintcheck", "/nonexistent/dir/x.sol"]))
            .expect_err("missing file must fail");
        assert!(err.contains("cannot read"));
        assert!(err.contains("/nonexistent/dir/x.sol"));
    }

    #[test]
    fn parse_failure_yields_clean_error() {
        let path = env::temp_dir().join("mintcheck_cli_blank.sol");
        fs::write(&path, "   \n").expect("temp file must be writable");
        let err = run(&args(&["mintcheck", &path.to_string_lossy()]))
            .expect_err("blank source must fail the audit");
        fs::remove_file(&path).ok();
        assert!(err.contains("audit failed"));
        assert!(err.contains("source text is empty"));
    }

    #[test]
    fn audits_real_file_and_renders_markdown() {
        let path = env::temp_dir().join("mintcheck_cli_phish.sol");
        fs::write(
            &path,
            "contract Phish { function auth() public { require(tx.origin == owner); } }",
        )
        .expect("temp file must be writable");
        let md =
            run(&args(&["mintcheck", &path.to_string_lossy()])).expect("valid contract must audit");
        fs::remove_file(&path).ok();

        assert!(md.starts_with("# Smart Contract Audit Report"));
        assert!(md.contains("SWC-115"));
        assert!(md.contains("| High | 1 |"));
    }
}
