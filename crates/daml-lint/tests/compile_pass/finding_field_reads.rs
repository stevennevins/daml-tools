use daml_lint::detector::{Finding, FindingLocation, Severity};

fn main() {
    let finding = Finding::new(
        "rule",
        Severity::High,
        FindingLocation::new("Test.daml", 3, 4),
        "message",
        "evidence",
    );
    assert_eq!(finding.detector, "rule");
    assert_eq!(finding.severity, Severity::High);
    assert_eq!(finding.line, 3);
    assert_eq!(finding.column, 4);
}
