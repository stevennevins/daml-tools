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
    #[must_use]
    pub fn new(detector: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            detector: detector.into(),
            message: message.into(),
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

impl Severity {
    /// Relative risk ordering used by report sorting and threshold checks.
    ///
    /// `Critical` has the highest rank (`5`) and `Info` the lowest (`1`).
    /// This stays additive and documents intent explicitly while preserving the
    /// existing `Ord` implementation used by downstream consumers.
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

/// Bundle of source location metadata for a finding.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct FindingLocation {
    /// Source file where the finding was reported.
    pub file: PathBuf,
    /// 1-based source line.
    pub line: usize,
    /// 1-based source column.
    pub column: usize,
}

impl FindingLocation {
    /// Construct a finding source location without relying on struct literal syntax.
    #[must_use]
    pub fn new(file: impl Into<PathBuf>, line: usize, column: usize) -> Self {
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

/// Parse a severity string accepted by the CLI.
#[must_use]
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
    #[must_use]
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
            .map_err(|e| DetectError::new(self.name(), e.message().to_string()))
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

#[cfg(all(test, feature = "js-runtime"))]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn finding() -> Finding {
        Finding::new(
            "unused-foo",
            Severity::High,
            FindingLocation::new("foo.daml", 10, 4),
            "consider removing",
            "foo",
        )
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
                    FindingLocation::new("named.daml", 9, 11),
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
        assert_eq!(findings[0].line, 9);
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
                    FindingLocation::new("severity.daml", 3, 7),
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
    fn severity_ord_follows_enum_order_not_risk_rank() {
        // `Ord` follows declaration order (Critical is the smallest variant).
        // Callers sorting or gating by risk must use `rank()` / `meets_or_exceeds`.
        assert!(Severity::Critical < Severity::Info);
        assert!(Severity::High < Severity::Low);
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
                FindingLocation::new("b.daml", 10, 4),
                "medium finding",
                "evidence",
            ),
            Finding::new(
                "rule-critical",
                Severity::Critical,
                FindingLocation::new("a.daml", 3, 1),
                "critical finding",
                "evidence",
            ),
            Finding::new(
                "rule-high",
                Severity::High,
                FindingLocation::new("a.daml", 5, 2),
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
}
