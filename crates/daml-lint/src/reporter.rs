use crate::detector::{Finding, Severity};
use crate::parser::ParseDiagnosticCategory;
use serde::Serialize;
use serde_json::json;
use std::error::Error;
use std::fmt::Write as _;
use std::fmt::{self, Display};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Sarif,
    Markdown,
    Json,
}

/// Error returned when parsing an unsupported output format value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputFormatParseError {
    value: String,
}

impl OutputFormatParseError {
    fn new(value: impl Into<String>) -> Self {
        Self {
            value: value.into(),
        }
    }
}

impl Display for OutputFormatParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid output format: {}", self.value)
    }
}

impl Error for OutputFormatParseError {}

impl std::str::FromStr for OutputFormat {
    type Err = OutputFormatParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "sarif" => Ok(Self::Sarif),
            "markdown" | "md" => Ok(Self::Markdown),
            "json" => Ok(Self::Json),
            _ => Err(OutputFormatParseError::new(s)),
        }
    }
}

/// A parse/lex diagnostic surfaced to the caller alongside findings. A file
/// with parse errors is NOT clean, even when no findings were produced.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParseError {
    pub file: String,
    pub line: usize,
    pub column: usize,
    /// End column when the diagnostic span sits on one line; `None` otherwise.
    pub end_column: Option<usize>,
    pub message: String,
    /// Recovery category tag (e.g. `skipped-declaration`, `unsupported-syntax`).
    pub category: ParseDiagnosticCategory,
}

impl ParseError {
    /// Construct a parse error without relying on struct literal syntax.
    #[must_use]
    pub fn new(
        file: impl Into<String>,
        line: usize,
        column: usize,
        end_column: Option<usize>,
        message: impl Into<String>,
        category: ParseDiagnosticCategory,
    ) -> Self {
        Self {
            file: file.into(),
            line,
            column,
            end_column,
            message: message.into(),
            category,
        }
    }
}

/// Format lint `findings` and parse errors into a stable output text format.
#[must_use]
pub fn format_findings(
    findings: &[Finding],
    parse_errors: &[ParseError],
    format: OutputFormat,
) -> String {
    match format {
        OutputFormat::Sarif => format_sarif(findings, parse_errors),
        OutputFormat::Markdown => format_markdown(findings, parse_errors),
        OutputFormat::Json => format_json(findings, parse_errors),
    }
}

fn format_sarif(findings: &[Finding], parse_errors: &[ParseError]) -> String {
    let results: Vec<serde_json::Value> = findings
        .iter()
        .map(|f| {
            json!({
                "ruleId": f.detector,
                "level": sarif_level(&f.severity),
                "message": {
                    "text": f.message,
                },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": {
                            "uri": f.file.display().to_string(),
                        },
                        "region": {
                            "startLine": f.line,
                            "startColumn": f.column,
                        }
                    }
                }],
                "properties": {
                    "evidence": f.evidence,
                }
            })
        })
        .collect();

    let rules: Vec<serde_json::Value> = {
        let mut seen = std::collections::HashSet::new();
        findings
            .iter()
            .filter(|f| seen.insert(f.detector.clone()))
            .map(|f| {
                json!({
                    "id": f.detector,
                    "shortDescription": {
                        "text": f.detector.replace('-', " "),
                    },
                    "defaultConfiguration": {
                        "level": sarif_level(&f.severity),
                    }
                })
            })
            .collect()
    };

    // Parse failures are reported as tool execution notifications, not as
    // findings, and they mark the run as unsuccessful so callers can detect
    // that the scan over invalid input is not authoritative.
    let notifications: Vec<serde_json::Value> = parse_errors
        .iter()
        .map(|e| {
            let mut region = json!({ "startLine": e.line, "startColumn": e.column });
            if let Some(end) = e.end_column {
                region["endColumn"] = json!(end);
            }
            json!({
                "level": "error",
                "message": { "text": e.message },
                "properties": { "category": e.category.as_str() },
                "locations": [{
                    "physicalLocation": {
                        "artifactLocation": { "uri": e.file },
                        "region": region
                    }
                }]
            })
        })
        .collect();

    let sarif = json!({
        "$schema": "https://raw.githubusercontent.com/oasis-tcs/sarif-spec/main/sarif-2.1/schema/sarif-schema-2.1.0.json",
        "version": "2.1.0",
        "runs": [{
            "tool": {
                "driver": {
                    "name": "daml-lint",
                    "version": env!("CARGO_PKG_VERSION"),
                    "informationUri": env!("CARGO_PKG_REPOSITORY"),
                    "rules": rules,
                }
            },
            "invocations": [{
                "executionSuccessful": parse_errors.is_empty(),
                "toolExecutionNotifications": notifications,
            }],
            "results": results,
        }]
    });

    serde_json::to_string_pretty(&sarif).expect("SARIF report value always serializes to JSON")
}

const fn sarif_level(severity: &Severity) -> &'static str {
    match severity {
        Severity::Critical | Severity::High => "error",
        Severity::Medium => "warning",
        Severity::Low | Severity::Info => "note",
    }
}

fn format_markdown(findings: &[Finding], parse_errors: &[ParseError]) -> String {
    let mut out = String::new();
    out.push_str("# daml-lint Report\n\n");

    if !parse_errors.is_empty() {
        write!(out, "## Parse Errors ({})\n\n", parse_errors.len())
            .expect("writing to a String cannot fail");
        for e in parse_errors {
            writeln!(
                out,
                "- `{}:{}:{}` [{}] {}",
                e.file,
                e.line,
                e.column,
                e.category.as_str(),
                e.message
            )
            .expect("writing to a String cannot fail");
        }
        out.push('\n');
    }

    if findings.is_empty() {
        if parse_errors.is_empty() {
            out.push_str("No findings.\n");
        } else {
            out.push_str("No findings, but parse errors were reported above.\n");
        }
        return out;
    }

    let (critical, high, medium, low, info) = count_by_severity(findings);
    write!(
        out,
        "**Summary:** {} finding(s) — {} Critical, {} High, {} Medium, {} Low, {} Info\n\n",
        findings.len(),
        critical,
        high,
        medium,
        low,
        info
    )
    .expect("writing to a String cannot fail");

    // Group by severity
    for severity in &[
        Severity::Critical,
        Severity::High,
        Severity::Medium,
        Severity::Low,
        Severity::Info,
    ] {
        let group: Vec<_> = findings
            .iter()
            .filter(|f| f.severity == *severity)
            .collect();
        if group.is_empty() {
            continue;
        }

        write!(out, "## {severity} ({})\n\n", group.len())
            .expect("writing to a String cannot fail");

        for f in &group {
            write!(out, "### {} `{}`\n\n", f.severity, f.detector)
                .expect("writing to a String cannot fail");
            write!(out, "**{}**\n\n", f.message).expect("writing to a String cannot fail");
            writeln!(out, "- **File:** `{}:{}`", f.file.display(), f.line)
                .expect("writing to a String cannot fail");
            write!(out, "- **Evidence:**\n  ```\n  {}\n  ```\n\n", f.evidence)
                .expect("writing to a String cannot fail");
        }
    }

    out
}

fn format_json(findings: &[Finding], parse_errors: &[ParseError]) -> String {
    #[derive(Serialize)]
    struct Report {
        tool: &'static str,
        version: &'static str,
        findings: Vec<FindingJson>,
        #[serde(rename = "parseErrors")]
        parse_errors: Vec<ParseErrorJson>,
        summary: Summary,
    }

    #[derive(Serialize)]
    struct ParseErrorJson {
        file: String,
        line: usize,
        column: usize,
        #[serde(rename = "endColumn", skip_serializing_if = "Option::is_none")]
        end_column: Option<usize>,
        message: String,
        category: &'static str,
    }

    #[derive(Serialize)]
    struct FindingJson {
        detector: String,
        severity: String,
        file: String,
        line: usize,
        column: usize,
        message: String,
        evidence: String,
    }

    #[derive(Serialize)]
    struct Summary {
        total: usize,
        critical: usize,
        high: usize,
        medium: usize,
        low: usize,
        info: usize,
        #[serde(rename = "parseErrors")]
        parse_errors: usize,
    }

    let (critical, high, medium, low, info) = count_by_severity(findings);

    let report = Report {
        tool: "daml-lint",
        version: env!("CARGO_PKG_VERSION"),
        findings: findings
            .iter()
            .map(|f| FindingJson {
                detector: f.detector.clone(),
                severity: f.severity.to_string(),
                file: f.file.display().to_string(),
                line: f.line,
                column: f.column,
                message: f.message.clone(),
                evidence: f.evidence.clone(),
            })
            .collect(),
        parse_errors: parse_errors
            .iter()
            .map(|e| ParseErrorJson {
                file: e.file.clone(),
                line: e.line,
                column: e.column,
                end_column: e.end_column,
                message: e.message.clone(),
                category: e.category.as_str(),
            })
            .collect(),
        summary: Summary {
            total: findings.len(),
            critical,
            high,
            medium,
            low,
            info,
            parse_errors: parse_errors.len(),
        },
    };

    serde_json::to_string_pretty(&report).expect("lint report value always serializes to JSON")
}

fn count_by_severity(findings: &[Finding]) -> (usize, usize, usize, usize, usize) {
    let critical = findings
        .iter()
        .filter(|f| f.severity == Severity::Critical)
        .count();
    let high = findings
        .iter()
        .filter(|f| f.severity == Severity::High)
        .count();
    let medium = findings
        .iter()
        .filter(|f| f.severity == Severity::Medium)
        .count();
    let low = findings
        .iter()
        .filter(|f| f.severity == Severity::Low)
        .count();
    let info = findings
        .iter()
        .filter(|f| f.severity == Severity::Info)
        .count();
    (critical, high, medium, low, info)
}

/// Returns exit code: 0 if no findings at or above the threshold, 1 otherwise.
#[must_use]
pub fn exit_code(findings: &[Finding], fail_on: Severity) -> i32 {
    if findings
        .iter()
        .any(|f| f.severity.meets_or_exceeds(fail_on))
    {
        1
    } else {
        0
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::detector::FindingLocation;

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

    fn finding(location: usize, severity: Severity) -> Finding {
        Finding::new(
            "test-rule",
            severity,
            FindingLocation::new("Test.daml", 3, location),
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

    // A valid file (no findings, no parse errors) must still read as clean.
    #[test]
    fn clean_file_says_no_findings() {
        assert!(format_markdown(&[], &[]).contains("No findings."));
        let json: serde_json::Value = serde_json::from_str(&format_json(&[], &[])).unwrap();
        assert_eq!(json["parseErrors"].as_array().unwrap().len(), 0);
        assert_eq!(json["summary"]["parseErrors"], 0);
        let sarif: serde_json::Value = serde_json::from_str(&format_sarif(&[], &[])).unwrap();
        assert_eq!(
            sarif["runs"][0]["invocations"][0]["executionSuccessful"],
            true
        );
    }

    // Parse errors on a finding-free file must be visible in every format —
    // never reported as "No findings." with no other signal.
    #[test]
    fn markdown_exposes_parse_errors() {
        let out = format_markdown(&[], &[parse_err()]);
        assert!(out.contains("Parse Errors (1)"));
        assert!(out.contains("unterminated string literal"));
        assert!(!out.contains("\nNo findings.\n"));
    }

    #[test]
    fn json_exposes_parse_errors() {
        let json: serde_json::Value =
            serde_json::from_str(&format_json(&[], &[parse_err()])).unwrap();
        assert_eq!(json["parseErrors"].as_array().unwrap().len(), 1);
        assert_eq!(json["parseErrors"][0]["line"], 3);
        assert_eq!(json["summary"]["parseErrors"], 1);
    }

    // The recovery category and end column must reach machine-readable output
    // so consumers can distinguish unsupported syntax from a real malformation
    // and highlight the offending range.
    #[test]
    fn json_carries_category_and_end_column() {
        let json: serde_json::Value =
            serde_json::from_str(&format_json(&[], &[parse_err()])).unwrap();
        assert_eq!(json["parseErrors"][0]["category"], "lexical-error");
        assert_eq!(json["parseErrors"][0]["endColumn"], 11);
    }

    #[test]
    fn sarif_notification_carries_category_and_end_column() {
        let sarif: serde_json::Value =
            serde_json::from_str(&format_sarif(&[], &[parse_err()])).unwrap();
        let note = &sarif["runs"][0]["invocations"][0]["toolExecutionNotifications"][0];
        assert_eq!(note["properties"]["category"], "lexical-error");
        assert_eq!(
            note["locations"][0]["physicalLocation"]["region"]["endColumn"],
            11
        );
    }

    #[test]
    fn sarif_marks_run_unsuccessful_on_parse_errors() {
        let sarif: serde_json::Value =
            serde_json::from_str(&format_sarif(&[], &[parse_err()])).unwrap();
        let inv = &sarif["runs"][0]["invocations"][0];
        assert_eq!(inv["executionSuccessful"], false);
        assert_eq!(
            inv["toolExecutionNotifications"].as_array().unwrap().len(),
            1
        );
        // Parse errors are notifications, not findings.
        assert_eq!(sarif["runs"][0]["results"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn sarif_driver_information_uri_matches_crate_repository_not_placeholder() {
        let sarif: serde_json::Value = serde_json::from_str(&format_sarif(&[], &[])).unwrap();
        let uri = sarif["runs"][0]["tool"]["driver"]["informationUri"]
            .as_str()
            .unwrap();
        assert_eq!(uri, env!("CARGO_PKG_REPOSITORY"));
        assert!(!uri.contains("example/daml-lint"));
    }

    #[test]
    fn exit_code_uses_named_threshold_semantics() {
        assert_eq!(
            exit_code(&[finding(1, Severity::Critical)], Severity::High),
            1
        );
        assert_eq!(exit_code(&[finding(2, Severity::High)], Severity::High), 1);
        assert_eq!(
            exit_code(&[finding(3, Severity::Medium)], Severity::High),
            0
        );
        assert_eq!(exit_code(&[finding(4, Severity::Info)], Severity::High), 0);
        assert_eq!(exit_code(&[finding(5, Severity::Low)], Severity::Info), 1);
    }
}
