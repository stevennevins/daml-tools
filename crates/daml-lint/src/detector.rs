use crate::ir::DamlModule;
use daml_syntax::{CharColumn, LineNumber};
use serde::{Serialize, Serializer};
use std::error::Error;
use std::path::PathBuf;

fn serialize_line_number<S>(line: &LineNumber, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_u64(line.get() as u64)
}

fn serialize_char_column<S>(column: &CharColumn, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_u64(column.get() as u64)
}

/// Error returned by fallible detector execution.
///
/// Built-in detectors are currently infallible. Custom JavaScript rules can
/// fail at runtime or be interrupted, and library callers should handle those
/// failures through [`Detector::try_detect`] instead of letting a rule terminate
/// the host process.
#[derive(Debug)]
#[non_exhaustive]
pub struct DetectError {
    detector: String,
    message: String,
    source: Option<Box<dyn Error + Send + Sync + 'static>>,
}

impl DetectError {
    /// Build a detector error for `detector` with a human-readable `message`.
    #[must_use]
    pub fn new(detector: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            detector: detector.into(),
            message: message.into(),
            source: None,
        }
    }

    /// Build a detector error that preserves the recoverable cause that made
    /// the detector fail.
    #[must_use]
    pub fn with_source<E>(
        detector: impl Into<String>,
        message: impl Into<String>,
        source: E,
    ) -> Self
    where
        E: Error + Send + Sync + 'static,
    {
        Self {
            detector: detector.into(),
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Detector or custom-rule name that produced the error.
    #[must_use]
    pub fn detector(&self) -> &str {
        &self.detector
    }

    /// Human-readable error detail.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Recoverable source error that caused detector execution to fail, when
    /// one is available.
    #[must_use]
    pub fn source(&self) -> Option<&(dyn Error + Send + Sync + 'static)> {
        self.source.as_deref()
    }
}

impl std::fmt::Display for DetectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "detector '{}': {}", self.detector, self.message)
    }
}

impl std::error::Error for DetectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source
            .as_deref()
            .map(|source| source as &(dyn std::error::Error + 'static))
    }
}

/// Error returned when parsing an unsupported severity value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeverityParseError {
    value: String,
}

impl SeverityParseError {
    /// Unsupported severity text that was rejected.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

impl std::fmt::Display for SeverityParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "invalid severity: {} (expected one of critical|high|medium|low|info)",
            self.value
        )
    }
}

impl std::error::Error for SeverityParseError {}

/// Severity assigned to a detector finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub enum Severity {
    /// Critical issue that should fail any release or CI gate.
    Critical,
    /// High-risk issue; this is the default fail threshold for the CLI.
    High,
    /// Medium-risk issue that may indicate nondeterminism or policy drift.
    Medium,
    /// Low-risk issue.
    Low,
    /// Informational issue.
    Info,
}

impl Severity {
    /// Relative risk ordering used by report sorting and threshold checks.
    ///
    /// `Critical` has the highest rank (`5`) and `Info` the lowest (`1`).
    /// `Severity` intentionally does not implement `Ord`; use `rank()` or
    /// `meets_or_exceeds()` for risk-based ordering and threshold checks.
    #[must_use]
    pub const fn rank(self) -> u8 {
        match self {
            Self::Critical => 5,
            Self::High => 4,
            Self::Medium => 3,
            Self::Low => 2,
            Self::Info => 1,
        }
    }

    /// Returns `true` when `self` is at least as severe as `threshold`.
    ///
    /// For example, with threshold `High`, `Critical` and `High` both return
    /// `true`, while `Medium` and below return `false`.
    #[must_use]
    pub const fn meets_or_exceeds(self, threshold: Self) -> bool {
        self.rank() >= threshold.rank()
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Critical => write!(f, "CRITICAL"),
            Self::High => write!(f, "HIGH"),
            Self::Medium => write!(f, "MEDIUM"),
            Self::Low => write!(f, "LOW"),
            Self::Info => write!(f, "INFO"),
        }
    }
}

impl std::str::FromStr for Severity {
    type Err = SeverityParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "critical" => Ok(Self::Critical),
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            "info" => Ok(Self::Info),
            _ => Err(SeverityParseError {
                value: s.to_string(),
            }),
        }
    }
}

/// A single detector result reported for one source location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct Finding {
    /// Detector or custom-rule name.
    pub detector: String,
    /// Finding severity.
    pub severity: Severity,
    /// Source file where the finding was reported.
    pub file: PathBuf,
    /// 1-based source line.
    #[serde(serialize_with = "serialize_line_number")]
    pub line: LineNumber,
    /// 1-based source column.
    #[serde(serialize_with = "serialize_char_column")]
    pub column: CharColumn,
    /// Human-readable finding message.
    pub message: String,
    /// Source excerpt or structural evidence for the finding.
    pub evidence: String,
}

/// Bundle of source location metadata for a finding.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct FindingLocation {
    /// Source file where the finding was reported.
    pub file: PathBuf,
    /// 1-based source line.
    pub line: LineNumber,
    /// 1-based source column.
    pub column: CharColumn,
}

impl FindingLocation {
    /// Construct a finding source location without relying on struct literal syntax.
    #[must_use]
    pub fn new(file: impl Into<PathBuf>, line: LineNumber, column: CharColumn) -> Self {
        Self {
            file: file.into(),
            line,
            column,
        }
    }
}

impl Finding {
    /// Construct a `Finding` without relying on struct literal syntax.
    #[must_use]
    pub fn new(
        detector: impl Into<String>,
        severity: Severity,
        location: FindingLocation,
        message: impl Into<String>,
        evidence: impl Into<String>,
    ) -> Self {
        Self {
            detector: detector.into(),
            severity,
            file: location.file,
            line: location.line,
            column: location.column,
            message: message.into(),
            evidence: evidence.into(),
        }
    }
}

// Scanning is single-threaded; detectors hold per-rule QuickJS state.
/// Static analysis rule over a lowered Daml module.
///
/// Implement [`Detector::try_detect`] for all rules; it is the required API and
/// communicates runtime/script failures to callers. Implementations that cannot
/// fail can use the default [`Detector::detect`] convenience adapter or provide
/// a fallible implementation that returns `Ok`.
pub trait Detector {
    /// Stable detector name used in reports and duplicate-rule checks.
    fn name(&self) -> &str;
    /// Severity assigned to findings from this detector.
    fn severity(&self) -> Severity;
    /// One-line detector description.
    fn description(&self) -> &str;
    /// Run an infallible detector over `module`.
    ///
    /// Implementations may continue to implement this directly; for most rules
    /// this adapter provides a panic-first convenience layer over
    /// [`Detector::try_detect`].
    ///
    /// # Panics
    ///
    /// Panics when [`Detector::try_detect`] returns [`DetectError`]. Use
    /// [`Detector::try_detect`] directly for custom JavaScript rules or other
    /// recoverable detector failures that should not terminate the caller.
    fn detect(&self, module: &DamlModule) -> Vec<Finding> {
        self.try_detect(module)
            .unwrap_or_else(|e| panic!("detector '{}' failed: {}", self.name(), e))
    }
    /// Run a detector that may fail without terminating the caller.
    ///
    /// # Errors
    ///
    /// Returns [`DetectError`] when the detector cannot analyze `module`.
    fn try_detect(&self, module: &DamlModule) -> Result<Vec<Finding>, DetectError>;
}

/// Detector wrapper that can rename a rule and/or override finding severity.
pub struct ConfiguredDetector {
    inner: Box<dyn Detector>,
    name_override: Option<String>,
    severity_override: Option<Severity>,
}

impl ConfiguredDetector {
    fn new(
        inner: Box<dyn Detector>,
        name_override: Option<String>,
        severity_override: Option<Severity>,
    ) -> Self {
        Self {
            inner,
            name_override,
            severity_override,
        }
    }

    /// Construct a configured detector that only renames the wrapped rule.
    #[must_use]
    pub fn with_name(inner: Box<dyn Detector>, name: impl Into<String>) -> Self {
        Self::new(inner, Some(name.into()), None)
    }

    /// Construct a configured detector that only overrides the wrapped rule
    /// severity.
    #[must_use]
    pub fn with_severity(inner: Box<dyn Detector>, severity: Severity) -> Self {
        Self::new(inner, None, Some(severity))
    }

    fn apply_overrides(&self, mut findings: Vec<Finding>) -> Vec<Finding> {
        let name = self.name().to_string();
        let severity = self.severity();
        for finding in &mut findings {
            if self.name_override.is_some() {
                finding.detector = name.clone();
            }
            if self.severity_override.is_some() {
                finding.severity = severity;
            }
        }
        findings
    }
}

impl Detector for ConfiguredDetector {
    fn name(&self) -> &str {
        self.name_override
            .as_deref()
            .unwrap_or_else(|| self.inner.name())
    }

    fn severity(&self) -> Severity {
        self.severity_override
            .unwrap_or_else(|| self.inner.severity())
    }

    fn description(&self) -> &str {
        self.inner.description()
    }

    fn try_detect(&self, module: &DamlModule) -> Result<Vec<Finding>, DetectError> {
        self.inner
            .try_detect(module)
            .map(|findings| self.apply_overrides(findings))
            .map_err(|e| {
                let wrapped_detector = e.detector().to_string();
                let message = format!(
                    "wrapped detector '{wrapped_detector}' failed: {}",
                    e.message()
                );
                DetectError::with_source(self.name(), message, e)
            })
    }
}

/// Returns the first detector name that appears more than once, if any.
#[must_use]
pub fn find_duplicate_detector_name(detectors: &[Box<dyn Detector>]) -> Option<String> {
    let mut seen = std::collections::HashSet::new();
    for det in detectors {
        if !seen.insert(det.name()) {
            return Some(det.name().to_string());
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod configured_detector_tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn configured_detector_with_name_rewrites_detector_name_and_findings() {
        struct NamedDetector;

        impl Detector for NamedDetector {
            fn name(&self) -> &str {
                "named-base"
            }

            fn severity(&self) -> Severity {
                Severity::High
            }

            fn description(&self) -> &str {
                "base description"
            }

            fn try_detect(
                &self,
                module: &crate::ir::DamlModule,
            ) -> Result<Vec<Finding>, DetectError> {
                let _ = module;
                Ok(vec![Finding::new(
                    self.name(),
                    self.severity(),
                    FindingLocation::new("named.daml", LineNumber::new(9), CharColumn::new(11)),
                    "rewritable finding",
                    "x",
                )])
            }
        }

        let detector: Box<dyn Detector> = Box::new(ConfiguredDetector::with_name(
            Box::new(NamedDetector),
            "rewrite",
        ));
        assert_eq!(detector.name(), "rewrite");
        let module = crate::ir::DamlModule {
            ir_version: 4,
            name: String::from("Main"),
            file: PathBuf::from("named.daml"),
            source: String::new(),
            imports: Vec::new(),
            templates: Vec::new(),
            interfaces: Vec::new(),
            functions: Vec::new(),
        };
        let findings = detector.try_detect(&module).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].detector, "rewrite");
        assert_eq!(findings[0].line, LineNumber::new(9));
    }

    #[test]
    fn configured_detector_with_severity_overrides_reported_severity() {
        struct SeverityDetector;

        impl Detector for SeverityDetector {
            fn name(&self) -> &str {
                "severity-base"
            }

            fn severity(&self) -> Severity {
                Severity::Critical
            }

            fn description(&self) -> &str {
                "base description"
            }

            fn try_detect(
                &self,
                module: &crate::ir::DamlModule,
            ) -> Result<Vec<Finding>, DetectError> {
                let _ = module;
                Ok(vec![Finding::new(
                    self.name(),
                    self.severity(),
                    FindingLocation::new("severity.daml", LineNumber::new(3), CharColumn::new(7)),
                    "high-severity finding",
                    "y",
                )])
            }
        }

        let detector: Box<dyn Detector> = Box::new(ConfiguredDetector::with_severity(
            Box::new(SeverityDetector),
            Severity::Info,
        ));
        assert_eq!(detector.severity(), Severity::Info);
        let module = crate::ir::DamlModule {
            ir_version: 4,
            name: String::from("Main"),
            file: PathBuf::from("severity.daml"),
            source: String::new(),
            imports: Vec::new(),
            templates: Vec::new(),
            interfaces: Vec::new(),
            functions: Vec::new(),
        };
        let findings = detector.try_detect(&module).unwrap();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity, Severity::Info);
    }

    #[test]
    fn configured_detector_preserves_wrapped_error_source_chain() {
        struct FailingDetector;

        impl Detector for FailingDetector {
            fn name(&self) -> &str {
                "failing-base"
            }

            fn severity(&self) -> Severity {
                Severity::High
            }

            fn description(&self) -> &str {
                "base description"
            }

            fn try_detect(
                &self,
                module: &crate::ir::DamlModule,
            ) -> Result<Vec<Finding>, DetectError> {
                let _ = module;
                Err(DetectError::with_source(
                    self.name(),
                    "could not run visitor",
                    std::io::Error::new(std::io::ErrorKind::Interrupted, "runtime stopped"),
                ))
            }
        }

        let detector: Box<dyn Detector> = Box::new(ConfiguredDetector::with_name(
            Box::new(FailingDetector),
            "configured-name",
        ));
        let module = crate::ir::DamlModule {
            ir_version: 4,
            name: String::from("Main"),
            file: PathBuf::from("failing.daml"),
            source: String::new(),
            imports: Vec::new(),
            templates: Vec::new(),
            interfaces: Vec::new(),
            functions: Vec::new(),
        };

        let err = detector.try_detect(&module).unwrap_err();
        assert_eq!(err.detector(), "configured-name");
        assert!(err.message().contains("failing-base"));
        let wrapped = std::error::Error::source(&err)
            .and_then(|source| source.downcast_ref::<DetectError>())
            .expect("configured detector should preserve the inner DetectError");
        assert_eq!(wrapped.detector(), "failing-base");
        assert!(std::error::Error::source(wrapped)
            .is_some_and(|source| source.to_string() == "runtime stopped"));
    }
}
