//! Integration tests for reporter public API contracts.

#![allow(clippy::unwrap_used)]

use daml_lint::detector::{Finding, FindingLocation, Severity};
use daml_lint::parser::ParseDiagnosticCategory;
use daml_lint::reporter::{self, OutputFormat, ParseError};

fn parse_err() -> ParseError {
    ParseError::new(
        "Bad.daml",
        3,
        5,
        Some(11),
        "unterminated string literal",
        ParseDiagnosticCategory::LexicalError,
    )
}

fn finding(column: usize, severity: Severity) -> Finding {
    Finding::new(
        "test-rule",
        severity,
        FindingLocation::new("Test.daml", 3, column),
        "finding message",
        "evidence",
    )
}

#[test]
fn output_format_parses_known_values_and_reports_unknown_with_display_text() {
    assert_eq!(
        "sarif".parse::<OutputFormat>().unwrap(),
        OutputFormat::Sarif
    );
    assert_eq!(
        "MARKDOWN".parse::<OutputFormat>().unwrap(),
        OutputFormat::Markdown
    );
    assert_eq!("JsOn".parse::<OutputFormat>().unwrap(), OutputFormat::Json);

    let err = "yaml".parse::<OutputFormat>().unwrap_err();
    assert_eq!(err.to_string(), "invalid output format: yaml");
}

#[test]
fn json_and_sarif_clean_output_has_no_parse_errors() {
    let json: serde_json::Value =
        serde_json::from_str(&reporter::format_findings(&[], &[], OutputFormat::Json)).unwrap();
    assert_eq!(json["parseErrors"].as_array().unwrap().len(), 0);
    assert_eq!(json["summary"]["parseErrors"], 0);

    let sarif: serde_json::Value =
        serde_json::from_str(&reporter::format_findings(&[], &[], OutputFormat::Sarif)).unwrap();
    assert_eq!(
        sarif["runs"][0]["invocations"][0]["executionSuccessful"],
        true
    );
}

#[test]
fn json_exposes_parse_errors() {
    let json: serde_json::Value = serde_json::from_str(&reporter::format_findings(
        &[],
        &[parse_err()],
        OutputFormat::Json,
    ))
    .unwrap();
    assert_eq!(json["parseErrors"].as_array().unwrap().len(), 1);
    assert_eq!(json["parseErrors"][0]["line"], 3);
    assert_eq!(json["summary"]["parseErrors"], 1);
}

#[test]
fn json_carries_category_and_end_column() {
    let json: serde_json::Value = serde_json::from_str(&reporter::format_findings(
        &[],
        &[parse_err()],
        OutputFormat::Json,
    ))
    .unwrap();
    assert_eq!(json["parseErrors"][0]["category"], "lexical-error");
    assert_eq!(json["parseErrors"][0]["endColumn"], 11);
}

#[test]
fn sarif_notification_carries_category_and_end_column() {
    let sarif: serde_json::Value = serde_json::from_str(&reporter::format_findings(
        &[],
        &[parse_err()],
        OutputFormat::Sarif,
    ))
    .unwrap();
    let note = &sarif["runs"][0]["invocations"][0]["toolExecutionNotifications"][0];
    assert_eq!(note["properties"]["category"], "lexical-error");
    assert_eq!(
        note["locations"][0]["physicalLocation"]["region"]["endColumn"],
        11
    );
}

#[test]
fn sarif_marks_run_unsuccessful_on_parse_errors() {
    let sarif: serde_json::Value = serde_json::from_str(&reporter::format_findings(
        &[],
        &[parse_err()],
        OutputFormat::Sarif,
    ))
    .unwrap();
    let inv = &sarif["runs"][0]["invocations"][0];
    assert_eq!(inv["executionSuccessful"], false);
    assert_eq!(
        inv["toolExecutionNotifications"].as_array().unwrap().len(),
        1
    );
    assert_eq!(sarif["runs"][0]["results"].as_array().unwrap().len(), 0);
}

#[test]
fn sarif_driver_information_uri_matches_crate_repository_not_placeholder() {
    let sarif: serde_json::Value =
        serde_json::from_str(&reporter::format_findings(&[], &[], OutputFormat::Sarif)).unwrap();
    let uri = sarif["runs"][0]["tool"]["driver"]["informationUri"]
        .as_str()
        .unwrap();
    assert_eq!(uri, env!("CARGO_PKG_REPOSITORY"));
    assert!(!uri.contains("example/daml-lint"));
}

#[test]
fn exit_code_uses_named_threshold_semantics() {
    assert_eq!(
        reporter::exit_code(&[finding(1, Severity::Critical)], Severity::High),
        1
    );
    assert_eq!(
        reporter::exit_code(&[finding(2, Severity::High)], Severity::High),
        1
    );
    assert_eq!(
        reporter::exit_code(&[finding(3, Severity::Medium)], Severity::High),
        0
    );
    assert_eq!(
        reporter::exit_code(&[finding(4, Severity::Info)], Severity::High),
        0
    );
    assert_eq!(
        reporter::exit_code(&[finding(5, Severity::Low)], Severity::Info),
        1
    );
}
