use super::script;
use crate::detector::{Detector, Severity};
use crate::parser::parse_daml;
use std::path::Path;

fn load_rule(name: &str) -> Box<dyn Detector> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("rules")
        .join(name);
    let source = std::fs::read_to_string(&path).unwrap();
    script::load_script_source(&path.display().to_string(), &source).unwrap()
}

fn assert_rule_findings(
    case_name: &str,
    source: &str,
    file: &Path,
    script_detector: &dyn Detector,
) {
    let module = parse_daml(source, file);
    let findings = script_detector.detect(&module);
    let expected_count = expected_count(script_detector.name(), case_name);
    assert_eq!(
        findings.len(),
        expected_count,
        "unexpected finding count for {} / {case_name}: {findings:?}",
        script_detector.name()
    );
    for finding in &findings {
        assert_eq!(finding.detector, script_detector.name());
        assert_eq!(finding.severity, expected_severity(script_detector.name()));
        assert!(
            !finding.message.is_empty(),
            "finding message should explain {} / {case_name}",
            script_detector.name()
        );
        assert!(
            !finding.evidence.is_empty(),
            "finding evidence should identify source for {} / {case_name}",
            script_detector.name()
        );
    }
}

fn expected_severity(rule_name: &str) -> Severity {
    match rule_name {
        "missing-ensure-decimal"
        | "unguarded-division"
        | "missing-positive-amount"
        | "archive-before-execute" => Severity::High,
        "head-of-list-query" | "unbounded-fields" => Severity::Medium,
        other => panic!("missing expected severity for {other}"),
    }
}

fn expected_count(rule_name: &str, case_name: &str) -> usize {
    match (rule_name, case_name) {
        ("archive-before-execute", "archive after try passes") => 0,
        ("archive-before-execute", "archive before try within one arm is flagged") => 1,
        ("archive-before-execute", "archive helper called before try flags at call site") => 1,
        ("archive-before-execute", "archive helper called inside try not flagged") => 0,
        ("archive-before-execute", "archive inside earlier try not flagged by later try") => 0,
        ("archive-before-execute", "archive mention in string does not trigger") => 0,
        ("archive-before-execute", "archive then try else not flagged") => 0,
        ("archive-before-execute", "choice finding reports real line") => 1,
        ("archive-before-execute", "comment mention does not trigger") => 0,
        ("archive-before-execute", "exercise Archive before try is flagged") => 1,
        ("archive-before-execute", "fetchAndArchive before try triggers") => 1,
        ("archive-before-execute", "identifier starting with try is not try/catch") => 0,
        ("archive-before-execute", "multiline archive before try is flagged") => 1,
        ("archive-before-execute", "multiple archives each reported") => 2,
        ("archive-before-execute", "multiple multiline archives each reported") => 2,
        ("archive-before-execute", "try then archive else not flagged") => 0,
        ("archive-before-execute", "uncalled archive helper not flagged") => 0,

        ("head-of-list-query", "alias chain of query result flags") => 1,
        ("head-of-list-query", "cons head bind from query flags") => 1,
        ("head-of-list-query", "cons pattern on query result flags") => 1,
        ("head-of-list-query", "cons pattern reported once") => 1,
        ("head-of-list-query", "derived binding is not an alias") => 0,
        ("head-of-list-query", "direct alias of query result flags") => 1,
        ("head-of-list-query", "fixed-many bind from query is safe") => 0,
        ("head-of-list-query", "fmap head over query flags") => 1,
        ("head-of-list-query", "head before sorted rebind still flags") => 1,
        ("head-of-list-query", "head dollar on query binding flags") => 1,
        ("head-of-list-query", "head dollar on sorted query is safe") => 0,
        ("head-of-list-query", "head fmap over query flags") => 1,
        ("head-of-list-query", "head of sorted query result is not flagged") => 0,
        ("head-of-list-query", "head on non-query list ignored") => 0,
        ("head-of-list-query", "head on query binding flags") => 1,
        ("head-of-list-query", "index on query binding flags") => 1,
        ("head-of-list-query", "last fmap over query flags") => 1,
        ("head-of-list-query", "let rebind to sorted list clears tracking") => 0,
        ("head-of-list-query", "monadic rebind to sorted list clears tracking") => 0,
        ("head-of-list-query", "nested case on local list is not flagged") => 0,
        ("head-of-list-query", "nested case on query result flags once") => 1,
        ("head-of-list-query", "non-head fmap over query is not flagged") => 0,
        ("head-of-list-query", "plain bind from query is safe until head use") => 0,
        ("head-of-list-query", "qualified head on query binding flags") => 1,
        ("head-of-list-query", "recursive cons binding tail is not flagged") => 0,
        ("head-of-list-query", "requery rebind still flags") => 1,
        ("head-of-list-query", "safe query usage passes") => 0,
        ("head-of-list-query", "singleton bind from query flags") => 1,
        ("head-of-list-query", "singleton pattern on query result flags") => 1,
        ("head-of-list-query", "tail-binding cons bind from query is safe") => 0,

        ("missing-ensure-decimal", "conjunction bound counts") => 0,
        ("missing-ensure-decimal", "disjunction bound does not guarantee positivity") => 1,
        ("missing-ensure-decimal", "flipped positive equality passes") => 0,
        ("missing-ensure-decimal", "missing ensure reports each decimal field") => 2,
        ("missing-ensure-decimal", "negated bound does not guarantee positivity") => 1,
        ("missing-ensure-decimal", "negative equality still flags") => 1,
        ("missing-ensure-decimal", "negative lower bound still flags") => 1,
        ("missing-ensure-decimal", "numeric fields are money fields") => 1,
        ("missing-ensure-decimal", "partial ensure only suppresses bounded field") => 1,
        ("missing-ensure-decimal", "positive ensure bound suppresses finding") => 0,
        ("missing-ensure-decimal", "positive equality passes") => 0,
        ("missing-ensure-decimal", "positive lower bound passes") => 0,
        ("missing-ensure-decimal", "substring field names do not alias") => 1,
        ("missing-ensure-decimal", "zero equality still flags") => 1,

        ("missing-positive-amount", "asserted positive amount passes") => 0,
        ("missing-positive-amount", "comment mention does not suppress") => 1,
        ("missing-positive-amount", "conditional if then assert does not suppress") => 1,
        ("missing-positive-amount", "conditional when assert does not suppress") => 1,
        ("missing-positive-amount", "extra whitespace positive guard passes") => 0,
        ("missing-positive-amount", "flipped positive guard passes") => 0,
        ("missing-positive-amount", "ge zero permits zero and flags") => 1,
        ("missing-positive-amount", "if nonpositive abort is a guard") => 0,
        ("missing-positive-amount", "list strict lower bound passes") => 0,
        ("missing-positive-amount", "list upper bound less-equal does not suppress") => 1,
        ("missing-positive-amount", "list upper bound less-than does not suppress") => 1,
        ("missing-positive-amount", "missing positive amount triggers") => 1,
        ("missing-positive-amount", "non-asserting mention does not suppress") => 1,
        ("missing-positive-amount", "not dollar null list guard passes") => 0,
        ("missing-positive-amount", "not null list guard passes") => 0,
        ("missing-positive-amount", "numeric amount is checked") => 1,
        ("missing-positive-amount", "positive floor guard passes") => 0,
        ("missing-positive-amount", "qualified Assert assertMsg guards") => 0,
        ("missing-positive-amount", "qualified DA.Assert assertMsg guards") => 0,
        ("missing-positive-amount", "strict negative abort is not enough") => 1,
        ("missing-positive-amount", "substring guard does not suppress") => 1,
        ("missing-positive-amount", "superstring list check does not suppress") => 1,
        ("missing-positive-amount", "transfer length max-only is flagged") => 1,
        ("missing-positive-amount", "transfer min and max is clean") => 0,
        ("missing-positive-amount", "transfer size max-only is flagged") => 1,
        ("missing-positive-amount", "unless positive abort is a guard") => 0,
        ("missing-positive-amount", "when nonpositive abort is a guard") => 0,

        ("unbounded-fields", "Map.size sibling field bound is attacker controlled") => 1,
        ("unbounded-fields", "bounded text passes") => 0,
        ("unbounded-fields", "exact size constraint passes") => 0,
        ("unbounded-fields", "field name in string literal is not a bound") => 1,
        ("unbounded-fields", "flipped sibling field bound is attacker controlled") => 1,
        ("unbounded-fields", "flipped upper length bound passes") => 0,
        ("unbounded-fields", "lower length bound is not a size bound") => 1,
        ("unbounded-fields", "map field is unbounded") => 1,
        ("unbounded-fields", "map field message is grammatical") => 1,
        ("unbounded-fields", "module constant bound passes") => 0,
        ("unbounded-fields", "optional collection is still unbounded") => 1,
        ("unbounded-fields", "prefix sibling field name does not alias") => 1,
        ("unbounded-fields", "relational length equality leaves both fields unbounded") => 1,
        ("unbounded-fields", "relational length greater-than leaves both fields unbounded") => 1,
        ("unbounded-fields", "relational length less-than leaves both fields unbounded") => 1,
        ("unbounded-fields", "sibling field bound is attacker controlled") => 1,
        ("unbounded-fields", "size bound through this passes") => 0,
        ("unbounded-fields", "unbounded TextMap triggers") => 1,
        ("unbounded-fields", "unbounded text fields trigger") => 1,

        ("unguarded-division", "branch arm guard suppresses division in same arm") => 0,
        ("unguarded-division", "conditional case guard does not suppress") => 1,
        ("unguarded-division", "conditional if guard does not suppress") => 1,
        ("unguarded-division", "disjunction guard does not suppress") => 1,
        ("unguarded-division", "ensure clause guards choice division") => 0,
        ("unguarded-division", "forA guard does not suppress") => 1,
        ("unguarded-division", "ge zero is not a guard") => 1,
        ("unguarded-division", "guard after division is flagged") => 1,
        ("unguarded-division", "guarded division passes") => 0,
        ("unguarded-division", "guarded intToDecimal division passes") => 0,
        ("unguarded-division", "guarded prefix div passes") => 0,
        ("unguarded-division", "if nonzero guard suppresses") => 0,
        ("unguarded-division", "if unrelated condition does not guard") => 1,
        ("unguarded-division", "if zero else guard suppresses") => 0,
        ("unguarded-division", "intToDecimal wrapper reports real denominator") => 1,
        ("unguarded-division", "line wrapped division is flagged") => 1,
        ("unguarded-division", "literal denominator safe") => 0,
        ("unguarded-division", "negative literal denominator safe") => 0,
        ("unguarded-division", "negative prefix literal denominator safe") => 0,
        ("unguarded-division", "negative zero denominator flags") => 1,
        ("unguarded-division", "parenthesized denominator reported whole") => 1,
        ("unguarded-division", "parenthesized literal denominator is safe") => 0,
        ("unguarded-division", "prefix div is flagged") => 1,
        ("unguarded-division", "prefix div literal denominator safe") => 0,
        ("unguarded-division", "same line guard after division is flagged") => 1,
        ("unguarded-division", "same line guard suppresses") => 0,
        ("unguarded-division", "second division on line is flagged") => 1,
        ("unguarded-division", "slash in comment is not division") => 0,
        ("unguarded-division", "slash in string literal is not division") => 0,
        ("unguarded-division", "substring guard does not suppress") => 1,
        ("unguarded-division", "unconditional guard before branch division suppresses") => 0,
        ("unguarded-division", "unguarded division triggers") => 1,
        ("unguarded-division", "when gated guard does not suppress") => 1,
        ("unguarded-division", "zero literal denominator flags") => 1,
        other => panic!("missing expected finding count for {other:?}"),
    }
}

#[test]
fn archive_before_execute_script_covers_regressions() {
    let script_detector = load_rule("archive-before-execute.js");
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
        assert_rule_findings(
            case_name,
            source,
            Path::new("ArchiveBeforeExecute.daml"),
            script_detector.as_ref(),
        );
    }
}

#[test]
fn head_of_list_query_script_covers_regressions() {
    let script_detector = load_rule("head-of-list-query.js");
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
        assert_rule_findings(
            case_name,
            source,
            Path::new("HeadOfListQuery.daml"),
            script_detector.as_ref(),
        );
    }
}

#[test]
fn missing_ensure_decimal_script_covers_regressions() {
    let script_detector = load_rule("missing-ensure-decimal.js");
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
        assert_rule_findings(
            case_name,
            source,
            Path::new("MissingEnsureDecimal.daml"),
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
        assert_rule_findings(
            case_name,
            &source,
            Path::new("MissingEnsureDecimal.daml"),
            script_detector.as_ref(),
        );
    }
}

#[test]
fn unbounded_fields_script_covers_regressions() {
    let script_detector = load_rule("unbounded-fields.js");
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
        assert_rule_findings(
            case_name,
            source,
            Path::new("UnboundedFields.daml"),
            script_detector.as_ref(),
        );
    }
}

#[test]
fn missing_positive_amount_script_covers_regressions() {
    let script_detector = load_rule("missing-positive-amount.js");
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
        assert_rule_findings(
            &case_name,
            &source,
            Path::new("MissingPositiveAmount.daml"),
            script_detector.as_ref(),
        );
    }
}

#[test]
fn unguarded_division_script_covers_regressions() {
    let script_detector = load_rule("unguarded-division.js");
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
        assert_rule_findings(
            case_name,
            source,
            Path::new("UnguardedDivision.daml"),
            script_detector.as_ref(),
        );
    }
}
