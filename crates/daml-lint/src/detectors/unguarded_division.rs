use crate::detector::{Detector, Finding, Severity};
use crate::ir::{DamlModule, Statement};

/// Detector #2: unguarded-division
///
/// Find all division expressions (/ operator or div function) in choice bodies
/// and functions. Walk backward through the statement list to find a prior
/// assertMsg or ensure check that bounds the denominator to > 0. Flag divisions
/// where no such guard exists.
///
/// Catches: M11 (amuletPrice division), M12 (capPerCoupon division)
pub struct UnguardedDivision;

impl UnguardedDivision {
    fn check_body_raw(
        &self,
        body_raw: &str,
        statements: &[Statement],
        file: &std::path::Path,
        base_line: usize,
        context_name: &str,
    ) -> Vec<Finding> {
        let mut findings = Vec::new();
        let lines: Vec<&str> = body_raw.lines().collect();

        for (line_idx, line) in lines.iter().enumerate() {
            let trimmed = line.trim();
            // Comment text (ASCII diagrams full of slashes) is not code.
            let trimmed = match trimmed.find("--") {
                Some(0) => continue,
                Some(idx) => trimmed[..idx].trim_end(),
                None => trimmed,
            };

            // Every division on this line (paren-aware; skips numeric-literal
            // denominators and numeric-conversion wrappers), so a second
            // division like `c / d` in `a / b + c / d` is not missed.
            let mut seen = std::collections::HashSet::new();
            for denominator in denominators_on_line(trimmed) {
                if denominator.is_empty() || !seen.insert(denominator.clone()) {
                    continue;
                }
                let has_guard =
                    self.has_prior_guard(&denominator, statements, body_raw, base_line, line_idx);
                if !has_guard {
                    findings.push(Finding {
                        detector: self.name().to_string(),
                        severity: self.severity(),
                        file: file.to_path_buf(),
                        line: base_line + line_idx,
                        column: 1,
                        message: format!(
                            "Unguarded division by '{}' — no prior > 0 check found in {}.",
                            denominator, context_name
                        ),
                        evidence: trimmed.to_string(),
                    });
                }
            }
        }

        findings
    }

    fn has_prior_guard(
        &self,
        denominator: &str,
        statements: &[Statement],
        body_raw: &str,
        base_line: usize,
        division_line: usize,
    ) -> bool {
        if denominator.is_empty() {
            return false;
        }
        // A guard only counts if it runs BEFORE the division. `division_abs` is
        // the division's real source line (base_line + its body-relative line).
        let division_abs = base_line + division_line;

        // `>= 0` is intentionally NOT accepted: y == 0 satisfies it yet still
        // divides by zero. Only strict-positive (`> 0`) or explicit non-zero
        // (`/= 0`, the Daml idiom) checks guard a division.
        let is_positivity = |s: &str| s.contains("> 0") || s.contains("/= 0") || s.contains("!= 0");

        // Structured asserts that appear strictly before the division. Match the
        // denominator as a WHOLE identifier so a guard on `quantity` does not
        // masquerade as a guard on `q`.
        for stmt in statements {
            if let Statement::Assert {
                condition, span, ..
            } = stmt
            {
                if span.line < division_abs
                    && crate::ir::mentions_ident(condition, denominator)
                    && is_positivity(condition)
                {
                    return true;
                }
            }
        }

        // Raw body lines strictly before the division line.
        let lines: Vec<&str> = body_raw.lines().collect();
        for (i, line) in lines.iter().enumerate() {
            if i >= division_line {
                break;
            }
            let trimmed = line.trim();
            if (trimmed.contains("assertMsg") || trimmed.contains("assert "))
                && crate::ir::mentions_ident(trimmed, denominator)
                && is_positivity(trimmed)
            {
                return true;
            }
        }

        false
    }
}

/// Numeric-conversion wrappers that are pure noise as a "denominator": the
/// value that can actually be zero is their argument. `x / intToDecimal n`
/// divides by `n`, not by the function `intToDecimal`.
const NUMERIC_WRAPPERS: [&str; 2] = ["intToDecimal", "intToNumeric"];

fn is_numeric_literal(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit() || b == b'.')
}

/// Read the operand starting at byte `start`: a balanced `(...)` group, or a run
/// of identifier / `.` / digit chars. None if there is nothing there. Stops at
/// the first non-ASCII byte, so it never slices a multi-byte char.
fn read_operand(line: &str, start: usize) -> Option<String> {
    let b = line.as_bytes();
    if start >= b.len() {
        return None;
    }
    if b[start] == b'(' {
        let mut depth = 0i32;
        let mut k = start;
        while k < b.len() {
            match b[k] {
                b'(' => depth += 1,
                b')' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(line[start..=k].to_string());
                    }
                }
                _ => {}
            }
            k += 1;
        }
        return Some(line[start..].to_string());
    }
    let mut k = start;
    while k < b.len()
        && (b[k].is_ascii_alphanumeric() || b[k] == b'_' || b[k] == b'.' || b[k] == b'\'')
    {
        k += 1;
    }
    if k == start {
        None
    } else {
        Some(line[start..k].to_string())
    }
}

/// The denominator just past a `/` at byte `start`, skipping numeric-conversion
/// wrappers (`x / intToDecimal n` → `n`) and suppressing non-zero numeric-literal
/// denominators (`x / 2.0` is safe; `x / 0` still flags).
fn resolve_denominator(line: &str, start: usize) -> Option<String> {
    let b = line.as_bytes();
    let mut j = start;
    while j < b.len() && b[j] == b' ' {
        j += 1;
    }
    let op = read_operand(line, j)?;
    if NUMERIC_WRAPPERS.contains(&op.as_str()) {
        return resolve_denominator(line, j + op.len());
    }
    if is_numeric_literal(&op) && op != "0" && op != "0.0" && op != "0." {
        return None;
    }
    Some(op)
}

/// Every division denominator on `line` (the `/` operator or the `` `div` ``
/// function), left to right — so multiple divisions on one line are all seen.
fn denominators_on_line(line: &str) -> Vec<String> {
    let b = line.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        // `/` division operator, excluding `/=` (not-equal) and `//`.
        if b[i] == b'/'
            && b.get(i + 1) != Some(&b'=')
            && b.get(i + 1) != Some(&b'/')
            && (i == 0 || b[i - 1] != b'/')
        {
            if let Some(d) = resolve_denominator(line, i + 1) {
                out.push(d);
            }
            i += 1;
            continue;
        }
        // `` `div` `` infix division.
        if b[i] == b'`' && b[i..].starts_with(b"`div`") {
            if let Some(d) = resolve_denominator(line, i + 5) {
                out.push(d);
            }
            i += 5;
            continue;
        }
        i += 1;
    }
    out
}

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
            for choice in &template.choices {
                findings.extend(self.check_body_raw(
                    &choice.body_raw,
                    &choice.body,
                    &module.file,
                    choice.span.line,
                    &format!("choice '{}'", choice.name),
                ));
            }
        }

        for func in &module.functions {
            findings.extend(self.check_body_raw(
                &func.body_raw,
                &func.body,
                &module.file,
                func.span.line,
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
  map (\f -> f { amount = f.amount * (1.0 / rate)) fees
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
