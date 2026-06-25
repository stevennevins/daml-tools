//! Integration tests for custom rule JS visitor/runtime surface via public APIs.

#![cfg(feature = "custom-rules")]
#![allow(clippy::unwrap_used)]

use daml_lint::detector::Detector;
use daml_lint::detectors::script::load_script;
use daml_lint::ir::DamlModule;
use daml_lint::parser::parse_daml_with_diagnostics;
use daml_syntax::{CharColumn, LineNumber};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

fn manifest_path(rel: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(rel)
}

fn temp_script_file(name: &str, contents: &str) -> PathBuf {
    let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "daml-lint-rule-{}-{}-{}",
        std::process::id(),
        id,
        name
    ));
    std::fs::write(&path, contents).unwrap();
    path
}

fn load_rule_script(label: &str, source: &str) -> Box<dyn Detector> {
    let path = temp_script_file(label, source);
    load_script(&path).unwrap_or_else(|e| panic!("failed to load {label}: {e}"))
}

fn parse_module(source: &str, file: &str) -> DamlModule {
    parse_daml_with_diagnostics(source, Path::new(file)).module
}

const TEMPLATE_NO_ENSURE: &str = r#"module Test where

template Iou
  with
    issuer : Party
    owner : Party
    amount : Decimal
  where
    signatory issuer
    observer owner

    choice Transfer : ()
      controller owner
      do
        pure ()
"#;

#[test]
fn on_template_visitor_reports() {
    let det = load_rule_script(
        "on-template.js",
        r#"
const NAME = "template-requires-ensure";
const SEVERITY = "medium";

function on_template(template) {
    if (template.ensure_clause === null) {
        report(template, `Template '${template.name}' has no ensure clause`);
    }
}
"#,
    );
    let module = parse_module(TEMPLATE_NO_ENSURE, "Test.daml");
    let findings = det.detect(&module);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].detector, "template-requires-ensure");
    assert_eq!(findings[0].file, Path::new("Test.daml"));
    assert_eq!(findings[0].line, LineNumber::new(3));
    assert_eq!(findings[0].column, CharColumn::new(1));
    assert!(findings[0].message.contains("Iou"));
    assert_eq!(findings[0].evidence, "template Iou");
}

#[test]
fn report_accepts_explicit_evidence() {
    let det = load_rule_script(
        "explicit-evidence.js",
        r#"
const NAME = "explicit-evidence";
const SEVERITY = "low";

function on_template(template) {
    report(template, "template evidence test", "custom evidence");
}
"#,
    );
    let module = parse_module(TEMPLATE_NO_ENSURE, "Test.daml");
    let findings = det.detect(&module);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].evidence, "custom evidence");
}

#[test]
fn dts_exposes_structured_only_contract() {
    let dts = std::fs::read_to_string(manifest_path("examples/daml-lint.d.ts"))
        .expect("read daml-lint.d.ts");
    assert!(dts.contains("ir_version: 4"));
    assert!(
        dts.contains("| { Lit: { kind:"),
        "daml-lint.d.ts must expose TypeNode.Lit for type-level literals"
    );
    for forbidden in [
        "body_raw",
        "raw_text",
        "cid_expr",
        "controllers: string",
        "signatories: string",
        "observers: string",
        "type_text",
        "key_type: string",
        "type_signature: string",
        "expr: string",
        "condition: string",
    ] {
        assert!(
            !dts.contains(forbidden),
            "daml-lint.d.ts must not expose removed field {forbidden:?}"
        );
    }
}

#[test]
fn runtime_module_exposes_structured_only_contract() {
    let det = load_rule_script(
        "structured-only-runtime.js",
        r#"
const NAME = "structured-only-runtime";
const SEVERITY = "low";

function has(o, k) {
  return Object.prototype.hasOwnProperty.call(o, k);
}

function checkNoOldFields(label, node, fields) {
  for (const field of fields) {
    if (has(node, field)) report(1, `${label} still exposes ${field}`);
  }
}

function spanOfType(ty) {
  const tag = Object.keys(ty)[0];
  return ty[tag].span;
}

function typeSource(m, ty) {
  const span = spanOfType(ty);
  return m.source.slice(span.start, span.end);
}

function field(template, name) {
  return template.fields.find((f) => f.name === name);
}

function check(m) {
  if (m.ir_version !== 4) report(1, `expected ir_version 4, got ${m.ir_version}`);
  const t = m.templates[0];
  checkNoOldFields("template", t, ["signatories", "observers"]);
  if (typeof t.key_type === "string") report(1, "template key_type is still a string");
  checkNoOldFields("ensure", t.ensure_clause, ["raw_text"]);
  for (const f of t.fields) {
    checkNoOldFields("field", f, ["type_text"]);
    if (typeof f.type_ === "string") report(1, `field ${f.name} type_ is still a string`);
  }
  if (typeSource(m, field(t, "maybeCid").type_) !== "Optional (ContractId T)") {
    report(1, "type span cannot recover Optional (ContractId T)");
  }
  if (typeSource(m, field(t, "precision").type_) !== "Numeric 10") {
    report(1, "type span cannot recover Numeric 10");
  }
  const c = t.choices[0];
  checkNoOldFields("choice", c, ["controllers", "body_raw", "return_type_text"]);
  if (typeof c.return_type === "string") report(1, "choice return_type is still a string");
  for (const p of c.parameters) checkNoOldFields("choice parameter", p, ["type_text"]);
  for (const stmt of c.body) {
    if ("Let" in stmt) checkNoOldFields("Let", stmt.Let, ["expr"]);
    if ("Assert" in stmt) checkNoOldFields("Assert", stmt.Assert, ["condition"]);
    if ("Fetch" in stmt) checkNoOldFields("Fetch", stmt.Fetch, ["cid_expr"]);
    if ("Archive" in stmt) checkNoOldFields("Archive", stmt.Archive, ["cid_expr"]);
    if ("Create" in stmt) checkNoOldFields("Create", stmt.Create, ["raw"]);
    if ("Exercise" in stmt) checkNoOldFields("Exercise", stmt.Exercise, ["cid_expr", "raw"]);
  }
  const fn = m.functions[0];
  checkNoOldFields("function", fn, ["body_raw", "type_text"]);
  if (typeof fn.type_signature === "string") report(1, "function type_signature is still a string");
}
"#,
    );
    let source = r#"
module RuntimeSurface where

template T
  with
    owner : Party
    cid : ContractId T
    maybeCid : Optional (ContractId T)
    amount : Decimal
    precision : Numeric 10
  where
    signatory owner
    observer owner
    ensure amount > 0.0
    key owner : Party
    maintainer key

    choice C : ContractId T
      with
        p : Party
        target : ContractId T
      controller owner
      observer p
      do
        let x = target
        assert True
        fetched <- fetch target
        archive target
        created <- create this with owner = p
        exercise target C with p = p; target = target

helper : Party -> Update ()
helper p = pure ()
"#;
    let module = parse_module(source, "RuntimeSurface.daml");
    assert!(det.detect(&module).is_empty());
}

#[test]
fn on_choice_visitor_gets_template_context() {
    let det = load_rule_script(
        "on-choice.js",
        r#"
const NAME = "consuming-choice-signatory-controller";
const SEVERITY = "medium";

function exprText(e) {
    if ("Var" in e) {
        const v = e.Var;
        return v.qualifier === null ? v.name : `${v.qualifier}.${v.name}`;
    }
    if ("Con" in e) {
        const c = e.Con;
        return c.qualifier === null ? c.name : `${c.qualifier}.${c.name}`;
    }
    if ("App" in e) return [exprText(e.App.func), ...e.App.args.map(exprText)].join(" ");
    if ("Unknown" in e) return e.Unknown.raw;
    return "";
}

function on_choice(choice, template) {
    if (choice.consuming === "non-consuming") {
        return;
    }
    const signatories = template.signatory_exprs.map(exprText);
    if (choice.controller_exprs.some(c => signatories.includes(exprText(c)))) {
        return;
    }
    report(choice, `Consuming choice '${choice.name}' has no signatory controller`);
}
"#,
    );
    let module = parse_module(TEMPLATE_NO_ENSURE, "Test.daml");
    let findings = det.detect(&module);
    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.contains("Transfer"));
}

#[test]
fn check_visitor_and_line_report() {
    let det = load_rule_script(
        "check-module.js",
        r#"
const NAME = "max-one-template";
const SEVERITY = "low";

function check(module) {
    if (module.templates.length > 0) {
        report(1, `Module '${module.name}' has templates`);
    }
}
"#,
    );
    let module = parse_module(TEMPLATE_NO_ENSURE, "Test.daml");
    let findings = det.detect(&module);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].line, LineNumber::new(1));
}

#[test]
fn statement_bodies_inspectable() {
    let det = load_rule_script(
        "statements.js",
        r#"
const NAME = "no-create-in-choice";
const SEVERITY = "low";

function on_choice(choice) {
    for (const stmt of choice.body) {
        if ("Create" in stmt) {
            report(choice, `Choice '${choice.name}' creates contracts`);
        }
    }
}
"#,
    );
    let module = parse_module(TEMPLATE_NO_ENSURE, "Test.daml");
    det.detect(&module);
}

#[test]
fn every_node_kind_reaches_scripts() {
    let probe = r#"module Probe where

import qualified DA.Map as Map
import DA.Time

template Probe
  with
    owner : Party
    note : Text
    amount : Decimal
    count : Int
    active : Bool
    issued : Date
    stamp : Time
    tags : [Text]
    backup : Optional Party
    parent : ContractId Probe
    scores : TextMap Int
    extra : Custom
    hasOwner : HasField "owner" Party Party
  where
    signatory owner
    ensure amount > 0.0

    choice Reissue : ContractId Probe
      with
        newOwner : Party
      controller owner
      do
        let total = amount + 1.0
        assert (total > 0.0)
        p <- fetch parent
        archive parent
        cid <- create this with owner = newOwner
        result <- exercise cid Noop
        try do
          pure ()
        catch
          (e : AnyException) -> pure ()
        pure cid

    nonconsuming choice Noop : ()
      observer owner
      controller owner
      do
        pure ()

    key (owner, note) : (Party, Text)
    maintainer key._1

    interface instance Probeable for Probe where
      view = ProbeView owner

interface Probeable where
  viewtype ProbeView
  getProbeOwner : Party

  nonconsuming choice GetProbeView : ProbeView
    with
      viewer : Party
    controller viewer
    do
      pure (view this)

helper : Int -> Int
helper x = x + 1

describe owner xs total parts = do
  let scaled = map (\y -> y * 2) xs
  let pair = (total / parts, [total, parts])
  let label = if total > 0.0 then "pos" else show (-total)
  let picked = case xs of
        [] -> None
        h :: _ -> Some h
  let result = let inner = Map.Map in inner
  pure (FooCon with field1 = label)
"#;
    let det = load_rule_script(
        "census.js",
        r#"
const NAME = "node-census";
const SEVERITY = "info";

function exprKinds(e, seen) {
  if (e === null || typeof e !== "object") return;
  const k = Object.keys(e)[0];
  seen.add("Expr:" + k);
  const p = e[k];
  for (const key of Object.keys(p)) {
    const v = p[key];
    if (key === "span") continue;
    if (Array.isArray(v)) {
      for (const item of v) {
        if (item && typeof item === "object") {
          if ("body" in item && "pattern" in item) exprKinds(item.body, seen);
          else if ("value" in item && "name" in item) {
            if (item.value !== null) exprKinds(item.value, seen);
          } else if (Object.keys(item).length === 1) {
            if (k === "DoBlock") stmtKinds([item], seen);
            else exprKinds(item, seen);
          }
        }
      }
    } else if (v && typeof v === "object") {
      exprKinds(v, seen);
    }
  }
}

function stmtKinds(stmts, seen) {
  for (const s of stmts) {
    const k = Object.keys(s)[0];
    seen.add(k);
    const p = s[k];
    if (p.binder !== undefined && p.binder !== null) seen.add("Binder");
    if (k === "TryCatch") {
      stmtKinds(p.try_body, seen);
      stmtKinds(p.catch_body, seen);
    }
    if (p.value) exprKinds(p.value, seen);
    if (p.condition_expr) exprKinds(p.condition_expr, seen);
    if (p.cid) exprKinds(p.cid, seen);
    if (p.argument) exprKinds(p.argument, seen);
    if (p.expr && typeof p.expr === "object") exprKinds(p.expr, seen);
  }
}

function typeHeadName(t) {
  if (t === null) return "Unknown";
  const tag = Object.keys(t)[0];
  if (tag === "Con") return t.Con.name;
  if (tag === "App") return typeHeadName(t.App.head);
  return tag;
}

function typeKind(t) {
  if (t === null) return "Scalar:Unknown";
  const tag = Object.keys(t)[0];
  if (tag === "Con") return "Scalar:" + t.Con.name;
  if (tag === "List") return "Param:List";
  if (tag === "App") return "Param:" + typeHeadName(t.App.head);
  return "Scalar:" + tag;
}

function typeKinds(t, seen) {
  if (t === null) return;
  const tag = Object.keys(t)[0];
  seen.add("Type:" + tag);
  const p = t[tag];
  if (tag === "App") {
    typeKinds(p.head, seen);
    for (const arg of p.args) typeKinds(arg, seen);
  } else if (tag === "List") {
    typeKinds(p.inner, seen);
  } else if (tag === "Tuple") {
    for (const item of p.items) typeKinds(item, seen);
  } else if (tag === "Fun") {
    typeKinds(p.param, seen);
    typeKinds(p.result, seen);
  } else if (tag === "Constrained") {
    typeKinds(p.body, seen);
  }
}

function check(m) {
  const seen = new Set();
  for (const t of m.templates) {
    if (t.ensure_clause !== null) {
      seen.add("Ensure");
      exprKinds(t.ensure_clause.expr, seen);
    }
    if (t.key_expr !== null) seen.add("KeyExpr");
    if (t.key_type !== null) seen.add("KeyType");
    if (t.maintainer_exprs.length > 0) seen.add("Maintainer");
    if (t.signatory_exprs.length > 0) seen.add("SignatoryExpr");
    if (t.interface_instances.length > 0) {
      seen.add("InterfaceInstance");
      if (t.interface_instances[0].methods.length > 0) seen.add("InstanceMethod");
    }
    for (const f of t.fields) {
      seen.add(typeKind(f.type_));
      typeKinds(f.type_, seen);
    }
    for (const c of t.choices) {
      if (c.parameters.length > 0) seen.add("ChoiceParams");
      if (c.consuming === "non-consuming") seen.add("Nonconsuming");
      if (c.controller_exprs.length > 0) seen.add("ControllerExpr");
      if (c.observer_exprs.length > 0) seen.add("ChoiceObserver");
      stmtKinds(c.body, seen);
    }
  }
  for (const i of m.interfaces) {
    seen.add("Interface");
    if (i.viewtype !== null) seen.add("Viewtype");
    if (i.methods.length > 0) seen.add("InterfaceMethod");
    if (i.choices.length > 0) seen.add("InterfaceChoice");
  }
  for (const i of m.imports) {
    if (i.qualified === "qualified" && i.alias !== null) seen.add("QualifiedAlias");
  }
  for (const fn of m.functions) {
    seen.add("Function");
    if (fn.type_signature !== null) seen.add("TypeSignature");
    stmtKinds(fn.body, seen);
  }
  for (const k of Array.from(seen).sort()) report(1, k);
}
"#,
    );
    let module = parse_module(probe, "Probe.daml");
    let seen: Vec<String> = det.detect(&module).into_iter().map(|f| f.message).collect();

    for expected in [
        "Scalar:Party",
        "Scalar:Text",
        "Scalar:Decimal",
        "Scalar:Int",
        "Scalar:Bool",
        "Scalar:Date",
        "Scalar:Time",
        "Param:List",
        "Param:Optional",
        "Param:ContractId",
        "Param:TextMap",
        "Scalar:Custom",
        "Ensure",
        "ChoiceParams",
        "Nonconsuming",
        "Let",
        "Assert",
        "Fetch",
        "Archive",
        "Create",
        "Exercise",
        "TryCatch",
        "QualifiedAlias",
        "Function",
        "Binder",
        "KeyExpr",
        "KeyType",
        "Maintainer",
        "SignatoryExpr",
        "ControllerExpr",
        "ChoiceObserver",
        "InterfaceInstance",
        "InstanceMethod",
        "Interface",
        "Viewtype",
        "InterfaceMethod",
        "InterfaceChoice",
        "TypeSignature",
        "Type:Lit",
        "Expr:Var",
        "Expr:Con",
        "Expr:Lit",
        "Expr:App",
        "Expr:BinOp",
        "Expr:Neg",
        "Expr:Lambda",
        "Expr:If",
        "Expr:Case",
        "Expr:LetIn",
        "Expr:Record",
        "Expr:Tuple",
        "Expr:List",
    ] {
        assert!(
            seen.iter().any(|m| m == expected),
            "node kind '{expected}' did not reach the script; saw: {seen:?}"
        );
    }
}

#[test]
fn demo_scripts_load() {
    assert!(load_script(&manifest_path("examples/dist/template-requires-ensure.js")).is_ok());
    assert!(load_script(&manifest_path(
        "examples/dist/consuming-choice-signatory-controller.js"
    ))
    .is_ok());
    assert!(load_script(&manifest_path("examples/dist/no-trace.js")).is_ok());
    assert!(load_script(&manifest_path("examples/dist/no-create-in-nonconsuming.js")).is_ok());
    assert!(load_script(&manifest_path("examples/dist/no-bare-contractid-field.js")).is_ok());
    assert!(load_script(&manifest_path("examples/dist/unqualified-da-import.js")).is_ok());
    assert!(load_script(&manifest_path("examples/dist/function-ledger-actions.js")).is_ok());
    assert!(load_script(&manifest_path(
        "examples/dist/choice-param-shadows-field.js"
    ))
    .is_ok());
}

#[test]
fn example_unguarded_division_flags_conditional_assert() {
    let det = load_script(&manifest_path("examples/dist/unguarded-division-ast.js")).unwrap();
    let source = r#"module CondFn where

template T
  with
    p : Party
  where
    signatory p

    choice Risky : Decimal
      with
        n : Decimal
        b : Bool
      controller p
      do
        if b then assert (n /= 0.0) else pure ()
        pure (100.0 / n)
"#;
    let module = parse_module(source, "CondFn.daml");
    let findings = det.detect(&module);
    assert_eq!(
        findings.len(),
        1,
        "conditional assert should not suppress the division"
    );
    assert!(findings[0].message.contains("'n'"));
}

#[test]
fn example_unguarded_division_respects_unconditional_assert() {
    let det = load_script(&manifest_path("examples/dist/unguarded-division-ast.js")).unwrap();
    let source = r#"module Safe where

template T
  with
    p : Party
  where
    signatory p

    choice OK : Decimal
      with
        n : Decimal
      controller p
      do
        assert (n /= 0.0)
        pure (100.0 / n)
"#;
    let module = parse_module(source, "Safe.daml");
    assert!(det.detect(&module).is_empty());
}

#[test]
fn example_signatory_controller_flags_lookalike_field() {
    let det = load_script(&manifest_path(
        "examples/dist/consuming-choice-signatory-controller.js",
    ))
    .unwrap();
    let source = r#"module SigFP where

template Bar
  with
    issuer : Party
    signatoryParty : Party
  where
    signatory issuer

    choice Grab : ContractId Bar
      controller signatoryParty
      do
        create this with issuer = issuer
"#;
    let module = parse_module(source, "SigFP.daml");
    let findings = det.detect(&module);
    assert_eq!(findings.len(), 1, "signatoryParty is not a signatory");
    assert!(findings[0].message.contains("Grab"));
}

#[test]
fn example_signatory_controller_allows_signatory_this() {
    let det = load_script(&manifest_path(
        "examples/dist/consuming-choice-signatory-controller.js",
    ))
    .unwrap();
    let source = r#"module SigOk where

template Baz
  with
    issuer : Party
    obs : Party
  where
    signatory issuer

    choice GrabA : ContractId Baz
      controller signatory this
      do
        create this with issuer = issuer

    choice GrabB : ContractId Baz
      controller signatory this, obs
      do
        create this with issuer = issuer
"#;
    let module = parse_module(source, "SigOk.daml");
    assert!(det.detect(&module).is_empty());
}

#[test]
fn example_no_create_in_nonconsuming_descends_branch_arms() {
    let det = load_script(&manifest_path("examples/dist/no-create-in-nonconsuming.js")).unwrap();
    let source = r#"module BranchCreate where

template T
  with
    p : Party
  where
    signatory p

    nonconsuming choice Fork : ContractId T
      with
        flag : Bool
      controller p
      do
        if flag then do
          create this
        else do
          create this
"#;
    let module = parse_module(source, "BranchCreate.daml");
    let findings = det.detect(&module);
    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.contains("Fork"));
}

#[test]
fn example_function_ledger_actions_descends_branch_arms() {
    let det = load_script(&manifest_path("examples/dist/function-ledger-actions.js")).unwrap();
    let source = r#"module BranchLedger where

template T
  with
    p : Party
  where
    signatory p

branchArchive : ContractId T -> Bool -> Update ()
branchArchive cid flag = do
  if flag then do
    archive cid
  else do
    archive cid
"#;
    let module = parse_module(source, "BranchLedger.daml");
    let findings = det.detect(&module);
    assert_eq!(findings.len(), 1);
    assert!(findings[0].message.contains("branchArchive"));
}

#[test]
fn example_no_trace_ignores_block_comment() {
    let det = load_script(&manifest_path("examples/dist/no-trace.js")).unwrap();
    let source = r#"module BlockComment where

{- This module used to call trace for debugging.
   We removed it. -}
foo : Int
foo = 1
"#;
    let module = parse_module(source, "BlockComment.daml");
    assert!(det.detect(&module).is_empty());
}

#[test]
fn example_no_trace_ignores_string_literal() {
    let det = load_script(&manifest_path("examples/dist/no-trace.js")).unwrap();
    let source = r#"module Trace2 where

msg : Text
msg = "please trace this transaction"
"#;
    let module = parse_module(source, "Trace2.daml");
    assert!(det.detect(&module).is_empty());
}

#[test]
fn example_no_trace_still_flags_real_call() {
    let det = load_script(&manifest_path("examples/dist/no-trace.js")).unwrap();
    let source = r#"module RealTrace where

foo : Int -> Int
foo x = trace "dbg" (x + 1)
"#;
    let module = parse_module(source, "RealTrace.daml");
    let findings = det.detect(&module);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].line, LineNumber::new(4));
}
