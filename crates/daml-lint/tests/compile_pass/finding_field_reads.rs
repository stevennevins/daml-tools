use daml_lint::detector::{Finding, FindingLocation, Severity};
use daml_syntax::{CharColumn, LineNumber};

fn main() {
    let finding = Finding::new(
        "rule",
        Severity::High,
        FindingLocation::new("Test.daml", LineNumber::new(3), CharColumn::new(4)),
        "message",
        "evidence",
    );
    assert_eq!(finding.detector, "rule");
    assert_eq!(finding.severity, Severity::High);
    assert_eq!(finding.line, LineNumber::new(3));
    assert_eq!(finding.column, CharColumn::new(4));
}
