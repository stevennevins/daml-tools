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

            // Find division: look for / operator or `div` function
            let has_division = trimmed.contains(" / ")
                || trimmed.contains("(1.0 /")
                || trimmed.contains("(1 /")
                || trimmed.contains(" `div` ");

            if !has_division {
                continue;
            }

            // Extract the likely denominator
            let denominator = extract_denominator(trimmed);

            // Check if there's a prior guard in the statements
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

        // Structured asserts that appear strictly before the division.
        for stmt in statements {
            if let Statement::Assert {
                condition, span, ..
            } = stmt
            {
                if span.line < division_abs
                    && condition.contains(denominator)
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
                && trimmed.contains(denominator)
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

fn extract_denominator(line: &str) -> String {
    // Try to extract the expression after / operator
    if let Some(idx) = line.find(" / ") {
        let after = &line[idx + 3..];
        // Skip leading numeric-conversion wrappers so the reported denominator
        // and the guard search target the real (possibly-zero) value.
        for tok in after.split(['(', ')', ' ', '\n']) {
            if tok.is_empty() || NUMERIC_WRAPPERS.contains(&tok) {
                continue;
            }
            return tok.to_string();
        }
        return String::new();
    }
    if let Some(idx) = line.find("(1.0 /") {
        let after = &line[idx + 6..];
        let denom: String = after
            .trim()
            .split([')', '\n'])
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        return denom;
    }
    if let Some(idx) = line.find("(1 /") {
        let after = &line[idx + 4..];
        let denom: String = after
            .trim()
            .split([')', '\n'])
            .next()
            .unwrap_or("")
            .trim()
            .to_string();
        return denom;
    }
    if let Some(idx) = line.find(" `div` ") {
        let after = &line[idx + 7..];
        let denom: String = after.split_whitespace().next().unwrap_or("").to_string();
        return denom;
    }
    String::new()
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
}
