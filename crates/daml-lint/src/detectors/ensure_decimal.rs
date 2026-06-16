use crate::detector::{Detector, Finding, Severity};
use crate::ir::DamlModule;

/// Detector #1: missing-ensure-decimal
///
/// For each template with a Decimal field, check that the template has an ensure
/// clause referencing that field with a > 0 or >= 0 comparison. Flags templates
/// where Decimal fields have no corresponding ensure bound.
///
/// Catches: G1 (missing ensure on monetary templates), M11 (round templates missing ensure)
pub struct MissingEnsureDecimal;

impl Detector for MissingEnsureDecimal {
    fn name(&self) -> &str {
        "missing-ensure-decimal"
    }

    fn severity(&self) -> Severity {
        Severity::High
    }

    fn description(&self) -> &str {
        "Template has Decimal field with no positivity bound in its ensure clause"
    }

    fn detect(&self, module: &DamlModule) -> Vec<Finding> {
        let mut findings = Vec::new();

        for template in &module.templates {
            let decimal_fields: Vec<_> = template
                .fields
                .iter()
                .filter(|f| f.daml_type.is_decimal())
                .collect();

            if decimal_fields.is_empty() {
                continue;
            }

            for field in &decimal_fields {
                let has_bound = template
                    .ensure_clause
                    .as_ref()
                    .is_some_and(|ec| ec.has_positive_bound(&field.name));

                if !has_bound {
                    let evidence = if template.ensure_clause.is_none() {
                        format!("{} : Decimal  -- no ensure clause found", field.name)
                    } else {
                        format!(
                            "{} : Decimal  -- ensure clause does not bound this field",
                            field.name
                        )
                    };

                    findings.push(Finding {
                        detector: self.name().to_string(),
                        severity: self.severity(),
                        file: template.span.file.clone(),
                        line: template.span.line,
                        column: template.span.column,
                        message: format!(
                            "Template '{}' has Decimal field '{}' with no positivity bound (e.g. `{} > 0`) in its ensure clause.",
                            template.name, field.name, field.name
                        ),
                        evidence,
                    });
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
    fn test_missing_ensure_decimal_triggers() {
        let source = r#"module Test where

template OpenMiningRound
  with
    admin : Party
    amuletPrice : Decimal
    tickDuration : Decimal
  where
    signatory admin
"#;
        let module = parse_daml(source, Path::new("Round.daml"));
        let findings = MissingEnsureDecimal.detect(&module);
        assert_eq!(findings.len(), 2);
        assert!(findings[0].message.contains("amuletPrice"));
        assert!(findings[1].message.contains("tickDuration"));
    }

    #[test]
    fn test_ensure_decimal_passes_with_bound() {
        let source = r#"module Test where

template SimpleHolding
  with
    admin : Party
    amount : Decimal
  where
    signatory admin
    ensure amount > 0.0
"#;
        let module = parse_daml(source, Path::new("Holding.daml"));
        let findings = MissingEnsureDecimal.detect(&module);
        assert!(findings.is_empty());
    }

    #[test]
    fn test_ensure_exists_but_doesnt_bound_field() {
        let source = r#"module Test where

template RoundWithPartialEnsure
  with
    admin : Party
    amuletPrice : Decimal
    tickDuration : Decimal
  where
    signatory admin
    ensure tickDuration > 0.0
"#;
        let module = parse_daml(source, Path::new("Round.daml"));
        let findings = MissingEnsureDecimal.detect(&module);
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.contains("amuletPrice"));
    }

    // Regression (sweep F6/F27): a Numeric field is the modern money type and
    // must be checked like Decimal.
    #[test]
    fn test_numeric_field_is_flagged() {
        let source = r#"module Test where

template Round
  with
    admin : Party
    price : Numeric 10
  where
    signatory admin
"#;
        let module = parse_daml(source, Path::new("Round.daml"));
        let findings = MissingEnsureDecimal.detect(&module);
        assert!(findings.iter().any(|f| f.message.contains("'price'")));
    }

    // Regression (round-3 F8): a NEGATED comparison does not bound the field —
    // `not (amount > 0)` guarantees nothing, so amount must still be flagged.
    #[test]
    fn test_negated_bound_does_not_count() {
        let source = r#"module Test where

template T
  with
    admin : Party
    amount : Decimal
  where
    signatory admin
    ensure not (amount > 0.0)
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        let findings = MissingEnsureDecimal.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("'amount'")),
            "negated bound must not satisfy positivity: {:?}",
            findings
        );
    }

    // Regression (audit round-3): a strict lower bound above a positive constant
    // (`amount > 100.0`, a minimum-trade-size) bounds the field positive and
    // must NOT be flagged. A negative bound (`amount > -5.0`) still flags.
    #[test]
    fn test_positive_constant_lower_bound_passes() {
        let tmpl = |ensure: &str| {
            format!(
                "module T where\n\ntemplate M\n  with\n    owner : Party\n    amount : Decimal\n  where\n    signatory owner\n    ensure {}\n",
                ensure
            )
        };
        let ok = parse_daml(&tmpl("amount > 100.0"), Path::new("M.daml"));
        assert!(
            !MissingEnsureDecimal
                .detect(&ok)
                .iter()
                .any(|f| f.message.contains("'amount'")),
            "amount > 100.0 bounds amount positive: {:?}",
            MissingEnsureDecimal.detect(&ok)
        );

        let neg = parse_daml(&tmpl("amount > -5.0"), Path::new("M.daml"));
        assert!(
            MissingEnsureDecimal
                .detect(&neg)
                .iter()
                .any(|f| f.message.contains("'amount'")),
            "a negative lower bound does not guarantee positivity: {:?}",
            MissingEnsureDecimal.detect(&neg)
        );
    }

    // Regression (audit round-3): `amount == <positive literal>` pins the field
    // to a positive constant and must NOT be flagged, written either way round.
    // `== 0.0` still admits zero and `== -5.0` (a Neg(Lit)) is negative, so both
    // keep flagging.
    #[test]
    fn test_equality_to_positive_constant_passes() {
        let tmpl = |ensure: &str| {
            format!(
                "module T where\n\ntemplate M\n  with\n    owner : Party\n    amount : Decimal\n  where\n    signatory owner\n    ensure {}\n",
                ensure
            )
        };
        let flags = |ensure: &str| {
            let m = parse_daml(&tmpl(ensure), Path::new("M.daml"));
            MissingEnsureDecimal
                .detect(&m)
                .iter()
                .any(|f| f.message.contains("'amount'"))
        };
        assert!(
            !flags("amount == 5.0"),
            "amount == 5.0 pins amount positive"
        );
        assert!(
            !flags("5.0 == amount"),
            "5.0 == amount pins amount positive"
        );
        assert!(flags("amount == -5.0"), "== -5.0 is negative, still flags");
        assert!(flags("amount == 0.0"), "== 0.0 admits zero, still flags");
    }

    // Regression (round-3 F8): a comparison under `||` is not guaranteed.
    #[test]
    fn test_disjunction_bound_does_not_count() {
        let source = r#"module Test where

template T
  with
    admin : Party
    amount : Decimal
    flag : Bool
  where
    signatory admin
    ensure flag || amount > 0.0
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        let findings = MissingEnsureDecimal.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("'amount'")),
            "a bound under || is not guaranteed: {:?}",
            findings
        );
    }

    // A comparison reached through top-level `&&` IS guaranteed.
    #[test]
    fn test_conjunction_bound_counts() {
        let source = r#"module Test where

template T
  with
    admin : Party
    amount : Decimal
  where
    signatory admin
    ensure amount > 0.0 && admin == admin
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        let findings = MissingEnsureDecimal.detect(&module);
        assert!(
            !findings.iter().any(|f| f.message.contains("'amount'")),
            "amount > 0.0 under && IS a bound: {:?}",
            findings
        );
    }

    // Regression (sweep F29): `count` is not bounded by a `discount > 0` ensure
    // on a different field.
    #[test]
    fn test_substring_field_not_considered_bounded() {
        let source = r#"module Test where

template T
  with
    admin : Party
    count : Decimal
    discount : Decimal
  where
    signatory admin
    ensure discount > 0.0
"#;
        let module = parse_daml(source, Path::new("T.daml"));
        let findings = MissingEnsureDecimal.detect(&module);
        assert!(
            findings.iter().any(|f| f.message.contains("'count'")),
            "count must be flagged: {:?}",
            findings
        );
        assert!(
            !findings.iter().any(|f| f.message.contains("'discount'")),
            "discount IS bounded: {:?}",
            findings
        );
    }
}
