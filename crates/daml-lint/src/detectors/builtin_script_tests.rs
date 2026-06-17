use super::ensure_decimal::MissingEnsureDecimal;
use super::script;
use super::unbounded_fields::UnboundedFields;
use crate::detector::{Detector, Finding, Severity};
use crate::parser::parse_daml;
use std::path::Path;

fn load_rule(name: &str) -> Box<dyn Detector> {
    script::load_script(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("rules")
            .join(name),
    )
    .unwrap()
}

fn snapshot(findings: Vec<Finding>) -> Vec<(String, Severity, usize, usize, String, String)> {
    findings
        .into_iter()
        .map(|f| {
            (
                f.detector, f.severity, f.line, f.column, f.message, f.evidence,
            )
        })
        .collect()
}

fn assert_rule_matches_rust(
    case_name: &str,
    source: &str,
    file: &Path,
    rust_detector: &dyn Detector,
    script_detector: &dyn Detector,
) {
    let module = parse_daml(source, file);
    assert_eq!(
        snapshot(script_detector.detect(&module)),
        snapshot(rust_detector.detect(&module)),
        "TypeScript built-in drifted from Rust detector for {case_name}"
    );
}

#[test]
fn missing_ensure_decimal_script_matches_rust_regressions() {
    let script_detector = load_rule("missing-ensure-decimal.js");
    let rust_detector = MissingEnsureDecimal;

    let simple_cases = [
        (
            "missing ensure reports each decimal field",
            r#"module Test where

template OpenMiningRound
  with
    admin : Party
    amuletPrice : Decimal
    tickDuration : Decimal
  where
    signatory admin
"#,
        ),
        (
            "positive ensure bound suppresses finding",
            r#"module Test where

template SimpleHolding
  with
    admin : Party
    amount : Decimal
  where
    signatory admin
    ensure amount > 0.0
"#,
        ),
        (
            "partial ensure only suppresses bounded field",
            r#"module Test where

template RoundWithPartialEnsure
  with
    admin : Party
    amuletPrice : Decimal
    tickDuration : Decimal
  where
    signatory admin
    ensure tickDuration > 0.0
"#,
        ),
        (
            "numeric fields are money fields",
            r#"module Test where

template Round
  with
    admin : Party
    price : Numeric 10
  where
    signatory admin
"#,
        ),
        (
            "negated bound does not guarantee positivity",
            r#"module Test where

template T
  with
    admin : Party
    amount : Decimal
  where
    signatory admin
    ensure not (amount > 0.0)
"#,
        ),
        (
            "disjunction bound does not guarantee positivity",
            r#"module Test where

template T
  with
    admin : Party
    amount : Decimal
    flag : Bool
  where
    signatory admin
    ensure flag || amount > 0.0
"#,
        ),
        (
            "conjunction bound counts",
            r#"module Test where

template T
  with
    admin : Party
    amount : Decimal
  where
    signatory admin
    ensure amount > 0.0 && admin == admin
"#,
        ),
        (
            "substring field names do not alias",
            r#"module Test where

template T
  with
    admin : Party
    count : Decimal
    discount : Decimal
  where
    signatory admin
    ensure discount > 0.0
"#,
        ),
    ];

    for (case_name, source) in simple_cases {
        assert_rule_matches_rust(
            case_name,
            source,
            Path::new("MissingEnsureDecimal.daml"),
            &rust_detector,
            script_detector.as_ref(),
        );
    }

    for (case_name, ensure) in [
        ("positive lower bound passes", "amount > 100.0"),
        ("negative lower bound still flags", "amount > -5.0"),
        ("positive equality passes", "amount == 5.0"),
        ("flipped positive equality passes", "5.0 == amount"),
        ("negative equality still flags", "amount == -5.0"),
        ("zero equality still flags", "amount == 0.0"),
    ] {
        let source = format!(
            "module T where\n\ntemplate M\n  with\n    owner : Party\n    amount : Decimal\n  where\n    signatory owner\n    ensure {}\n",
            ensure
        );
        assert_rule_matches_rust(
            case_name,
            &source,
            Path::new("MissingEnsureDecimal.daml"),
            &rust_detector,
            script_detector.as_ref(),
        );
    }
}

#[test]
fn unbounded_fields_script_matches_rust_regressions() {
    let script_detector = load_rule("unbounded-fields.js");
    let rust_detector = UnboundedFields;

    let cases = [
        (
            "unbounded text fields trigger",
            r#"module Test where

template BuyTrafficRequest
  with
    admin : Party
    trackingId : Text
    memberId : Text
    synchronizerId : Text
    reason : Text
  where
    signatory admin
"#,
        ),
        (
            "unbounded TextMap triggers",
            r#"module Test where

template Metadata
  with
    owner : Party
    context : TextMap Text
  where
    signatory owner
"#,
        ),
        (
            "bounded text passes",
            r#"module Test where

template SafeRequest
  with
    admin : Party
    reason : Text
  where
    signatory admin
    ensure T.length reason < 280
"#,
        ),
        (
            "map field is unbounded",
            r#"module Test where

template Meta
  with
    owner : Party
    ctx : Map Text Text
  where
    signatory owner
"#,
        ),
        (
            "lower length bound is not a size bound",
            r#"module Test where

template T
  with
    admin : Party
    reason : Text
  where
    signatory admin
    ensure length reason > 0
"#,
        ),
        (
            "flipped upper length bound passes",
            r#"module Test where

template T
  with
    admin : Party
    reason : Text
  where
    signatory admin
    ensure 280 > length reason
"#,
        ),
        (
            "field name in string literal is not a bound",
            r#"module Test where

template T
  with
    admin : Party
    reason : Text
  where
    signatory admin
    ensure reason /= "length reason here"
"#,
        ),
        (
            "exact size constraint passes",
            r#"module Test where

template T
  with
    admin : Party
    tags : [Text]
  where
    signatory admin
    ensure length tags == 3
"#,
        ),
        (
            "size bound through this passes",
            r#"module Test where

template T
  with
    admin : Party
    note : Text
  where
    signatory admin
    ensure length this.note < 280
"#,
        ),
        (
            "optional collection is still unbounded",
            r#"module Test where

template T
  with
    owner : Party
    note : Optional Text
  where
    signatory owner
"#,
        ),
        (
            "map field message is grammatical",
            r#"module Test where

template T
  with
    owner : Party
    ctx : Map Text Int
  where
    signatory owner
"#,
        ),
        (
            "sibling field bound is attacker controlled",
            r#"module Test where

template T
  with
    owner : Party
    tags : [Text]
    cap : Int
  where
    signatory owner
    ensure length tags < cap
"#,
        ),
        (
            "flipped sibling field bound is attacker controlled",
            r#"module Test where

template T
  with
    owner : Party
    tags : [Text]
    cap : Int
  where
    signatory owner
    ensure cap > length tags
"#,
        ),
        (
            "Map.size sibling field bound is attacker controlled",
            r#"module Test where

template T
  with
    owner : Party
    ctx : Map Text Text
    maxEntries : Int
  where
    signatory owner
    ensure Map.size ctx <= maxEntries
"#,
        ),
        (
            "module constant bound passes",
            r#"module Test where

maxTags : Int
maxTags = 100

template T
  with
    owner : Party
    tags : [Text]
  where
    signatory owner
    ensure length tags < maxTags
"#,
        ),
        (
            "relational length equality leaves both fields unbounded",
            r#"module Test where

template T
  with
    owner : Party
    a : [Text]
    b : [Text]
  where
    signatory owner
    ensure length a == length b
"#,
        ),
        (
            "relational length less-than leaves both fields unbounded",
            r#"module Test where

template T
  with
    owner : Party
    a : [Text]
    b : [Text]
  where
    signatory owner
    ensure length a < length b
"#,
        ),
        (
            "relational length greater-than leaves both fields unbounded",
            r#"module Test where

template T
  with
    owner : Party
    a : [Text]
    b : [Text]
  where
    signatory owner
    ensure length a > length b
"#,
        ),
        (
            "prefix sibling field name does not alias",
            r#"module Test where

template T
  with
    admin : Party
    reason : Text
    reasons : Text
  where
    signatory admin
    ensure T.length reasons < 280
"#,
        ),
    ];

    for (case_name, source) in cases {
        assert_rule_matches_rust(
            case_name,
            source,
            Path::new("UnboundedFields.daml"),
            &rust_detector,
            script_detector.as_ref(),
        );
    }
}
