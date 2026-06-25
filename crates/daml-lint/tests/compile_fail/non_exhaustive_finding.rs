use daml_lint::detector::{Finding, FindingLocation, Severity};
use daml_syntax::{CharColumn, LineNumber};
use std::path::PathBuf;

fn main() {
    let _finding = Finding {
        detector: String::from("rule"),
        severity: Severity::High,
        file: PathBuf::from("Test.daml"),
        line: LineNumber::new(1),
        column: CharColumn::new(1),
        message: String::from("msg"),
        evidence: String::from("evidence"),
    };
    let _ = FindingLocation {
        file: PathBuf::from("Test.daml"),
        line: LineNumber::new(1),
        column: CharColumn::new(1),
    };
}
