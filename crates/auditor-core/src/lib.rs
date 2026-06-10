//! Central orchestration engine: pipeline driver, configuration, and the
//! sole I/O boundary of the MintCheck workspace (see `PROJECT_SPEC.md` §2.2).

pub mod pipeline;
pub mod report;

pub use pipeline::audit_source;
pub use report::{AuditReport, Severity, Vulnerability};
