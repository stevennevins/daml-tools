use crate::ir::DamlModule;
use serde::Serialize;
use std::path::PathBuf;

/// Error returned by fallible detector execution.
///
/// Built-in detectors are currently infallible. Custom JavaScript rules can
/// fail at runtime or be interrupted, and library callers should handle those
/// failures through [`Detector::try_detect`] instead of letting a rule terminate
/// the host process.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct DetectError {
    detector: String,
    message: String,
}

impl DetectError {
    /// Build a detector error for `detector` with a human-readable `message`.
    pub fn new(detector: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            detector: detector.into(),
            message: message.into(),
        }
    }

    /// Detector or custom-rule name that produced the error.
    pub fn detector(&self) -> &str {
        &self.detector
    }

    /// Human-readable error detail.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for DetectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "detector '{}': {}", self.detector, self.message)
    }
}

impl std::error::Error for DetectError {}

/// Severity assigned to a detector finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize)]
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
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "critical" => Ok(Self::Critical),
            "high" => Ok(Self::High),
            "medium" => Ok(Self::Medium),
            "low" => Ok(Self::Low),
            "info" => Ok(Self::Info),
            _ => Err(()),
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
    pub line: usize,
    /// 1-based source column.
    pub column: usize,
    /// Human-readable finding message.
    pub message: String,
    /// Source excerpt or structural evidence for the finding.
    pub evidence: String,
}

/// Parse a severity string accepted by the CLI.
pub fn parse_severity(s: &str) -> Option<Severity> {
    s.parse().ok()
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
    fn detect(&self, module: &DamlModule) -> Vec<Finding> {
        self.try_detect(module)
            .unwrap_or_else(|e| panic!("detector '{}' failed: {}", self.name(), e))
    }
    /// Run a detector that may fail without terminating the caller.
    fn try_detect(&self, module: &DamlModule) -> Result<Vec<Finding>, DetectError>;
}

/// Detector wrapper that can rename a rule and/or override finding severity.
pub struct ConfiguredDetector {
    inner: Box<dyn Detector>,
    name_override: Option<String>,
    severity_override: Option<Severity>,
}

impl ConfiguredDetector {
    pub fn new(
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
            .map_err(|e| DetectError::new(self.name(), e.message().to_string()))
    }
}

/// Returns the first detector name that appears more than once, if any.
pub fn find_duplicate_detector_name(detectors: &[Box<dyn Detector>]) -> Option<String> {
    let mut seen = std::collections::HashSet::new();
    for det in detectors {
        if !seen.insert(det.name()) {
            return Some(det.name().to_string());
        }
    }
    None
}

#[cfg(all(test, feature = "js-runtime"))]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn finding() -> Finding {
        Finding {
            detector: "unused-foo".to_string(),
            severity: Severity::High,
            file: PathBuf::from("foo.daml"),
            line: 10,
            column: 4,
            message: "consider removing".to_string(),
            evidence: "foo".to_string(),
        }
    }

    #[test]
    fn returns_none_when_detector_names_are_unique() {
        assert_eq!(
            find_duplicate_detector_name(&crate::detectors::create_builtin_detectors()),
            None
        );
    }

    #[test]
    fn returns_duplicate_detector_name() {
        let mut doubled = crate::detectors::create_builtin_detectors();
        doubled.extend(crate::detectors::create_builtin_detectors());
        assert!(find_duplicate_detector_name(&doubled).is_some());
    }

    #[test]
    fn finding_is_comparable() {
        assert_eq!(finding(), finding());
    }
}
