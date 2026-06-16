use crate::detector::{Detector, Finding, Severity};
use crate::ir::{DamlModule, Expr, Statement};

/// True if the choice body, on its `Expr` tree, guarantees `param` is strictly
/// positive — either by ASSERTING it (`assertMsg "..." (amount > 0)`) or by a
/// defensive guard that rejects the non-positive case. Decided structurally, so
/// whitespace, comments, and a non-asserting mention (`let isPos = amount > 0`)
/// never matter.
fn has_positive_amount_check(body: &[Statement], param: &str) -> bool {
    let mut found = false;
    // Only an UNCONDITIONAL assert guarantees the bound on every path; an assert
    // inside one `if`/`case` arm leaves the other arms unchecked.
    crate::ir::walk_unconditional_stmts(body, &mut |s| {
        if let Statement::Assert { condition_expr, .. } = s {
            if crate::ir::expr_guarantees_strict_positive(condition_expr, param) {
                found = true;
            }
        }
    });
    found || has_defensive_amount_guard(body, param)
}

/// A defensive guard that REJECTS a non-positive amount is just as safe as a
/// `> 0` assertion: `when (amount <= 0) abort`, `unless (amount > 0) abort`,
/// `if amount <= 0 then abort else ...`. Decided on the structured tree.
fn has_defensive_amount_guard(body: &[Statement], param: &str) -> bool {
    let mut found = false;
    crate::ir::walk_body_exprs(body, &mut |e| {
        if found {
            return;
        }
        if let Expr::App { func, args, .. } = e {
            if let Expr::Var {
                name,
                qualifier: None,
                ..
            } = func.as_ref()
            {
                match (name.as_str(), args.first(), args.get(1)) {
                    ("when", Some(cond), Some(action))
                        if is_nonpositive_test(cond, param) && aborts(action) =>
                    {
                        found = true;
                    }
                    ("unless", Some(cond), Some(action))
                        if is_strict_positive_test(cond, param) && aborts(action) =>
                    {
                        found = true;
                    }
                    _ => {}
                }
            }
        }
    });
    if found {
        return true;
    }
    // `if amount <= 0 then abort else ...` rejects the non-positive case;
    // `if amount > 0 then ... else abort` is the same guard flipped. A
    // statement-position `if` is lowered to a `Branch` whose scrutinee is the
    // condition and whose arms (then, else) are independent scopes.
    crate::ir::walk_body_stmts(body, &mut |s| {
        if let Statement::Branch {
            scrutinee: Some(cond),
            arms,
            ..
        } = s
        {
            // An `if` (not a `case`): two arms, neither carrying a pattern.
            if arms.len() == 2
                && arms.iter().all(|a| a.pattern.is_none())
                && ((is_nonpositive_test(cond, param) && body_aborts(&arms[0].body))
                    || (is_strict_positive_test(cond, param) && body_aborts(&arms[1].body)))
            {
                found = true;
            }
        }
    });
    found
}

/// True if any statement in `body` aborts the transaction (`abort`/`error`/
/// `fail`/`assertFail`).
fn body_aborts(body: &[Statement]) -> bool {
    let mut found = false;
    crate::ir::walk_body_exprs(body, &mut |e| {
        if aborts(e) {
            found = true;
        }
    });
    found
}

/// `amount <= 0`, `0 >= amount`, or `not (amount > 0)` — a rejection of the
/// non-positive case (strict `< 0` is NOT enough; it would still admit zero).
fn is_nonpositive_test(c: &Expr, param: &str) -> bool {
    match c {
        Expr::BinOp { op, lhs, rhs, .. } => match op.as_str() {
            "<=" => lhs.refers_to(param) && rhs.is_zero_lit(),
            ">=" => rhs.refers_to(param) && lhs.is_zero_lit(),
            _ => false,
        },
        Expr::App { func, args, .. } => {
            matches!(func.as_ref(), Expr::Var { name, .. } if name == "not")
                && args.len() == 1
                && is_strict_positive_test(&args[0], param)
        }
        _ => false,
    }
}

/// A strict-positive test on `param` (`amount > 0`, `0 < amount`, or a positive
/// floor `amount >= 0.01`). `amount >= 0` is NOT strict — it admits zero.
fn is_strict_positive_test(c: &Expr, param: &str) -> bool {
    crate::ir::is_strict_positive_bound(c, param)
}

/// True if the action stops the transaction (`abort`/`error`/`fail` appears).
fn aborts(action: &Expr) -> bool {
    let mut found = false;
    crate::ir::for_each_subexpr(action, &mut |e| {
        if let Expr::Var {
            name,
            qualifier: None,
            ..
        } = e
        {
            if matches!(name.as_str(), "abort" | "error" | "fail" | "assertFail") {
                found = true;
            }
        }
    });
    found
}

/// True if the choice body, on its `Expr` tree, guarantees the list `param` is
/// NON-EMPTY — by ASSERTING a strict lower bound on its length (`length p > 0`,
/// `length p >= 1`, `0 < length p`, `length p /= 0`) or `not (null p)` /
/// `not $ null p`. Decided structurally on the assert conditions, so an UPPER
/// bound (`length p < 10`) never satisfies it and a check naming a DIFFERENT,
/// similarly-prefixed list (`inputHoldingCidsBackup`) never aliases onto `p`.
fn has_nonempty_list_check(body: &[Statement], param: &str) -> bool {
    let mut found = false;
    crate::ir::walk_body_stmts(body, &mut |s| {
        if let Statement::Assert { condition_expr, .. } = s {
            if crate::ir::expr_guarantees_nonempty(condition_expr, param) {
                found = true;
            }
        }
    });
    found
}

/// True if the body asserts an UPPER bound on `length/size <field>` (a max-count
/// check such as `length transfer.inputHoldingCids < maxNumInputs`) without also
/// asserting a non-empty lower bound on the same field. This is the H2
/// anti-pattern: an attacker can still pass the empty list. Decided structurally.
fn has_max_only_count_check(body: &[Statement], field: &str) -> bool {
    let mut has_upper = false;
    let mut has_lower = false;
    crate::ir::walk_body_stmts(body, &mut |s| {
        if let Statement::Assert { condition_expr, .. } = s {
            if crate::ir::expr_has_size_upper_bound(condition_expr, field) {
                has_upper = true;
            }
            if crate::ir::expr_guarantees_nonempty(condition_expr, field) {
                has_lower = true;
            }
        }
    });
    has_upper && !has_lower
}

/// Detector #5: missing-positive-amount
///
/// For each choice that accepts a Decimal/Numeric PARAMETER whose name mentions
/// `amount` (or is `quantity` / `price`), check that the choice body guarantees
/// it is strictly positive — by asserting `amount > 0` or by a defensive guard
/// that rejects the non-positive case. Also check for list parameters named
/// inputHoldingCids or similar — flag if there is no min-length check.
/// (Record-field amounts nested inside a parameter are out of scope.)
///
/// Catches: G2 (missing positive-amount check), H2 (zero-input transfer)
pub struct MissingPositiveAmount;

impl Detector for MissingPositiveAmount {
    fn name(&self) -> &str {
        "missing-positive-amount"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn description(&self) -> &str {
        "Choice accepts amount/transfer parameter without positive-value or non-empty check"
    }

    fn detect(&self, module: &DamlModule) -> Vec<Finding> {
        let mut findings = Vec::new();

        for template in &module.templates {
            for choice in &template.choices {
                // Check for Decimal parameters named "amount" or containing "amount"
                let amount_params: Vec<_> = choice
                    .parameters
                    .iter()
                    .filter(|p| {
                        p.daml_type.is_decimal()
                            && (p.name.to_lowercase().contains("amount")
                                || p.name.to_lowercase() == "quantity"
                                || p.name.to_lowercase() == "price")
                    })
                    .collect();

                for param in &amount_params {
                    // Only STRICT positivity counts — `amount >= 0` permits a
                    // zero-amount exercise (the H2 vuln). Decided on the tree:
                    // a guard must be ASSERTED or a defensive rejection, not a
                    // mere mention of `amount > 0` somewhere in the body.
                    let has_check = has_positive_amount_check(&choice.body, &param.name);

                    if !has_check {
                        findings.push(Finding {
                            detector: self.name().to_string(),
                            severity: self.severity(),
                            file: choice.span.file.clone(),
                            line: choice.span.line,
                            column: choice.span.column,
                            message: format!(
                                "Choice '{}' accepts Decimal parameter '{}' without asserting > 0.",
                                choice.name, param.name
                            ),
                            evidence: format!(
                                "{} : Decimal  -- no positive-amount check",
                                param.name
                            ),
                        });
                    }
                }

                // Check for list parameters that should be non-empty
                // Look for patterns like inputHoldingCids, inputs, etc.
                let list_params: Vec<_> = choice
                    .parameters
                    .iter()
                    .filter(|p| {
                        p.daml_type.is_list()
                            && (p.name.to_lowercase().contains("input")
                                || p.name.to_lowercase().contains("holding")
                                || p.name.to_lowercase().contains("cids"))
                    })
                    .collect();

                for param in &list_params {
                    // The check must name THIS parameter and actually establish a
                    // NON-EMPTY guarantee — a strict lower bound on its length
                    // (`length p > 0`, `>= 1`, `0 < length p`, `length p /= 0`) or
                    // `not (null p)` / `not $ null p`. Decided structurally on the
                    // assert conditions, so an UPPER bound (`length p < 10`) and a
                    // check on a different, similarly-prefixed list
                    // (`inputHoldingCidsBackup`) never suppress this finding.
                    let has_nonempty_check = has_nonempty_list_check(&choice.body, &param.name);

                    if !has_nonempty_check {
                        findings.push(Finding {
                            detector: self.name().to_string(),
                            severity: self.severity(),
                            file: choice.span.file.clone(),
                            line: choice.span.line,
                            column: choice.span.column,
                            message: format!(
                                "Choice '{}' accepts list parameter '{}' but has no minimum-length check.",
                                choice.name, param.name
                            ),
                            evidence: format!("No 'not $ null {}' or min-length check", param.name),
                        });
                    }
                }

                // The input list often lives on a record field threaded through
                // the choice (`transfer.inputHoldingCids`, `ctx.inputs`) rather
                // than as a direct parameter. A choice that bounds that count
                // only from ABOVE (`length transfer.inputHoldingCids <
                // maxNumInputs`) still admits the empty list — the H2
                // zero-input vuln. Decided structurally on the assert tree, so
                // the idiomatic `length`-spelled max check is caught exactly like
                // the `size`-spelled one, while a genuine min+max stays clean.
                for field in ["transfer.inputHoldingCids", "transfer.inputs"] {
                    if has_max_only_count_check(&choice.body, field) {
                        findings.push(Finding {
                            detector: self.name().to_string(),
                            severity: self.severity(),
                            file: choice.span.file.clone(),
                            line: choice.span.line,
                            column: choice.span.column,
                            message: format!(
                                "Choice '{}' checks max input count but not min. Empty inputs allowed.",
                                choice.name,
                            ),
                            evidence: format!(
                                "Bounds '{field}' from above but never asserts it is non-empty"
                            ),
                        });
                    }
                }
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
    fn test_missing_positive_amount_triggers() {
        let source = r#"module Test where

template Token
  with
    owner : Party
  where
    signatory owner

    choice Transfer : ContractId Token
      with
        amount : Decimal
        newOwner : Party
      controller owner
      do
        create this with owner = newOwner
"#;
        let module = parse_daml(source, Path::new("Token.daml"));
        let findings = MissingPositiveAmount.detect(&module);
        assert!(!findings.is_empty());
        assert!(findings[0].message.contains("amount"));
    }

    #[test]
    fn test_positive_amount_check_passes() {
        let source = r#"module Test where

template Token
  with
    owner : Party
  where
    signatory owner

    choice Transfer : ContractId Token
      with
        amount : Decimal
        newOwner : Party
      controller owner
      do
        assertMsg "amount must be positive" (amount > 0.0)
        create this with owner = newOwner
"#;
        let module = parse_daml(source, Path::new("Token.daml"));
        let findings = MissingPositiveAmount.detect(&module);
        assert!(findings.is_empty());
    }

    fn choice_with_amount_guard(guard_line: &str, ty: &str) -> Vec<Finding> {
        let source = format!(
            r#"module Test where

template Token
  with
    owner : Party
  where
    signatory owner

    choice Transfer : ()
      with
        amount : {ty}
      controller owner
      do
        {guard_line}
        pure ()
"#
        );
        let module = parse_daml(&source, Path::new("Token.daml"));
        MissingPositiveAmount.detect(&module)
    }

    // Regression (sweep F10): `amount >= 0` permits zero (the H2 vuln) — must flag.
    #[test]
    fn test_ge_zero_amount_is_flagged() {
        assert!(
            !choice_with_amount_guard("assertMsg \"nn\" (amount >= 0.0)", "Decimal").is_empty()
        );
    }

    // Regression (sweep F13): the idiomatic flipped guard `0.0 < amount` is valid.
    #[test]
    fn test_flipped_positive_guard_passes() {
        assert!(choice_with_amount_guard("assertMsg \"pos\" (0.0 < amount)", "Decimal").is_empty());
    }

    // Regression (sweep F12): a guard on `xamount` must not be read as a guard on `amount`.
    #[test]
    fn test_substring_param_guard_does_not_suppress() {
        assert!(!choice_with_amount_guard("assertMsg \"x\" (xamount > 0.0)", "Decimal").is_empty());
    }

    // Regression (sweep F11/F28): a `> 0` mention in a comment is not a guard.
    #[test]
    fn test_comment_guard_does_not_suppress() {
        assert!(
            !choice_with_amount_guard("-- amount > 0 is checked elsewhere", "Decimal").is_empty()
        );
    }

    // Regression (sweep F9): a Numeric n amount param is the modern money type
    // and must be checked.
    #[test]
    fn test_numeric_amount_is_checked() {
        assert!(!choice_with_amount_guard("pure ()", "Numeric 10").is_empty());
    }

    // Regression (round-3 F14): a defensive `when (amount <= 0) abort` rejects
    // the non-positive case and is a valid guard — not a false positive.
    #[test]
    fn test_when_nonpositive_abort_is_a_guard() {
        assert!(choice_with_amount_guard(
            "when (amount <= 0.0) (abort \"must be positive\")",
            "Decimal"
        )
        .is_empty());
    }

    // Regression (round-3 F14): the `unless (amount > 0) abort` form too.
    #[test]
    fn test_unless_positive_abort_is_a_guard() {
        assert!(
            choice_with_amount_guard("unless (amount > 0.0) (abort \"bad\")", "Decimal").is_empty()
        );
    }

    // `when (amount < 0) abort` rejects only negatives — zero still slips
    // through (the H2 vuln), so it must NOT count as a guard.
    #[test]
    fn test_when_strict_negative_abort_is_not_enough() {
        assert!(
            !choice_with_amount_guard("when (amount < 0.0) (abort \"bad\")", "Decimal").is_empty()
        );
    }

    // Regression (audit round-3): the `if amount <= 0 then abort else ...`
    // defensive form is recognized.
    #[test]
    fn test_if_nonpositive_abort_is_a_guard() {
        assert!(choice_with_amount_guard(
            "if amount <= 0.0 then abort \"bad\" else pure ()",
            "Decimal"
        )
        .is_empty());
    }

    // Regression (audit round-3): extra whitespace in the guard does not matter —
    // the check is on the tree, not the text.
    #[test]
    fn test_extra_whitespace_guard_passes() {
        assert!(
            choice_with_amount_guard("assertMsg \"pos\" (amount  >  0.0)", "Decimal").is_empty()
        );
    }

    // Regression (audit round-3): a positive floor `amount >= 0.01` excludes
    // zero, so it IS a valid strict-positive guard.
    #[test]
    fn test_positive_floor_guard_passes() {
        assert!(
            choice_with_amount_guard("assertMsg \"floor\" (amount >= 0.01)", "Decimal").is_empty()
        );
    }

    // Regression (audit round-3): a NON-asserting mention (`let isPos = ...`)
    // does not guard anything — it must still be flagged (was a false negative).
    #[test]
    fn test_non_asserting_mention_does_not_suppress() {
        assert!(
            !choice_with_amount_guard("let isPos = amount > 0.0", "Decimal").is_empty(),
            "a let-binding that merely computes amount > 0 is not a guard"
        );
    }

    // Regression (finding 3): a module-qualified `DA.Assert.assertMsg
    // (amount > 0.0)` is the same guard as the bare form — the choice must NOT
    // be flagged.
    #[test]
    fn test_qualified_assert_is_a_positive_guard() {
        assert!(
            choice_with_amount_guard("DA.Assert.assertMsg \"p\" (amount > 0.0)", "Decimal")
                .is_empty(),
            "DA.Assert.assertMsg is the stdlib guard and must suppress"
        );
        assert!(
            choice_with_amount_guard("Assert.assertMsg \"p\" (amount > 0.0)", "Decimal").is_empty(),
            "Assert.assertMsg is the stdlib guard and must suppress"
        );
    }

    // Regression (finding 4): an `amount > 0` assert that runs only `when isLarge`
    // does not protect the `isLarge == False` path, so the choice must be flagged.
    #[test]
    fn test_conditional_when_assert_does_not_suppress() {
        assert!(
            !choice_with_amount_guard("when isLarge (assertMsg \"p\" (amount > 0.0))", "Decimal")
                .is_empty(),
            "an assert gated on `when isLarge` leaves the False path unchecked"
        );
    }

    // Regression (finding 18): an `amount > 0` assert in only the THEN arm of an
    // `if flag` leaves flag=False with a zero amount unchecked — must flag.
    #[test]
    fn test_conditional_if_then_assert_does_not_suppress() {
        let source = r#"module Test where

template Token
  with
    owner : Party
  where
    signatory owner

    choice Transfer : ()
      with
        amount : Decimal
        flag : Bool
      controller owner
      do
        if flag
          then assertMsg "pos" (amount > 0.0)
          else pure ()
        create this with owner = owner
"#;
        let module = parse_daml(source, Path::new("Token.daml"));
        assert!(
            !MissingPositiveAmount.detect(&module).is_empty(),
            "an assert only on the then-arm of `if flag` does not guarantee amount > 0"
        );
    }

    // Counter-case (finding 4/18): `if amount <= 0 then abort else pure ()` is a
    // defensive guard that rejects the bad case on EVERY path — still clean.
    #[test]
    fn test_if_nonpositive_abort_both_paths_is_a_guard() {
        assert!(
            choice_with_amount_guard(
                "if amount <= 0.0 then abort \"bad\" else pure ()",
                "Decimal"
            )
            .is_empty(),
            "rejecting amount <= 0 on the abort path is a real guard"
        );
    }

    // ----- list / min-length guard regressions ---------------------------

    /// A choice taking one list param `inputHoldingCids` (and an optional second
    /// `inputHoldingCidsBackup`) guarded by `guard_line`.
    fn choice_with_list_guard(guard_line: &str, extra_param: &str) -> Vec<Finding> {
        let source = format!(
            r#"module Test where

template Batch
  with
    owner : Party
  where
    signatory owner

    choice Exec : ()
      with
        inputHoldingCids : [ContractId Token]
{extra_param}      controller owner
      do
        {guard_line}
        pure ()
"#
        );
        let module = parse_daml(&source, Path::new("Batch.daml"));
        MissingPositiveAmount.detect(&module)
    }

    // Regression (audit finding 0): a `length p < N` UPPER bound does NOT bound
    // the empty list — length [] = 0 < N passes — so the min-length finding must
    // still fire.
    #[test]
    fn test_list_upper_bound_does_not_suppress() {
        assert!(
            !choice_with_list_guard("assertMsg \"max\" (length inputHoldingCids < 10)", "")
                .is_empty(),
            "`length p < 10` is an upper bound; the empty list still passes"
        );
        assert!(
            !choice_with_list_guard("assertMsg \"max\" (length inputHoldingCids <= 10)", "")
                .is_empty(),
            "`length p <= 10` is an upper bound; the empty list still passes"
        );
    }

    // Regression (audit finding 0): a genuine `length p > 0` min-length check is
    // a valid non-empty guard and must suppress the finding.
    #[test]
    fn test_list_strict_lower_bound_passes() {
        assert!(
            choice_with_list_guard("assertMsg \"ne\" (length inputHoldingCids > 0)", "").is_empty(),
            "`length p > 0` proves non-empty"
        );
    }

    // Regression (audit finding 1): a min-length / null check on a DIFFERENT,
    // similarly-prefixed list (`inputHoldingCidsBackup`) must NOT suppress the
    // finding on `inputHoldingCids` (substring collision).
    #[test]
    fn test_superstring_list_check_does_not_suppress() {
        let findings = choice_with_list_guard(
            "assertMsg \"ne\" (not (null inputHoldingCidsBackup))",
            "        inputHoldingCidsBackup : [ContractId Token]\n",
        );
        assert!(
            findings
                .iter()
                .any(|f| f.message.contains("inputHoldingCids")
                    && f.message.contains("minimum-length")),
            "a check on inputHoldingCidsBackup does not guard inputHoldingCids"
        );
    }

    // Regression (audit finding 1): `not (null p)` and `not $ null p` are valid
    // non-empty guards on THIS param.
    #[test]
    fn test_not_null_passes() {
        assert!(
            choice_with_list_guard("assertMsg \"ne\" (not (null inputHoldingCids))", "").is_empty()
        );
        assert!(
            choice_with_list_guard("assertMsg \"ne\" (not $ null inputHoldingCids)", "").is_empty()
        );
    }

    /// A settlement choice whose input list lives on the `transfer` record field,
    /// guarded by `guard_line`.
    fn settlement_with_guard(guard_line: &str) -> Vec<Finding> {
        let source = format!(
            r#"module Test where

template Settlement
  with
    owner : Party
    transfer : TransferData
  where
    signatory owner

    choice Execute : ()
      with
        ctx : Context
      controller owner
      do
        {guard_line}
        pure ()
"#
        );
        let module = parse_daml(&source, Path::new("Settlement.daml"));
        MissingPositiveAmount.detect(&module)
    }

    // Regression (audit finding 2): `length transfer.inputHoldingCids <
    // maxNumInputs` is a max-only check — the empty list still passes — so the
    // zero-input finding must fire, exactly like the `size`-spelled form.
    #[test]
    fn test_transfer_length_max_only_is_flagged() {
        assert!(
            !settlement_with_guard(
                "assertMsg \"max\" (length transfer.inputHoldingCids < maxNumInputs)"
            )
            .is_empty(),
            "`length transfer.inputHoldingCids < maxNumInputs` is max-only"
        );
    }

    // Regression (audit finding 2): the flipped, `size`-spelled equivalent flags
    // identically.
    #[test]
    fn test_transfer_size_max_only_is_flagged() {
        assert!(!settlement_with_guard(
            "assertMsg \"max\" (maxNumInputs > size transfer.inputHoldingCids)"
        )
        .is_empty());
    }

    // Regression (audit finding 2): a genuine min+max guard — the lower bound
    // proves non-empty, so the choice is clean.
    #[test]
    fn test_transfer_min_and_max_is_clean() {
        assert!(
            settlement_with_guard(
                "assertMsg \"ne\" (length transfer.inputHoldingCids > 0 && length transfer.inputHoldingCids < maxNumInputs)"
            )
            .is_empty(),
            "a real `length > 0` lower bound alongside the max check is safe"
        );
    }
}
