//! Integration tests for detector public API contracts.

#![allow(clippy::unwrap_used)]

use daml_lint::detector::{Finding, FindingLocation, Severity};
use daml_syntax::{CharColumn, LineNumber};
use std::path::PathBuf;

fn finding() -> Finding {
    Finding::new(
        "unused-foo",
        Severity::High,
        FindingLocation::new("foo.daml", LineNumber::new(10), CharColumn::new(4)),
        "consider removing",
        "foo",
    )
}

#[test]
fn finding_is_comparable() {
    assert_eq!(finding(), finding());
}

#[test]
fn finding_new_populates_public_fields_from_named_location() {
    let finding = Finding::new(
        "named-rule",
        Severity::Medium,
        FindingLocation::new("src/Main.daml", LineNumber::new(7), CharColumn::new(4)),
        "expected a check",
        "amount = x",
    );
    assert_eq!(finding.detector, "named-rule");
    assert_eq!(finding.severity, Severity::Medium);
    assert_eq!(finding.file, PathBuf::from("src/Main.daml"));
    assert_eq!(finding.line, LineNumber::new(7));
    assert_eq!(finding.column, CharColumn::new(4));
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
fn severity_from_str_preserves_meaningful_errors() {
    assert_eq!("high".parse::<Severity>(), Ok(Severity::High));
    let err = "bogus".parse::<Severity>().unwrap_err();
    assert_eq!(err.value(), "bogus");
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

#[test]
fn findings_are_sorted_by_explicit_severity_ranking() {
    let mut findings = [
        Finding::new(
            "rule-medium",
            Severity::Medium,
            FindingLocation::new("b.daml", LineNumber::new(10), CharColumn::new(4)),
            "medium finding",
            "evidence",
        ),
        Finding::new(
            "rule-critical",
            Severity::Critical,
            FindingLocation::new("a.daml", LineNumber::new(3), CharColumn::new(1)),
            "critical finding",
            "evidence",
        ),
        Finding::new(
            "rule-high",
            Severity::High,
            FindingLocation::new("a.daml", LineNumber::new(5), CharColumn::new(2)),
            "high finding",
            "evidence",
        ),
    ];

    findings.sort_by(|a, b| {
        b.severity
            .rank()
            .cmp(&a.severity.rank())
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
    });

    assert_eq!(findings[0].severity, Severity::Critical);
    assert_eq!(findings[1].severity, Severity::High);
    assert_eq!(findings[2].severity, Severity::Medium);
}

#[cfg(feature = "js-runtime")]
#[test]
fn returns_none_when_builtin_detector_names_are_unique() {
    use daml_lint::detector::find_duplicate_detector_name;
    use daml_lint::detectors::create_builtin_detectors;

    assert_eq!(
        find_duplicate_detector_name(&create_builtin_detectors()),
        None
    );
}

#[cfg(feature = "js-runtime")]
#[test]
fn returns_duplicate_builtin_detector_name() {
    use daml_lint::detector::find_duplicate_detector_name;
    use daml_lint::detectors::create_builtin_detectors;

    let mut doubled = create_builtin_detectors();
    doubled.extend(create_builtin_detectors());
    assert!(find_duplicate_detector_name(&doubled).is_some());
}
