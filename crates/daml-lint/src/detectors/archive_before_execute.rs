use crate::detector::{Detector, Finding, Severity};
use crate::ir::{DamlModule, Statement};

/// Detector #6: archive-before-execute
///
/// In each choice body, find sequences where archive or fetchAndArchive appears
/// before a try/catch block. The archive consumes the contract; if execution in
/// the try block fails, the archived contract is permanently lost.
///
/// Catches: H3 (archive-before-execute in CloseVoteRequest)
pub struct ArchiveBeforeExecute;

impl ArchiveBeforeExecute {
    fn check_statements(
        &self,
        statements: &[Statement],
        body_raw: &str,
        file: &std::path::Path,
        base_line: usize,
        _choice_name: &str,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();
        let mut archive_seen = false;
        let mut archive_line = 0usize;
        let mut archive_evidence = String::new();

        let lines: Vec<&str> = body_raw.lines().collect();

        // Track line positions for archive and try statements
        for (line_idx, line) in lines.iter().enumerate() {
            let raw_trimmed = line.trim();
            // Strip line comments so a comment mentioning fetchAndArchive cannot
            // masquerade as a real archive statement.
            let trimmed = match raw_trimmed.find("--") {
                Some(0) => continue,
                Some(idx) => raw_trimmed[..idx].trim_end(),
                None => raw_trimmed,
            };

            if trimmed.contains("fetchAndArchive") || trimmed.starts_with("archive ") {
                archive_seen = true;
                archive_line = base_line + line_idx;
                archive_evidence = trimmed.to_string();
            }

            if archive_seen && (trimmed.starts_with("try") || trimmed == "try") {
                findings.push(Finding {
                    detector: self.name().to_string(),
                    severity: self.severity(),
                    file: file.to_path_buf(),
                    line: archive_line,
                    column: 1,
                    message: format!(
                        "Contract archived via '{}' at line {} before try/catch block at line {}. \
                         If execution fails, the archived contract is permanently consumed.",
                        if archive_evidence.contains("fetchAndArchive") {
                            "fetchAndArchive"
                        } else {
                            "archive"
                        },
                        archive_line,
                        base_line + line_idx,
                    ),
                    evidence: format!("{}\n  ...\n  try do ...", archive_evidence),
                });
                // Only report once per archive-then-try sequence
                archive_seen = false;
            }
        }

        // Structured fallback: the raw line scan only matches an archive at the
        // start of a line, so a multiline `archive\n  cid` slips past it. The
        // parser still produces Statement::Archive, so catch the archive-then-try
        // pattern here too — deduped by line against the raw-scan findings so the
        // common case is not double-reported.
        let reported: std::collections::HashSet<usize> = findings.iter().map(|f| f.line).collect();
        let mut last_archive: Option<(usize, String)> = None;
        for stmt in statements {
            match stmt {
                Statement::Archive { span, cid_expr, .. } => {
                    last_archive = Some((span.line, cid_expr.clone()));
                }
                Statement::TryCatch { span, .. } => {
                    if let Some((archive_line, cid)) = last_archive.take() {
                        if !reported.contains(&archive_line) {
                            findings.push(Finding {
                                detector: self.name().to_string(),
                                severity: self.severity(),
                                file: file.to_path_buf(),
                                line: archive_line,
                                column: 1,
                                message: format!(
                                    "Contract archived via 'archive {}' at line {} before try/catch block at line {}. \
                                     If execution fails, the archived contract is permanently consumed.",
                                    cid.trim(),
                                    archive_line,
                                    span.line,
                                ),
                                evidence: format!("archive {}\n  ...\n  try do ...", cid.trim()),
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        findings
    }
}

impl Detector for ArchiveBeforeExecute {
    fn name(&self) -> &str {
        "archive-before-execute"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn description(&self) -> &str {
        "Contract archived before try/catch — archived contract lost if execution fails"
    }

    fn detect(&self, module: &DamlModule) -> Vec<Finding> {
        let mut findings = Vec::new();

        for template in &module.templates {
            for choice in &template.choices {
                findings.extend(self.check_statements(
                    &choice.body,
                    &choice.body_raw,
                    &module.file,
                    choice.span.line,
                    &choice.name,
                ));
            }
        }

        findings
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_daml;
    use std::path::Path;

    #[test]
    fn test_archive_before_try_triggers() {
        let source = r#"module Test where

template VoteManager
  with
    admin : Party
  where
    signatory admin

    choice CloseVoteRequest : ()
      with
        requestCid : ContractId VoteRequest
      controller admin
      do
        request <- fetchAndArchive requestCid
        let action = request.action
        try do
          executeAction action
        catch
          e -> pure ()
"#;
        let module = parse_daml(source, Path::new("DsoRules.daml"));
        let findings = ArchiveBeforeExecute.detect(&module);
        assert!(!findings.is_empty());
        assert!(findings[0].message.contains("fetchAndArchive"));
        assert!(findings[0].message.contains("try/catch"));
    }

    #[test]
    fn test_archive_after_try_passes() {
        let source = r#"module Test where

template SafeManager
  with
    admin : Party
  where
    signatory admin

    choice SafeClose : ()
      with
        requestCid : ContractId VoteRequest
      controller admin
      do
        try do
          executeAction requestCid
        catch
          e -> pure ()
        archive requestCid
"#;
        let module = parse_daml(source, Path::new("Safe.daml"));
        let findings = ArchiveBeforeExecute.detect(&module);
        assert!(findings.is_empty());
    }

    // Regression (audit F1): a finding from a CHOICE body must report the real
    // source line of the archive, not one line too early.
    #[test]
    fn test_choice_finding_reports_real_line() {
        let source = r#"module Test where

template VoteManager
  with
    admin : Party
  where
    signatory admin

    choice CloseVoteRequest : ()
      with
        requestCid : ContractId VoteRequest
      controller admin
      do
        request <- fetchAndArchive requestCid
        try do
          executeAction request
        catch
          e -> pure ()
"#;
        let module = parse_daml(source, Path::new("Test.daml"));
        let findings = ArchiveBeforeExecute.detect(&module);
        assert_eq!(findings.len(), 1);
        let real_line = source
            .lines()
            .position(|l| l.contains("fetchAndArchive"))
            .unwrap()
            + 1;
        assert_eq!(
            findings[0].line, real_line,
            "must report the real archive line, not off-by-one"
        );
    }

    // Regression (audit F5): a comment mentioning fetchAndArchive must not be
    // treated as a real archive statement.
    #[test]
    fn test_comment_mentioning_archive_does_not_trigger() {
        let source = r#"module Test where

template T
  with
    admin : Party
  where
    signatory admin

    choice C : ()
      controller admin
      do
        -- fetchAndArchive is performed by a helper elsewhere
        try do
          executeAction admin
        catch
          e -> pure ()
"#;
        let module = parse_daml(source, Path::new("Comment.daml"));
        let findings = ArchiveBeforeExecute.detect(&module);
        assert!(
            findings.is_empty(),
            "a comment must not trigger archive-before-execute: {:?}",
            findings
        );
    }

    // Regression (audit F6): a multiline `archive\n  cid` is one application the
    // raw line scan misses, but the parser produces Statement::Archive, so the
    // structured fallback must still report it.
    #[test]
    fn test_multiline_archive_before_try_is_flagged() {
        let source = r#"module Test where

template T
  with
    admin : Party
  where
    signatory admin

    choice C : ()
      with
        requestCid : ContractId Foo
      controller admin
      do
        archive
          requestCid
        try do
          executeAction admin
        catch
          e -> pure ()
"#;
        let module = parse_daml(source, Path::new("Multiline.daml"));
        let findings = ArchiveBeforeExecute.detect(&module);
        assert!(
            !findings.is_empty(),
            "multiline archive before try must be caught by the structured fallback"
        );
    }
}
