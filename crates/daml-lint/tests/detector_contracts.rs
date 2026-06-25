//! Integration tests for detector public API contracts.

#![allow(clippy::unwrap_used)]

use daml_lint::detector::{Finding, FindingLocation, Severity};
use std::path::PathBuf;

#[test]
fn finding_new_populates_public_fields_from_named_location() {
    let finding = Finding::new(
        "named-rule",
        Severity::Medium,
        FindingLocation::new("src/Main.daml", 7, 4),
        "expected a check",
        "amount = x",
    );
    assert_eq!(finding.detector, "named-rule");
    assert_eq!(finding.severity, Severity::Medium);
    assert_eq!(finding.file, PathBuf::from("src/Main.daml"));
    assert_eq!(finding.line, 7);
    assert_eq!(finding.column, 4);
    assert_eq!(finding.message, "expected a check");
    assert_eq!(finding.evidence, "amount = x");
}

#[test]
fn severity_parse_error_reports_invalid_value_and_allowed_levels() {
    let err = "bogus".parse::<Severity>().unwrap_err();
    assert_eq!(err.value(), "bogus");
    assert!(err.to_string().contains("bogus"));
    assert!(err.to_string().contains("critical|high|medium|low|info"));
}

#[test]
fn severity_rank_is_explicitly_risk_ordered() {
    assert!(Severity::Critical.rank() > Severity::High.rank());
    assert!(Severity::High.rank() > Severity::Medium.rank());
    assert!(Severity::Medium.rank() > Severity::Low.rank());
    assert!(Severity::Low.rank() > Severity::Info.rank());
    assert!(Severity::Critical.meets_or_exceeds(Severity::High));
    assert!(Severity::High.meets_or_exceeds(Severity::High));
    assert!(!Severity::Medium.meets_or_exceeds(Severity::High));
    assert!(!Severity::Low.meets_or_exceeds(Severity::High));
    assert!(!Severity::Info.meets_or_exceeds(Severity::High));
}
