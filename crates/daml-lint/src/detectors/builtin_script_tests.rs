use super::archive_before_execute::ArchiveBeforeExecute;
use super::ensure_decimal::MissingEnsureDecimal;
use super::head_of_list::HeadOfListQuery;
use super::positive_amount::MissingPositiveAmount;
use super::script;
use super::unbounded_fields::UnboundedFields;
use super::unguarded_division::UnguardedDivision;
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
fn head_of_list_query_script_matches_rust_regressions() {
    let script_detector = load_rule("head-of-list-query.js");
    let rust_detector = HeadOfListQuery;

    let cases = [
        (
            "cons pattern on query result flags",
            r#"module Test where

getFeaturedAppRight owner = do
  results <- queryFilter @FeaturedAppRight (\r -> r.provider == owner)
  case results of
    (rightCid, _) :: _ -> do
      pure (Some rightCid)
    [] -> pure None
"#,
        ),
        (
            "singleton pattern on query result flags",
            r#"module Test where

getTransferFactory owner = do
  results <- query @TransferFactory owner
  case results of
    [(rulesCid, _)] -> pure rulesCid
"#,
        ),
        (
            "safe query usage passes",
            r#"module Test where

getAllFactories owner = do
  results <- query @TransferFactory owner
  mapA (\(cid, _) -> fetch cid) results
"#,
        ),
        (
            "cons pattern reported once",
            r#"module Test where

getOne owner = do
  results <- query @Foo owner
  case results of
    x :: _ -> pure (Some x)
    [] -> pure None
"#,
        ),
        (
            "head on non-query list ignored",
            r#"module Test where

firstOf xs = pure (head xs)
"#,
        ),
        (
            "head on query binding flags",
            r#"module Test where

pick owner = do
  results <- query @Foo owner
  pure (head results)
"#,
        ),
        (
            "index on query binding flags",
            r#"module Test where

pick owner = do
  results <- query @Foo owner
  pure (results !! 0)
"#,
        ),
        (
            "head dollar on query binding flags",
            r#"module Test where

pick owner = do
  results <- query @Foo owner
  pure (head $ results)
"#,
        ),
        (
            "head dollar on sorted query is safe",
            r#"module Test where

pick owner = do
  results <- query @Foo owner
  pure (head $ sortOn f results)
"#,
        ),
        (
            "qualified head on query binding flags",
            r#"module Test where

pick owner = do
  results <- query @Foo owner
  pure (DA.List.head results)
"#,
        ),
        (
            "head of sorted query result is not flagged",
            r#"module Test where

pick owner = do
  results <- query @Foo owner
  let sorted = sortOn snd results
  case sorted of
    x :: _ -> pure (Some x)
    [] -> pure None
"#,
        ),
        (
            "recursive cons binding tail is not flagged",
            r#"module Test where

go owner = do
  results <- query @Foo owner
  case results of
    x :: rest -> process x rest
    [] -> pure ()
"#,
        ),
        (
            "singleton bind from query flags",
            r#"module Test where

pick owner = do
  [theOne] <- query @Foo owner
  pure theOne
"#,
        ),
        (
            "cons head bind from query flags",
            r#"module Test where

pick owner = do
  (x :: _) <- query @Foo owner
  pure x
"#,
        ),
        (
            "fixed-many bind from query is safe",
            "module Test where\n\nf owner = do\n  [a, b] <- query @Foo owner\n  pure (a, b)\n",
        ),
        (
            "tail-binding cons bind from query is safe",
            "module Test where\n\nf owner = do\n  (x :: rest) <- query @Foo owner\n  process x rest\n",
        ),
        (
            "plain bind from query is safe until head use",
            "module Test where\n\nf owner = do\n  results <- query @Foo owner\n  mapA fetch results\n",
        ),
        (
            "nested case on local list is not flagged",
            r#"module Test where

pick owner = do
  results <- query @Foo owner
  case results of
    _ -> do
      let names = ["a", "b"]
      case names of
        first :: _ -> pure (Some first)
        [] -> pure None
"#,
        ),
        (
            "nested case on query result flags once",
            r#"module Test where

pick owner = do
  results <- query @Foo owner
  case owner of
    _ -> do
      case results of
        first :: _ -> pure (Some first)
        [] -> pure None
"#,
        ),
        (
            "monadic rebind to sorted list clears tracking",
            r#"module Test where

pick owner = do
  results <- query @Foo owner
  results <- pure (sortOn snd results)
  pure (head results)
"#,
        ),
        (
            "let rebind to sorted list clears tracking",
            r#"module Test where

pick owner = do
  raw <- query @Foo owner
  let raw = sortOn snd raw
  pure (head raw)
"#,
        ),
        (
            "head before sorted rebind still flags",
            r#"module Test where

pick owner = do
  results <- query @Foo owner
  first <- pure (head results)
  results <- pure (sortOn snd results)
  pure first
"#,
        ),
        (
            "requery rebind still flags",
            r#"module Test where

pick owner = do
  results <- query @Foo owner
  results <- query @Bar owner
  pure (head results)
"#,
        ),
        (
            "direct alias of query result flags",
            r#"module Test where

pick owner = do
  results <- query @Foo owner
  let alias = results
  pure (head alias)
"#,
        ),
        (
            "alias chain of query result flags",
            r#"module Test where

pick owner = do
  results <- query @Foo owner
  let a = results
  let b = a
  pure (head b)
"#,
        ),
        (
            "derived binding is not an alias",
            r#"module Test where

pick owner = do
  results <- query @Foo owner
  let sorted = sortOn snd results
  pure (head sorted)
"#,
        ),
        (
            "head fmap over query flags",
            "module Test where\n\npick owner = do\n  x <- head <$> query @Foo owner\n  pure x\n",
        ),
        (
            "fmap head over query flags",
            "module Test where\n\npick owner = do\n  x <- fmap head (query @Foo owner)\n  pure x\n",
        ),
        (
            "last fmap over query flags",
            "module Test where\n\npick owner = do\n  x <- last <$> query @Foo owner\n  pure x\n",
        ),
        (
            "non-head fmap over query is not flagged",
            r#"module Test where

pick owner = do
  xs <- sortOn snd <$> query @Foo owner
  pure xs
"#,
        ),
    ];

    for (case_name, source) in cases {
        assert_rule_matches_rust(
            case_name,
            source,
            Path::new("HeadOfListQuery.daml"),
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

#[test]
fn unguarded_division_script_matches_rust_regressions() {
    let script_detector = load_rule("unguarded-division.js");
    let rust_detector = UnguardedDivision;

    let cases = [
        (
            "unguarded division triggers",
            r#"module Test where

scaleFees fees rate =
  map (\f -> f with amount = f.amount * (1.0 / rate)) fees
"#,
        ),
        (
            "guarded division passes",
            r#"module Test where

safeDivide x y = do
  assertMsg "denominator must be positive" (y > 0)
  pure (x / y)
"#,
        ),
        (
            "intToDecimal wrapper reports real denominator",
            r#"module Test where

dayCount total n = total / intToDecimal n
"#,
        ),
        (
            "guarded intToDecimal division passes",
            r#"module Test where

dayCount total n = do
  assertMsg "n must be positive" (n > 0)
  pure (total / intToDecimal n)
"#,
        ),
        (
            "guard after division is flagged",
            r#"module Test where

unsafeDivide x y = do
  pure (x / y)
  assertMsg "denominator must be positive" (y > 0)
"#,
        ),
        (
            "substring guard does not suppress",
            r#"module Test where

compute x q = do
  assertMsg "quantity" (quantity > 0)
  pure (x / q)
"#,
        ),
        (
            "ge zero is not a guard",
            r#"module Test where

divCheck x y = do
  assertMsg "non-negative" (y >= 0)
  pure (x / y)
"#,
        ),
        (
            "second division on line is flagged",
            r#"module Test where

compute a b c d = do
  assertMsg "b ok" (b > 0)
  pure (a / b + c / d)
"#,
        ),
        ("literal denominator safe", "module T where\nf x = x / 2.0\n"),
        ("zero literal denominator flags", "module T where\nf x = x / 0\n"),
        (
            "slash in string literal is not division",
            r#"module Test where

logUrl = debug "http://host/api/v1/data"
"#,
        ),
        (
            "slash in comment is not division",
            r#"module Test where

f x = do
  {- ratio a/b/c is documented elsewhere -}
  pure x -- see n/m below
"#,
        ),
        ("line wrapped division is flagged", "module Test where\n\nratio a b = a /\n  b\n"),
        (
            "parenthesized literal denominator is safe",
            "module T where\nf x = x / (2.0)\n",
        ),
        (
            "if nonzero guard suppresses",
            "module T where\nf x denom = pure (if denom /= 0.0 then x / denom else 0.0)\n",
        ),
        (
            "if zero else guard suppresses",
            "module T where\nf x denom = pure (if denom == 0.0 then 0.0 else x / denom)\n",
        ),
        (
            "if unrelated condition does not guard",
            "module T where\nf x denom flag = pure (if flag then x / denom else 0.0)\n",
        ),
        (
            "prefix div is flagged",
            r#"module Test where

share total n = pure (div total n)
"#,
        ),
        (
            "guarded prefix div passes",
            r#"module Test where

share total n = do
  assertMsg "n positive" (n > 0)
  pure (div total n)
"#,
        ),
        ("prefix div literal denominator safe", "module T where\nf x = pure (div x 2)\n"),
        (
            "ensure clause guards choice division",
            r#"module Test where

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
"#,
        ),
        (
            "disjunction guard does not suppress",
            r#"module Test where

f x y = do
  assertMsg "weak" (y > 0 || x > 5)
  pure (x / y)
"#,
        ),
        (
            "same line guard suppresses",
            "module Test where\nf x y = do { assertMsg \"y\" (y > 0.0); pure (x / y) }\n",
        ),
        (
            "same line guard after division is flagged",
            "module Test where\nf x y = do { pure (x / y); assertMsg \"y\" (y > 0.0) }\n",
        ),
        ("negative literal denominator safe", "module T where\nf x = x / (-2.0)\n"),
        (
            "negative prefix literal denominator safe",
            "module T where\nf x = div x (-3)\n",
        ),
        ("negative zero denominator flags", "module T where\nf x = x / (-0.0)\n"),
        (
            "conditional if guard does not suppress",
            "module Test where\nf flag x y = do\n  if flag\n    then assertMsg \"y ok\" (y > 0.0)\n    else pure ()\n  pure (x / y)\n",
        ),
        (
            "conditional case guard does not suppress",
            "module Test where\nf k x y = do\n  case k of\n    _ -> assertMsg \"y\" (y > 0.0)\n  pure (x / y)\n",
        ),
        (
            "branch arm guard suppresses division in same arm",
            r#"module Test where

f flag x y =
  if flag then do
    assertMsg "y" (y /= 0.0)
    pure (x / y)
  else
    pure 0.0
"#,
        ),
        (
            "forA guard does not suppress",
            "module Test where\nf x y items = do\n  forA_ items (\\i -> assertMsg \"y\" (y > 0.0))\n  pure (x / y)\n",
        ),
        (
            "when gated guard does not suppress",
            "module Test where\nf x y b = do\n  when b (assertMsg \"y\" (y > 0.0))\n  pure (x / y)\n",
        ),
        (
            "unconditional guard before branch division suppresses",
            "module Test where\nf x y b = do\n  assertMsg \"y\" (y > 0.0)\n  if b then pure (x / y) else pure 0.0\n",
        ),
        (
            "parenthesized denominator reported whole",
            "module T where\nf x y = x / (y + 1)\n",
        ),
    ];

    for (case_name, source) in cases {
        assert_rule_matches_rust(
            case_name,
            source,
            Path::new("UnguardedDivision.daml"),
            &rust_detector,
            script_detector.as_ref(),
        );
    }
}
