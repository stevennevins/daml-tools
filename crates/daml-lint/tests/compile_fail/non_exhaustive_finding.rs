use daml_lint::detector::{Finding, FindingLocation, Severity};
use std::path::PathBuf;

fn main() {
    let _finding = Finding {
        detector: String::from("rule"),
        severity: Severity::High,
        file: PathBuf::from("Test.daml"),
        line: 1,
        column: 1,
        message: String::from("msg"),
        evidence: String::from("evidence"),
    };
    let _ = FindingLocation {
        file: PathBuf::from("Test.daml"),
        line: 1,
        column: 1,
    };
}
