use super::archive_before_execute::ArchiveBeforeExecute;
use super::ensure_decimal::MissingEnsureDecimal;
use super::positive_amount::MissingPositiveAmount;
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
fn archive_before_execute_script_matches_rust_regressions() {
    let script_detector = load_rule("archive-before-execute.js");
    let rust_detector = ArchiveBeforeExecute;

    let cases = [
        (
            "fetchAndArchive before try triggers",
            r#"module Test where

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
"#,
        ),
        (
            "archive after try passes",
            r#"module Test where

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
"#,
        ),
        (
            "choice finding reports real line",
            r#"module Test where

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
"#,
        ),
        (
            "comment mention does not trigger",
            r#"module Test where

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
"#,
        ),
        (
            "multiline archive before try is flagged",
            r#"module Test where

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
"#,
        ),
        (
            "exercise Archive before try is flagged",
            r#"module Test where

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
"#,
        ),
        (
            "identifier starting with try is not try/catch",
            r#"module Test where

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
"#,
        ),
        (
            "archive inside earlier try not flagged by later try",
            r#"module Test where

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
"#,
        ),
        (
            "archive mention in string does not trigger",
            r#"module Test where

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
"#,
        ),
        (
            "multiple multiline archives each reported",
            r#"module Test where

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
"#,
        ),
        (
            "archive then try else not flagged",
            r#"module Test where

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
"#,
        ),
        (
            "try then archive else not flagged",
            r#"module Test where

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
"#,
        ),
        (
            "archive before try within one arm is flagged",
            r#"module Test where

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
"#,
        ),
        (
            "uncalled archive helper not flagged",
            r#"module Test where

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
"#,
        ),
        (
            "archive helper called inside try not flagged",
            r#"module Test where

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
"#,
        ),
        (
            "archive helper called before try flags at call site",
            r#"module Test where

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
"#,
        ),
        (
            "multiple archives each reported",
            r#"module Test where

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
"#,
        ),
    ];

    for (case_name, source) in cases {
        assert_rule_matches_rust(
            case_name,
            source,
            Path::new("ArchiveBeforeExecute.daml"),
            &rust_detector,
            script_detector.as_ref(),
        );
    }
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

#[test]
fn missing_positive_amount_script_matches_rust_regressions() {
    let script_detector = load_rule("missing-positive-amount.js");
    let rust_detector = MissingPositiveAmount;

    let mut cases: Vec<(String, String)> = Vec::new();
    cases.push((
        "missing positive amount triggers".to_string(),
        r#"module Test where

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
"#
        .to_string(),
    ));
    cases.push((
        "asserted positive amount passes".to_string(),
        r#"module Test where

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
"#
        .to_string(),
    ));

    for (case_name, guard_line, ty) in [
        (
            "ge zero permits zero and flags",
            "assertMsg \"nn\" (amount >= 0.0)",
            "Decimal",
        ),
        (
            "flipped positive guard passes",
            "assertMsg \"pos\" (0.0 < amount)",
            "Decimal",
        ),
        (
            "substring guard does not suppress",
            "assertMsg \"x\" (xamount > 0.0)",
            "Decimal",
        ),
        (
            "comment mention does not suppress",
            "-- amount > 0 is checked elsewhere",
            "Decimal",
        ),
        ("numeric amount is checked", "pure ()", "Numeric 10"),
        (
            "when nonpositive abort is a guard",
            "when (amount <= 0.0) (abort \"must be positive\")",
            "Decimal",
        ),
        (
            "unless positive abort is a guard",
            "unless (amount > 0.0) (abort \"bad\")",
            "Decimal",
        ),
        (
            "strict negative abort is not enough",
            "when (amount < 0.0) (abort \"bad\")",
            "Decimal",
        ),
        (
            "if nonpositive abort is a guard",
            "if amount <= 0.0 then abort \"bad\" else pure ()",
            "Decimal",
        ),
        (
            "extra whitespace positive guard passes",
            "assertMsg \"pos\" (amount  >  0.0)",
            "Decimal",
        ),
        (
            "positive floor guard passes",
            "assertMsg \"floor\" (amount >= 0.01)",
            "Decimal",
        ),
        (
            "non-asserting mention does not suppress",
            "let isPos = amount > 0.0",
            "Decimal",
        ),
        (
            "qualified DA.Assert assertMsg guards",
            "DA.Assert.assertMsg \"p\" (amount > 0.0)",
            "Decimal",
        ),
        (
            "qualified Assert assertMsg guards",
            "Assert.assertMsg \"p\" (amount > 0.0)",
            "Decimal",
        ),
        (
            "conditional when assert does not suppress",
            "when isLarge (assertMsg \"p\" (amount > 0.0))",
            "Decimal",
        ),
    ] {
        cases.push((
            case_name.to_string(),
            format!(
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
            ),
        ));
    }

    cases.push((
        "conditional if then assert does not suppress".to_string(),
        r#"module Test where

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
"#
        .to_string(),
    ));

    for (case_name, guard_line, extra_param) in [
        (
            "list upper bound less-than does not suppress",
            "assertMsg \"max\" (length inputHoldingCids < 10)",
            "",
        ),
        (
            "list upper bound less-equal does not suppress",
            "assertMsg \"max\" (length inputHoldingCids <= 10)",
            "",
        ),
        (
            "list strict lower bound passes",
            "assertMsg \"ne\" (length inputHoldingCids > 0)",
            "",
        ),
        (
            "superstring list check does not suppress",
            "assertMsg \"ne\" (not (null inputHoldingCidsBackup))",
            "        inputHoldingCidsBackup : [ContractId Token]\n",
        ),
        (
            "not null list guard passes",
            "assertMsg \"ne\" (not (null inputHoldingCids))",
            "",
        ),
        (
            "not dollar null list guard passes",
            "assertMsg \"ne\" (not $ null inputHoldingCids)",
            "",
        ),
    ] {
        cases.push((
            case_name.to_string(),
            format!(
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
            ),
        ));
    }

    for (case_name, guard_line) in [
        (
            "transfer length max-only is flagged",
            "assertMsg \"max\" (length transfer.inputHoldingCids < maxNumInputs)",
        ),
        (
            "transfer size max-only is flagged",
            "assertMsg \"max\" (maxNumInputs > size transfer.inputHoldingCids)",
        ),
        (
            "transfer min and max is clean",
            "assertMsg \"ne\" (length transfer.inputHoldingCids > 0 && length transfer.inputHoldingCids < maxNumInputs)",
        ),
    ] {
        cases.push((
            case_name.to_string(),
            format!(
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
            ),
        ));
    }

    for (case_name, source) in cases {
        assert_rule_matches_rust(
            &case_name,
            &source,
            Path::new("MissingPositiveAmount.daml"),
            &rust_detector,
            script_detector.as_ref(),
        );
    }
}
