use crate::detector::{Finding, Severity};
use crate::parser::ParseDiagnosticCategory;
use daml_syntax::{CharColumn, LineNumber};
use serde::Serialize;
use serde_json::json;
use std::error::Error;
use std::fmt::Write as _;
use std::fmt::{self, Display};

/// Output format for `daml-lint` reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    /// SARIF JSON suitable for GitHub code scanning and IDE integrations.
    Sarif,
    /// Human-readable Markdown report.
    Markdown,
    /// Machine-readable JSON report.
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

impl Display for OutputFormat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Sarif => "sarif",
            Self::Markdown => "markdown",
            Self::Json => "json",
        })
    }
}

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
    /// Source file path or display name.
    pub file: String,
    /// 1-based diagnostic start line.
    pub line: LineNumber,
    /// 1-based Unicode-scalar diagnostic start column.
    pub column: CharColumn,
    /// End column when the diagnostic span sits on one line; `None` otherwise.
    pub end_column: Option<CharColumn>,
    /// Human-readable parser diagnostic message.
    pub message: String,
    /// Recovery category tag (e.g. `skipped-declaration`, `unsupported-syntax`).
    pub category: ParseDiagnosticCategory,
}

impl ParseError {
    /// Construct a parse error without relying on struct literal syntax.
    #[must_use]
    pub fn new(
        file: impl Into<String>,
        line: LineNumber,
        column: CharColumn,
        end_column: Option<CharColumn>,
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
                            "startLine": f.line.get(),
                            "startColumn": f.column.get(),
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
            let mut region = json!({ "startLine": e.line.get(), "startColumn": e.column.get() });
            if let Some(end) = e.end_column {
                region["endColumn"] = json!(end.get());
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
                e.line.get(),
                e.column.get(),
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
            writeln!(out, "- **File:** `{}:{}`", f.file.display(), f.line.get())
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
                line: f.line.get(),
                column: f.column.get(),
                message: f.message.clone(),
                evidence: f.evidence.clone(),
            })
            .collect(),
        parse_errors: parse_errors
            .iter()
            .map(|e| ParseErrorJson {
                file: e.file.clone(),
                line: e.line.get(),
                column: e.column.get(),
                end_column: e.end_column.map(|column| column.get()),
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
