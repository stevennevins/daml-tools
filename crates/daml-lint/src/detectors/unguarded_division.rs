use crate::detector::{Detector, Finding, Severity};
use crate::ir::{DamlModule, EnsureClause, Expr, SrcPos, Statement};
use std::collections::HashSet;

/// Shared, read-only context threaded through the recursive division scan.
struct Ctx<'a> {
    all_stmts: &'a [Statement],
    ensure: Option<&'a EnsureClause>,
    file: &'a std::path::Path,
    context_name: &'a str,
}

/// Detector #2: unguarded-division
///
/// Find every division — the `/` operator, infix `` `div` ``, and prefix
/// `div x y` — in choice bodies and functions, and flag any whose denominator
/// is not bounded away from zero by a prior `assert`/`assertMsg` or the
/// enclosing template `ensure`.
///
/// Decided entirely on the structured `Expr` tree: a `/` inside a string
/// literal or a comment is not a `BinOp`, a line-wrapped division is one node,
/// and the denominator is a real sub-expression (so `intToDecimal n` and
/// `(a + b)` are handled structurally, never as text).
///
/// Catches: M11 (amuletPrice division), M12 (capPerCoupon division)
pub struct UnguardedDivision;

impl UnguardedDivision {
    fn check_body(
        &self,
        statements: &[Statement],
        ensure: Option<&EnsureClause>,
        file: &std::path::Path,
        context_name: &str,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();
        let ctx = Ctx {
            all_stmts: statements,
            ensure,
            file,
            context_name,
        };
        self.scan_stmts(statements, &HashSet::new(), &ctx, &mut findings);
        findings
    }

    /// Walk statements, threading the set of denominator keys an enclosing `if`
    /// has already proven non-zero.
    fn scan_stmts(
        &self,
        stmts: &[Statement],
        guarded: &HashSet<String>,
        ctx: &Ctx,
        findings: &mut Vec<Finding>,
    ) {
        for s in stmts {
            for e in crate::ir::statement_exprs(s) {
                self.scan_expr(e, guarded, ctx, findings);
            }
            match s {
                Statement::TryCatch {
                    try_body,
                    catch_body,
                    ..
                } => {
                    self.scan_stmts(try_body, guarded, ctx, findings);
                    self.scan_stmts(catch_body, guarded, ctx, findings);
                }
                // An `if`/`case` arm is its own scope; scan each for divisions.
                Statement::Branch { arms, .. } => {
                    for arm in arms {
                        self.scan_stmts(&arm.body, guarded, ctx, findings);
                    }
                }
                _ => {}
            }
        }
    }

    fn scan_expr(
        &self,
        e: &Expr,
        guarded: &HashSet<String>,
        ctx: &Ctx,
        findings: &mut Vec<Finding>,
    ) {
        if let Some((denom_expr, span)) = division_denominator(e) {
            let denom_expr = unwrap_numeric_wrapper(denom_expr);
            // `x / 2.0` and `x / (-2.0)` divide by a non-zero constant — safe.
            if !denom_expr.is_nonzero_numeric_divisor() {
                let denom = denom_display(denom_expr);
                // An enclosing `if denom /= 0 then ...` already proved it.
                let if_guarded = matches!(denom_expr.ref_string(), Some(k) if guarded.contains(&k));
                if !if_guarded && !self.has_prior_guard(&denom, ctx.all_stmts, ctx.ensure, span) {
                    findings.push(self.finding(
                        ctx.file,
                        span.line,
                        &denom,
                        ctx.context_name,
                        &e.render_text(),
                    ));
                }
            }
        }

        match e {
            // `if denom /= 0 then x / denom else fallback`: the then-branch holds
            // the condition; the else-branch holds its negation (so `if denom == 0
            // then fallback else x / denom` is guarded too).
            Expr::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => {
                self.scan_expr(cond, guarded, ctx, findings);
                let mut then_guarded = guarded.clone();
                collect_nonzero_keys(cond, &mut then_guarded);
                self.scan_expr(then_branch, &then_guarded, ctx, findings);
                let mut else_guarded = guarded.clone();
                collect_else_nonzero_keys(cond, &mut else_guarded);
                self.scan_expr(else_branch, &else_guarded, ctx, findings);
            }
            Expr::DoBlock { statements, .. } => self.scan_stmts(statements, guarded, ctx, findings),
            _ => {
                for c in crate::ir::child_exprs(e) {
                    self.scan_expr(c, guarded, ctx, findings);
                }
            }
        }
    }

    fn finding(
        &self,
        file: &std::path::Path,
        line: usize,
        denominator: &str,
        context_name: &str,
        evidence: &str,
    ) -> Finding {
        Finding {
            detector: self.name().to_string(),
            severity: self.severity(),
            file: file.to_path_buf(),
            line,
            column: 1,
            message: format!(
                "Unguarded division by '{}' — no prior > 0 check found in {}.",
                denominator, context_name
            ),
            evidence: evidence.to_string(),
        }
    }

    /// True if some `assert`/`assertMsg` that runs strictly before `division`,
    /// or the enclosing template `ensure`, structurally bounds `denominator`
    /// away from zero. Matching is on the assert's `condition_expr`, so a check
    /// buried under `||` / `not (...)` cannot masquerade as a guard.
    ///
    /// "Before" is decided on the `(line, column)` source position, not the line
    /// alone: in a one-line do-block (`do { assertMsg ...; pure (x / y) }`) the
    /// guard and the division share a line, and the earlier column is the one
    /// that runs first. A guard placed *after* the division (later column on the
    /// same line, or a later line) still does not suppress.
    fn has_prior_guard(
        &self,
        denominator: &str,
        statements: &[Statement],
        ensure: Option<&EnsureClause>,
        division: &SrcPos,
    ) -> bool {
        let denominator = denominator.trim();
        if denominator.is_empty() {
            return false;
        }

        // The ensure clause holds before the choice body runs.
        if ensure.is_some_and(|ec| ec.guarantees_nonzero(denominator)) {
            return true;
        }

        let division_pos = (division.line, division.column);
        let mut guarded = false;
        // Only asserts that UNCONDITIONALLY run before the division can guard it.
        // An assert inside a `Branch` arm runs only when that arm is taken, so it
        // never dominates a later division — do not descend into Branch arms.
        crate::ir::walk_unconditional_stmts(statements, &mut |stmt| {
            if let Statement::Assert {
                condition_expr,
                span,
                ..
            } = stmt
            {
                if (span.line, span.column) < division_pos
                    && crate::ir::expr_guarantees_nonzero(condition_expr, denominator)
                {
                    guarded = true;
                }
            }
        });
        guarded
    }
}

/// Add every key that `cond` proves non-zero (under top-level `&&`) — so a
/// division by that key inside the then-branch of `if cond` is guarded.
fn collect_nonzero_keys(cond: &Expr, out: &mut HashSet<String>) {
    for c in cond.conjuncts() {
        if let Expr::BinOp { lhs, rhs, .. } = c {
            for operand in [lhs.as_ref(), rhs.as_ref()] {
                if let Some(k) = operand.ref_string() {
                    if crate::ir::is_nonzero_bound(c, &k) {
                        out.insert(k);
                    }
                }
            }
        }
    }
}

/// Add keys the NEGATION of `cond` proves non-zero — the else-branch case. The
/// useful idiom is `if denom == 0 then fallback else x / denom`: when `denom ==
/// 0` is false, `denom` is non-zero.
fn collect_else_nonzero_keys(cond: &Expr, out: &mut HashSet<String>) {
    if let Expr::BinOp { op, lhs, rhs, .. } = cond {
        if op == "==" {
            if lhs.is_zero_lit() {
                if let Some(k) = rhs.ref_string() {
                    out.insert(k);
                }
            } else if rhs.is_zero_lit() {
                if let Some(k) = lhs.ref_string() {
                    out.insert(k);
                }
            }
        }
    }
}

/// The denominator and source position of a division expression, if `e` is one:
/// the `/` operator, infix `` `div` ``, or prefix `div x y` (denominator is the
/// second argument).
fn division_denominator(e: &Expr) -> Option<(&Expr, &SrcPos)> {
    match e {
        Expr::BinOp { op, rhs, span, .. } if op == "/" || op == "`div`" => Some((rhs, span)),
        Expr::App { func, args, span } if args.len() >= 2 => match func.as_ref() {
            Expr::Var {
                name,
                qualifier: None,
                ..
            } if name == "div" => Some((&args[1], span)),
            _ => None,
        },
        _ => None,
    }
}

/// Render a denominator for the message, parenthesizing a compound expression
/// so `x / (a + b)` reports `(a + b)`, not a bare `a + b`.
fn denom_display(e: &Expr) -> String {
    match e {
        Expr::Var { .. } | Expr::Con { .. } | Expr::Lit { .. } => e.render_text(),
        // A record projection `a.b` is atomic enough to show unwrapped.
        Expr::BinOp { op, .. } if op == "." => e.render_text(),
        _ => format!("({})", e.render_text()),
    }
}

/// Peel a numeric-conversion wrapper off a denominator: `intToDecimal n`
/// divides by `n`, not by the function.
fn unwrap_numeric_wrapper(e: &Expr) -> &Expr {
    if let Expr::App { func, args, .. } = e {
        if let Expr::Var { name, .. } = func.as_ref() {
            if NUMERIC_WRAPPERS.contains(&name.as_str()) && args.len() == 1 {
                return unwrap_numeric_wrapper(&args[0]);
            }
        }
    }
    e
}

/// Numeric-conversion wrappers that are pure noise as a "denominator": the
/// value that can actually be zero is their argument. `x / intToDecimal n`
/// divides by `n`, not by the function `intToDecimal`.
const NUMERIC_WRAPPERS: [&str; 2] = ["intToDecimal", "intToNumeric"];

impl Detector for UnguardedDivision {
    fn name(&self) -> &str {
        "unguarded-division"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn description(&self) -> &str {
        "Division without prior > 0 check on denominator"
    }

    fn detect(&self, module: &DamlModule) -> Vec<Finding> {
        let mut findings = Vec::new();

        for template in &module.templates {
            // A denominator bounded by the template `ensure` is guarded before
            // any choice body runs.
            let ensure = template.ensure_clause.as_ref();
            for choice in &template.choices {
                findings.extend(self.check_body(
                    &choice.body,
                    ensure,
                    &module.file,
                    &format!("choice '{}'", choice.name),
                ));
            }
        }

        for func in &module.functions {
            findings.extend(self.check_body(
                &func.body,
                None,
                &module.file,
                &format!("function '{}'", func.name),
            ));
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
    fn test_unguarded_division_triggers() {
        let source = r#"module Test where

scaleFees fees rate =
  map (\f -> f with amount = f.amount * (1.0 / rate)) fees
"#;
        let module = parse_daml(source, Path::new("AmuletRules.daml"));
        let findings = UnguardedDivision.detect(&module);
        assert!(!findings.is_empty());
    }

    #[test]
    fn test_guarded_division_passes() {
        let source = r#"module Test where

safeDivide x y = do
  assertMsg "denominator must be positive" (y > 0)
  pure (x / y)
"#;
        let module = parse_daml(source, Path::new("Safe.daml"));
        let findings = UnguardedDivision.detect(&module);
        assert!(findings.is_empty());
    }

    // Regression: `x / intToDecimal n` divides by `n`, not by the wrapper
    // function. The reported denominator and the guard search must target `n`.
    #[test]
    fn test_inttodecimal_wrapper_reports_real_denominator() {
        let source = r#"module Test where

dayCount total n = total / intToDecimal n
"#;
        let module = parse_daml(source, Path::new("DayCount.daml"));
        let findings = UnguardedDivision.detect(&module);
        assert_eq!(findings.len(), 1);
        assert!(
            findings[0].message.contains("'n'"),
            "expected real denominator 'n', got: {}",
            findings[0].message
        );
    }

    #[test]
    fn test_guarded_inttodecimal_division_passes() {
        let source = r#"module Test where

dayCount total n = do
  assertMsg "n must be positive" (n > 0)
  pure (total / intToDecimal n)
"#;
        let module = parse_daml(source, Path::new("DayCount.daml"));
        let findings = UnguardedDivision.detect(&module);
        assert!(
            findings.is_empty(),
            "guard on real denominator should suppress: {:?}",
            findings
        );
    }

    // Regression (audit F3): a guard placed AFTER the division does not prevent
    // division by zero — the check runs too late. Must still be flagged.
    #[test]
    fn test_guard_after_division_is_flagged() {
        let source = r#"module Test where

unsafeDivide x y = do
  pure (x / y)
  assertMsg "denominator must be positive" (y > 0)
"#;
        let module = parse_daml(source, Path::new("Late.daml"));
        let findings = UnguardedDivision.detect(&module);
        assert!(
            !findings.is_empty(),
            "a guard after the division must NOT suppress the finding"
        );
    }

    // Regression (sweep F1): a guard on `quantity` must NOT suppress a division
    // by `q` — the denominator must match as a whole identifier.
    #[test]
    fn test_substring_guard_does_not_suppress() {
        let source = r#"module Test where

compute x q = do
  assertMsg "quantity" (quantity > 0)
  pure (x / q)
"#;
        let module = parse_daml(source, Path::new("Substr.daml"));
        let findings = UnguardedDivision.detect(&module);
        assert!(
            !findings.is_empty(),
            "a guard on `quantity` must not be read as a guard on `q`"
        );
    }

    // Regression (audit F4): `y >= 0` is not a division guard — y == 0 still
    // passes the check and divides. Only `> 0` / `/= 0` count.
    #[test]
    fn test_ge_zero_is_not_a_guard() {
        let source = r#"module Test where

divCheck x y = do
  assertMsg "non-negative" (y >= 0)
  pure (x / y)
"#;
        let module = parse_daml(source, Path::new("GeZero.daml"));
        let findings = UnguardedDivision.detect(&module);
        assert!(
            !findings.is_empty(),
            "`>= 0` allows zero, so it must not suppress the division finding"
        );
    }

    // Regression (sweep F2): every division on a line is analyzed, so a guarded
    // first division does not mask a later unguarded one.
    #[test]
    fn test_second_division_on_line_is_flagged() {
        let source = r#"module Test where

compute a b c d = do
  assertMsg "b ok" (b > 0)
  pure (a / b + c / d)
"#;
        let module = parse_daml(source, Path::new("Multi.daml"));
        let findings = UnguardedDivision.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("'d'")),
            "the unguarded `c / d` must be flagged: {:?}",
            findings
        );
    }

    // Regression (sweep F26): division by a non-zero numeric literal is safe and
    // must not be flagged; division by a literal 0 still flags.
    #[test]
    fn test_literal_denominator_handling() {
        let safe = parse_daml("module T where\nf x = x / 2.0\n", Path::new("Lit.daml"));
        assert!(
            UnguardedDivision.detect(&safe).is_empty(),
            "x / 2.0 is safe"
        );

        let zero = parse_daml("module T where\nf x = x / 0\n", Path::new("Zero.daml"));
        assert!(
            !UnguardedDivision.detect(&zero).is_empty(),
            "x / 0 must flag"
        );
    }

    // Regression (audit round-3): a `/` inside a STRING LITERAL is not a
    // division — a URL or path in a log line must not be flagged.
    #[test]
    fn test_slash_in_string_literal_is_not_division() {
        let source = r#"module Test where

logUrl = debug "http://host/api/v1/data"
"#;
        let module = parse_daml(source, Path::new("Url.daml"));
        assert!(
            UnguardedDivision.detect(&module).is_empty(),
            "slashes inside a string literal are not divisions: {:?}",
            UnguardedDivision.detect(&module)
        );
    }

    // Regression (audit round-3): a `/` inside a comment is not a division.
    #[test]
    fn test_slash_in_comment_is_not_division() {
        let source = r#"module Test where

f x = do
  {- ratio a/b/c is documented elsewhere -}
  pure x -- see n/m below
"#;
        let module = parse_daml(source, Path::new("Cmt.daml"));
        assert!(
            UnguardedDivision.detect(&module).is_empty(),
            "slashes inside comments are not divisions: {:?}",
            UnguardedDivision.detect(&module)
        );
    }

    // Regression (audit round-3): a division whose operator wraps to the next
    // line is one expression and must still be flagged.
    #[test]
    fn test_line_wrapped_division_is_flagged() {
        let source = "module Test where\n\nratio a b = a /\n  b\n";
        let module = parse_daml(source, Path::new("Wrap.daml"));
        assert!(
            !UnguardedDivision.detect(&module).is_empty(),
            "a line-wrapped `a / b` is still an unguarded division"
        );
    }

    // Regression (audit round-3): division by a parenthesized non-zero numeric
    // constant is safe.
    #[test]
    fn test_parenthesized_literal_denominator_is_safe() {
        let module = parse_daml("module T where\nf x = x / (2.0)\n", Path::new("P.daml"));
        assert!(
            UnguardedDivision.detect(&module).is_empty(),
            "x / (2.0) is safe: {:?}",
            UnguardedDivision.detect(&module)
        );
    }

    // Regression (audit round-3): the defensive `if denom /= 0 then x/denom else
    // fallback` idiom is guarded by its condition — no finding.
    #[test]
    fn test_if_nonzero_guard_suppresses() {
        let m = parse_daml(
            "module T where\nf x denom = pure (if denom /= 0.0 then x / denom else 0.0)\n",
            Path::new("If.daml"),
        );
        assert!(
            UnguardedDivision.detect(&m).is_empty(),
            "if denom /= 0 then x/denom guards the division: {:?}",
            UnguardedDivision.detect(&m)
        );
    }

    // Regression (audit round-3): the flipped form `if denom == 0 then fallback
    // else x/denom` is guarded on the else-branch.
    #[test]
    fn test_if_zero_else_guard_suppresses() {
        let m = parse_daml(
            "module T where\nf x denom = pure (if denom == 0.0 then 0.0 else x / denom)\n",
            Path::new("IfElse.daml"),
        );
        assert!(
            UnguardedDivision.detect(&m).is_empty(),
            "else-branch of `if denom == 0` has denom /= 0: {:?}",
            UnguardedDivision.detect(&m)
        );
    }

    // But an `if` on an UNRELATED condition does not guard the division.
    #[test]
    fn test_if_unrelated_condition_does_not_guard() {
        let m = parse_daml(
            "module T where\nf x denom flag = pure (if flag then x / denom else 0.0)\n",
            Path::new("IfUnrel.daml"),
        );
        assert!(
            !UnguardedDivision.detect(&m).is_empty(),
            "`if flag` says nothing about denom — must still flag"
        );
    }

    // Regression (round-3 F22): the prefix application form `div x y` divides
    // by its SECOND argument and must be flagged like `x / y`.
    #[test]
    fn test_prefix_div_is_flagged() {
        let source = r#"module Test where

share total n = pure (div total n)
"#;
        let module = parse_daml(source, Path::new("Share.daml"));
        let findings = UnguardedDivision.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("'n'")),
            "prefix `div total n` divides by n and must flag: {:?}",
            findings
        );
    }

    // Regression (round-3 F22): a guard on the prefix-div denominator suppresses.
    #[test]
    fn test_guarded_prefix_div_passes() {
        let source = r#"module Test where

share total n = do
  assertMsg "n positive" (n > 0)
  pure (div total n)
"#;
        let module = parse_daml(source, Path::new("Share.daml"));
        assert!(
            UnguardedDivision.detect(&module).is_empty(),
            "a `> 0` guard on n must suppress the prefix-div finding"
        );
    }

    // Regression (round-3 F22): `div x 2` divides by a non-zero literal — safe.
    #[test]
    fn test_prefix_div_literal_denominator_is_safe() {
        let source = "module T where\nf x = pure (div x 2)\n";
        let module = parse_daml(source, Path::new("Lit.daml"));
        assert!(
            UnguardedDivision.detect(&module).is_empty(),
            "div x 2 is safe"
        );
    }

    // Regression (round-3 F24): a denominator bounded by the enclosing template
    // `ensure` clause is guarded before the choice body runs.
    #[test]
    fn test_ensure_clause_guards_choice_division() {
        let source = r#"module Test where

template Pool
  with
    admin : Party
    rate : Decimal
  where
    signatory admin
    ensure rate > 0.0

    choice Share : Decimal
      with
        total : Decimal
      controller admin
      do
        pure (total / rate)
"#;
        let module = parse_daml(source, Path::new("Pool.daml"));
        assert!(
            UnguardedDivision.detect(&module).is_empty(),
            "ensure rate > 0.0 guards the division by rate: {:?}",
            UnguardedDivision.detect(&module)
        );
    }

    // Regression (round-3 F24): a guard hidden under `||` is NOT a guarantee —
    // the division must still be flagged.
    #[test]
    fn test_disjunction_guard_does_not_suppress() {
        let source = r#"module Test where

f x y = do
  assertMsg "weak" (y > 0 || x > 5)
  pure (x / y)
"#;
        let module = parse_daml(source, Path::new("Or.daml"));
        assert!(
            !UnguardedDivision.detect(&module).is_empty(),
            "`y > 0 || x > 5` does not guarantee y > 0, so the division must flag"
        );
    }

    // Regression (audit F10): a guard on the SAME line as the division (a
    // one-line braced do-block) runs before it and must suppress. Ordering is by
    // source position, not line alone, so the earlier-column assert counts.
    #[test]
    fn test_same_line_guard_suppresses() {
        let source = "module Test where\nf x y = do { assertMsg \"y\" (y > 0.0); pure (x / y) }\n";
        let module = parse_daml(source, Path::new("SameLine.daml"));
        assert!(
            UnguardedDivision.detect(&module).is_empty(),
            "a guard before the division on the same line must suppress: {:?}",
            UnguardedDivision.detect(&module)
        );
    }

    // Regression (audit F10): the converse must still flag — a guard placed
    // AFTER the division on the same line runs too late.
    #[test]
    fn test_same_line_guard_after_division_is_flagged() {
        let source = "module Test where\nf x y = do { pure (x / y); assertMsg \"y\" (y > 0.0) }\n";
        let module = parse_daml(source, Path::new("SameLineLate.daml"));
        assert!(
            !UnguardedDivision.detect(&module).is_empty(),
            "a guard after the division on the same line must NOT suppress"
        );
    }

    // Regression (audit F26): division by a non-zero NEGATIVE literal is safe,
    // exactly like `x / 2.0`. The parser spells `-2.0` as `Neg(Lit)`.
    #[test]
    fn test_negative_literal_denominator_is_safe() {
        let infix = parse_daml("module T where\nf x = x / (-2.0)\n", Path::new("Neg.daml"));
        assert!(
            UnguardedDivision.detect(&infix).is_empty(),
            "x / (-2.0) is safe: {:?}",
            UnguardedDivision.detect(&infix)
        );

        let prefix = parse_daml(
            "module T where\nf x = div x (-3)\n",
            Path::new("NegDiv.daml"),
        );
        assert!(
            UnguardedDivision.detect(&prefix).is_empty(),
            "div x (-3) is safe: {:?}",
            UnguardedDivision.detect(&prefix)
        );

        // But `-0.0` is still zero — it must stay flagged.
        let neg_zero = parse_daml(
            "module T where\nf x = x / (-0.0)\n",
            Path::new("NegZero.daml"),
        );
        assert!(
            !UnguardedDivision.detect(&neg_zero).is_empty(),
            "x / (-0.0) divides by zero and must flag"
        );
    }

    // Regression (finding 9/17): a guard inside one arm of an `if`/`case` runs
    // only on that path, so it does NOT guard a later unconditional division.
    #[test]
    fn test_conditional_if_guard_does_not_suppress() {
        let m = parse_daml(
            "module Test where\nf flag x y = do\n  if flag\n    then assertMsg \"y ok\" (y > 0.0)\n    else pure ()\n  pure (x / y)\n",
            Path::new("CondIf.daml"),
        );
        assert!(
            !UnguardedDivision.detect(&m).is_empty(),
            "an assert only on the then-path must not suppress the later x / y"
        );

        let mc = parse_daml(
            "module Test where\nf k x y = do\n  case k of\n    _ -> assertMsg \"y\" (y > 0.0)\n  pure (x / y)\n",
            Path::new("CondCase.daml"),
        );
        assert!(
            !UnguardedDivision.detect(&mc).is_empty(),
            "an assert only in a case alt must not suppress the later x / y"
        );
    }

    // Regression (finding 25): an assert inside a `forA_` lambda runs zero times
    // for an empty list, so it does not prove the denominator non-zero. The
    // `when`-gated form is conditional too.
    #[test]
    fn test_iterative_or_conditional_guard_does_not_suppress() {
        let for_a = parse_daml(
            "module Test where\nf x y items = do\n  forA_ items (\\i -> assertMsg \"y\" (y > 0.0))\n  pure (x / y)\n",
            Path::new("ForA.daml"),
        );
        assert!(
            !UnguardedDivision.detect(&for_a).is_empty(),
            "an assert inside a forA_ lambda runs zero times on []; must still flag"
        );

        let when_gated = parse_daml(
            "module Test where\nf x y b = do\n  when b (assertMsg \"y\" (y > 0.0))\n  pure (x / y)\n",
            Path::new("WhenG.daml"),
        );
        assert!(
            !UnguardedDivision.detect(&when_gated).is_empty(),
            "an assert under `when` runs only when the guard holds; must still flag"
        );
    }

    // Counter-case: an `if cond then x/denom` where cond proves denom non-zero
    // is guarded on that branch and stays clean, and a top-level unconditional
    // assert before a division inside a branch still suppresses.
    #[test]
    fn test_unconditional_guard_before_branch_division_suppresses() {
        let m = parse_daml(
            "module Test where\nf x y b = do\n  assertMsg \"y\" (y > 0.0)\n  if b then pure (x / y) else pure 0.0\n",
            Path::new("Dom.daml"),
        );
        assert!(
            UnguardedDivision.detect(&m).is_empty(),
            "an unconditional guard dominating the branch must suppress: {:?}",
            UnguardedDivision.detect(&m)
        );
    }

    // Regression (sweep F23/F30): a parenthesized denominator is reported whole,
    // not as just its first token.
    #[test]
    fn test_parenthesized_denominator_reported_whole() {
        let module = parse_daml(
            "module T where\nf x y = x / (y + 1)\n",
            Path::new("Paren.daml"),
        );
        let findings = UnguardedDivision.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("(y + 1)")),
            "expected full parenthesized denominator, got: {:?}",
            findings
        );
    }
}
