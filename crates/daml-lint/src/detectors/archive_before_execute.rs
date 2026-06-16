use crate::detector::{Detector, Finding, Severity};
use crate::ir::{DamlModule, Expr, Statement};

/// Detector #6: archive-before-execute
///
/// In each choice body, find sequences where archive or fetchAndArchive appears
/// before a try/catch block. The archive consumes the contract; if execution in
/// the try block fails, the archived contract is permanently lost.
///
/// Catches: H3 (archive-before-execute in CloseVoteRequest)
pub struct ArchiveBeforeExecute;

/// One archive that consumes a contract: where it is, what spelled it, and the
/// contract id text (for the message).
struct Archived {
    line: usize,
    kind: &'static str,
    cid: String,
}

impl ArchiveBeforeExecute {
    /// Walk a statement scope IN ORDER. Every archive seen before a `try/catch`
    /// at this level is reported (each consumes a contract). Archives inside an
    /// EARLIER try/catch live in that block's body, not this scope, so a later
    /// sibling `try` cannot wrongly flag them. Each try/catch body is then
    /// scanned as its own scope.
    fn check_statements(&self, statements: &[Statement], file: &std::path::Path) -> Vec<Finding> {
        let mut findings = Vec::new();
        let mut pending: Vec<Archived> = Vec::new();

        for (i, stmt) in statements.iter().enumerate() {
            match stmt {
                Statement::Archive { span, cid, .. } => {
                    // `fetchAndArchive` lowers to an Archive immediately followed
                    // by a Fetch of the same cid; recognize it for the message.
                    let kind = if is_fetch_and_archive(statements, i) {
                        "fetchAndArchive"
                    } else {
                        "archive"
                    };
                    pending.push(Archived {
                        line: span.line,
                        kind,
                        cid: expr_text(cid),
                    });
                }
                // `exercise cid Archive` (the built-in Archive choice) also
                // consumes the contract — the canonical H3 pattern.
                Statement::Exercise {
                    span,
                    cid,
                    choice_name,
                    ..
                } if choice_name == "Archive" || choice_name.ends_with(".Archive") => {
                    pending.push(Archived {
                        line: span.line,
                        kind: "archive",
                        cid: expr_text(cid),
                    });
                }
                Statement::TryCatch {
                    span,
                    try_body,
                    catch_body,
                    ..
                } => {
                    for a in std::mem::take(&mut pending) {
                        findings.push(self.finding(file, &a, span.line));
                    }
                    // Each branch is its own scope (catches nested archive-then-try).
                    findings.extend(self.check_statements(try_body, file));
                    findings.extend(self.check_statements(catch_body, file));
                }
                // An `if`/`case` runs exactly ONE arm, so an archive in one arm and
                // a `try` in another are mutually exclusive — never paired. Scan
                // each arm as its own independent scope. Parent `pending` archives
                // are left untouched: they still precede any LATER top-level try,
                // but they do not reach into (or escape from) these arms.
                Statement::Branch { arms, .. } => {
                    for arm in arms {
                        findings.extend(self.check_statements(&arm.body, file));
                    }
                }
                _ => {}
            }
        }

        findings
    }

    fn finding(&self, file: &std::path::Path, a: &Archived, try_line: usize) -> Finding {
        Finding {
            detector: self.name().to_string(),
            severity: self.severity(),
            file: file.to_path_buf(),
            line: a.line,
            column: 1,
            message: format!(
                "Contract archived via '{}' at line {} before try/catch block at line {}. \
                 If execution fails, the archived contract is permanently consumed.",
                a.kind, a.line, try_line,
            ),
            evidence: format!("{} {}\n  ...\n  try do ...", a.kind, a.cid.trim()),
        }
    }
}

/// True if `statements[i]` is the Archive half of a `fetchAndArchive` — the
/// parser lowers that to an Archive directly followed by a Fetch of the same
/// contract at the same source line.
fn is_fetch_and_archive(statements: &[Statement], i: usize) -> bool {
    let Statement::Archive { span, cid, .. } = &statements[i] else {
        return false;
    };
    matches!(
        statements.get(i + 1),
        Some(Statement::Fetch { span: fspan, cid: fcid, .. })
            if fspan.line == span.line && fcid == cid
    )
}

fn expr_text(expr: &Expr) -> String {
    match expr {
        Expr::Var {
            qualifier, name, ..
        }
        | Expr::Con {
            qualifier, name, ..
        } => qualifier
            .as_ref()
            .map_or_else(|| name.clone(), |q| format!("{q}.{name}")),
        Expr::Lit { value, .. } => value.clone(),
        Expr::App { func, args, .. } => {
            let mut parts = Vec::with_capacity(args.len() + 1);
            parts.push(expr_text(func));
            parts.extend(args.iter().map(expr_text));
            parts.join(" ")
        }
        Expr::BinOp { op, lhs, rhs, .. } if op == "." => {
            format!("{}.{}", expr_text(lhs), expr_text(rhs))
        }
        Expr::Unknown { raw, .. } => raw.clone(),
        other => format!("{other:?}"),
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
                findings.extend(self.check_statements(&choice.body, &module.file));
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

    // Regression (sweep F3/F4): archival via `exercise cid Archive` before a try
    // is the canonical H3 pattern and must be flagged.
    #[test]
    fn test_exercise_archive_before_try_is_flagged() {
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
        exercise requestCid Archive
        try do
          executeAction admin
        catch
          e -> pure ()
"#;
        let module = parse_daml(source, Path::new("Test.daml"));
        let findings = ArchiveBeforeExecute.detect(&module);
        assert!(
            !findings.is_empty(),
            "exercise Archive before try must be flagged"
        );
    }

    // Regression (audit round-3): an identifier that merely STARTS with "try"
    // (`tryAgain`) is not a try/catch block and must not flag a prior archive.
    #[test]
    fn test_identifier_starting_with_try_is_not_a_try_block() {
        let source = r#"module Test where

template T
  with
    admin : Party
  where
    signatory admin

    choice C : ()
      with
        cid : ContractId Foo
      controller admin
      do
        archive cid
        tryAgain admin
        pure ()
"#;
        let module = parse_daml(source, Path::new("TryName.daml"));
        let findings = ArchiveBeforeExecute.detect(&module);
        assert!(
            findings.is_empty(),
            "`tryAgain` is not a try/catch block: {:?}",
            findings
        );
    }

    // Regression (audit round-3): an archive nested inside an EARLIER try/catch
    // is not flagged by a LATER, separate try block.
    #[test]
    fn test_archive_inside_earlier_try_not_flagged_by_later_try() {
        let source = r#"module Test where

template T
  with
    admin : Party
  where
    signatory admin

    choice C : ()
      with
        cid : ContractId Foo
      controller admin
      do
        try do
          archive cid
        catch
          e -> pure ()
        try do
          executeAction admin
        catch
          e -> pure ()
"#;
        let module = parse_daml(source, Path::new("Scoped.daml"));
        let findings = ArchiveBeforeExecute.detect(&module);
        assert!(
            findings.is_empty(),
            "archive inside the first try is not 'before' the second try: {:?}",
            findings
        );
    }

    // Regression (audit round-3): a fetchAndArchive inside a STRING literal is
    // not a real archive.
    #[test]
    fn test_archive_in_string_literal_is_not_archive() {
        let source = r#"module Test where

template T
  with
    admin : Party
  where
    signatory admin

    choice C : ()
      controller admin
      do
        debug "call fetchAndArchive before retrying"
        try do
          executeAction admin
        catch
          e -> pure ()
"#;
        let module = parse_daml(source, Path::new("StrArch.daml"));
        let findings = ArchiveBeforeExecute.detect(&module);
        assert!(
            findings.is_empty(),
            "fetchAndArchive inside a string is not an archive: {:?}",
            findings
        );
    }

    // Regression (audit round-3): multiple MULTILINE archives before one try are
    // each reported (the structured walk does not drop earlier ones).
    #[test]
    fn test_multiple_multiline_archives_each_reported() {
        let source = r#"module Test where

template T
  with
    admin : Party
  where
    signatory admin

    choice C : ()
      with
        a : ContractId Foo
        b : ContractId Foo
      controller admin
      do
        archive
          a
        archive
          b
        try do
          executeAction admin
        catch
          e -> pure ()
"#;
        let module = parse_daml(source, Path::new("MultiMulti.daml"));
        let findings = ArchiveBeforeExecute.detect(&module);
        assert_eq!(
            findings.len(),
            2,
            "both multiline archives must be reported: {:?}",
            findings
        );
    }

    // Regression (findings 13/16): an archive in the THEN arm and a try in the
    // ELSE arm are mutually exclusive — exactly one runs — so the archive can
    // never execute before that try. No finding.
    #[test]
    fn test_archive_then_try_else_not_flagged() {
        let source = r#"module Test where

template T
  with
    admin : Party
  where
    signatory admin

    choice C : ()
      with
        cid : ContractId Foo
        useArchive : Bool
      controller admin
      do
        if useArchive
          then archive cid
          else try do
                 doWork admin
               catch
                 e -> pure ()
"#;
        let module = parse_daml(source, Path::new("Branch.daml"));
        assert!(
            ArchiveBeforeExecute.detect(&module).is_empty(),
            "archive in then / try in else are mutually exclusive: {:?}",
            ArchiveBeforeExecute.detect(&module)
        );
    }

    // Regression (finding 14): the SWAPPED arms — try in then, archive in else —
    // is the SAME mutually-exclusive situation and must be treated identically
    // (also no finding), not order-dependently.
    #[test]
    fn test_try_then_archive_else_not_flagged() {
        let source = r#"module Test where

template T
  with
    admin : Party
  where
    signatory admin

    choice C : ()
      with
        cid : ContractId Foo
        useArchive : Bool
      controller admin
      do
        if useArchive
          then try do
                 doWork admin
               catch
                 e -> pure ()
          else archive cid
"#;
        let module = parse_daml(source, Path::new("Branch.daml"));
        assert!(
            ArchiveBeforeExecute.detect(&module).is_empty(),
            "try in then / archive in else are mutually exclusive: {:?}",
            ArchiveBeforeExecute.detect(&module)
        );
    }

    // Regression (finding 13/16): an archive-before-try WITHIN a single arm still
    // flags — the per-arm scope must catch ordered archive→try.
    #[test]
    fn test_archive_before_try_within_one_arm_is_flagged() {
        let source = r#"module Test where

template T
  with
    admin : Party
  where
    signatory admin

    choice C : ()
      with
        cid : ContractId Foo
        flag : Bool
      controller admin
      do
        if flag
          then do
            archive cid
            try do
              doWork admin
            catch
              e -> pure ()
          else pure ()
"#;
        let module = parse_daml(source, Path::new("WithinArm.daml"));
        assert!(
            !ArchiveBeforeExecute.detect(&module).is_empty(),
            "an archive then try in the SAME arm must still flag"
        );
    }

    // Regression (finding 11): a let-helper that DEFINES `archive` but is never
    // invoked archives nothing — no finding.
    #[test]
    fn test_uncalled_archive_helper_not_flagged() {
        let source = r#"module Test where

template T
  with
    admin : Party
  where
    signatory admin

    choice C : ()
      with
        cid : ContractId Foo
      controller admin
      do
        let doArchive x = archive x
        try do
          doWork admin
        catch
          e -> pure ()
"#;
        let module = parse_daml(source, Path::new("Helper.daml"));
        assert!(
            ArchiveBeforeExecute.detect(&module).is_empty(),
            "a never-invoked archive helper consumes nothing: {:?}",
            ArchiveBeforeExecute.detect(&module)
        );
    }

    // Regression (finding 12): an archive helper invoked INSIDE the try body runs
    // inside the protected block — no finding.
    #[test]
    fn test_archive_helper_called_inside_try_not_flagged() {
        let source = r#"module Test where

template T
  with
    admin : Party
  where
    signatory admin

    choice C : ()
      with
        cid : ContractId Foo
      controller admin
      do
        let doArchive x = archive x
        try do
          doWork admin
          doArchive cid
        catch
          e -> pure ()
"#;
        let module = parse_daml(source, Path::new("Helper.daml"));
        assert!(
            ArchiveBeforeExecute.detect(&module).is_empty(),
            "a helper called inside the try archives inside the protected block: {:?}",
            ArchiveBeforeExecute.detect(&module)
        );
    }

    // Regression (finding 15): a helper invoked BEFORE the try flags at the
    // CALL line with the real cid, not the definition line / formal param.
    #[test]
    fn test_archive_helper_called_before_try_flags_at_call_site() {
        let source = r#"module Test where

template T
  with
    admin : Party
  where
    signatory admin

    choice C : ()
      with
        cid : ContractId Foo
      controller admin
      do
        let doArchive x = archive x
        doArchive cid
        try do
          executeAction admin
        catch
          e -> pure ()
"#;
        let module = parse_daml(source, Path::new("Helper.daml"));
        let findings = ArchiveBeforeExecute.detect(&module);
        assert_eq!(
            findings.len(),
            1,
            "the helper call before the try archives: {findings:?}"
        );
        let call_line = source
            .lines()
            .position(|l| l.trim() == "doArchive cid")
            .unwrap()
            + 1;
        assert_eq!(
            findings[0].line, call_line,
            "must cite the call site, not the definition line"
        );
        assert!(
            findings[0].evidence.contains("cid") && !findings[0].evidence.contains(" x\n"),
            "evidence must cite the real cid, not the formal param: {}",
            findings[0].evidence
        );
    }

    // Regression (sweep F33): each archive before one try is reported, not only
    // the last.
    #[test]
    fn test_multiple_archives_each_reported() {
        let source = r#"module Test where

template T
  with
    admin : Party
  where
    signatory admin

    choice C : ()
      with
        a : ContractId Foo
        b : ContractId Foo
      controller admin
      do
        archive a
        archive b
        try do
          executeAction admin
        catch
          e -> pure ()
"#;
        let module = parse_daml(source, Path::new("Test.daml"));
        let findings = ArchiveBeforeExecute.detect(&module);
        assert_eq!(
            findings.len(),
            2,
            "both archives must be reported: {:?}",
            findings
        );
    }
}
