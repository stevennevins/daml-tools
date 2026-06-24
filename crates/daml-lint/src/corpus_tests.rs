//! AST ground-truth integration tests against the daml-finance corpus.
//!
//! Facts below were hand-verified against the sources once (grep + read);
//! these tests pin them so parser changes cannot silently regress structure
//! extraction. The corpus is vendored under corpus/daml-finance/.

#![cfg(test)]
#![allow(clippy::unwrap_used)]

use crate::ir::*;
use crate::parser::parse_daml_with_diagnostics;
use std::path::{Path, PathBuf};

pub fn corpus_root() -> PathBuf {
    // Shared integration corpus, vendored once at the workspace root and used
    // by both daml-parser (lex/layout gate) and daml-lint (parse/IR gate).
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/daml-finance/daml")
}

/// True if the vendored corpus is present. Absent corpus is a legitimate skip
/// OFF the workspace (e.g. a published crate), but under CI it must be present —
/// fail loud so a missing/forgotten corpus can't pass green. Mirrors the guard
/// in daml-parser's `span_tests`.
fn corpus_present() -> bool {
    let root = corpus_root();
    if root.exists() {
        return true;
    }
    assert!(
        std::env::var_os("CI").is_none(),
        "vendored corpus missing under CI (was it committed?): {}",
        root.display()
    );
    eprintln!("corpus absent (published crate?), skipping");
    false
}

fn load(rel: &str) -> Option<DamlModule> {
    if !corpus_present() {
        return None;
    }
    let path = corpus_root().join("src/main/daml/Daml/Finance").join(rel);
    assert!(path.exists(), "corpus file missing: {}", path.display());
    let source = std::fs::read_to_string(&path).unwrap();
    let result = parse_daml_with_diagnostics(&source, Path::new(rel));
    assert!(
        result.diagnostics.is_empty(),
        "parse diagnostics in {rel}: {:?}",
        result.diagnostics
    );
    Some(result.module)
}

fn template<'a>(m: &'a DamlModule, name: &str) -> &'a Template {
    m.templates
        .iter()
        .find(|t| t.name == name)
        .unwrap_or_else(|| panic!("template {name} not found"))
}

fn interface<'a>(m: &'a DamlModule, name: &str) -> &'a Interface {
    m.interfaces
        .iter()
        .find(|i| i.name == name)
        .unwrap_or_else(|| panic!("interface {name} not found"))
}

fn expr_text(expr: &Expr) -> String {
    match expr {
        Expr::Var {
            qualifier, name, ..
        }
        | Expr::Con {
            qualifier, name, ..
        } => qualifier
            .as_ref()
            .map_or_else(|| name.clone(), |q| format!("{q}.{name}")),
        Expr::App { func, args, .. } => {
            let mut parts = Vec::with_capacity(args.len() + 1);
            parts.push(expr_text(func));
            parts.extend(args.iter().map(expr_text));
            parts.join(" ")
        }
        Expr::List { items, .. } => {
            let parts: Vec<String> = items.iter().map(expr_text).collect();
            format!("[{}]", parts.join(", "))
        }
        Expr::BinOp { op, lhs, rhs, .. } if op == "." => {
            format!("{}.{}", expr_text(lhs), expr_text(rhs))
        }
        Expr::Unknown { raw, .. } => raw.clone(),
        other => format!("{other:?}"),
    }
}

fn expr_texts(exprs: &[Expr]) -> Vec<String> {
    exprs.iter().map(expr_text).collect()
}

fn con_name(ty: Option<&TypeNode>) -> Option<&str> {
    match ty {
        Some(TypeNode::Con { name, .. }) => Some(name.as_str()),
        _ => None,
    }
}

fn has_kind(stmts: &[Statement], pred: &dyn Fn(&Statement) -> bool) -> bool {
    stmts.iter().any(|s| {
        pred(s)
            || matches!(s, Statement::TryCatch { try_body, catch_body, .. }
                if has_kind(try_body, pred) || has_kind(catch_body, pred))
            // An `if`/`case` keeps its arms as separate scopes; the effect
            // may live inside one of them.
            || matches!(s, Statement::Branch { arms, .. }
                if arms.iter().any(|arm| has_kind(&arm.body, pred)))
    })
}

#[test]
fn settlement_instruction_template() {
    let Some(m) = load("Settlement/V4/Instruction.daml") else {
        return;
    };
    assert_eq!(m.name, "Daml.Finance.Settlement.V4.Instruction");
    let qualified_aliased = m
        .imports
        .iter()
        .filter(|i| i.qualified.is_qualified() && i.alias.is_some())
        .count();
    assert_eq!(qualified_aliased, 9);

    let t = template(&m, "Instruction");
    assert_eq!(t.fields.len(), 12);
    assert_eq!(
        expr_texts(&t.signatory_exprs),
        vec![
            "instructor",
            "consenters",
            "signedSenders",
            "signedReceivers"
        ]
    );
    assert_eq!(con_name(t.key_type.as_ref()), Some("InstructionKey"));
    assert!(t.key_expr.is_some());
    let instances: Vec<&str> = t
        .interface_instances
        .iter()
        .map(|i| i.interface_name.as_str())
        .collect();
    assert_eq!(instances, vec!["Disclosure.I", "Instruction.I"]);

    // releasePreviousAllocation exercises Lockable.Release and fetches the
    // holding (hand-verified at source lines ~301-311).
    let f = m
        .functions
        .iter()
        .find(|f| f.name == "releasePreviousAllocation")
        .expect("function releasePreviousAllocation");
    assert!(has_kind(&f.body, &|s| matches!(
        s,
        Statement::Exercise { .. }
    )));
    assert!(has_kind(&f.body, &|s| matches!(s, Statement::Fetch { .. })));
}

#[test]
fn interface_settlement_instruction() {
    let Some(m) = load("Interface/Settlement/V4/Instruction.daml") else {
        return;
    };
    let i = interface(&m, "Instruction");
    assert_eq!(i.viewtype.as_deref(), Some("V"));
    let methods: Vec<&str> = i.methods.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(methods, vec!["allocate", "approve", "execute", "cancel"]);
    let choices: Vec<(&str, bool)> = i
        .choices
        .iter()
        .map(|c| (c.name.as_str(), c.consuming.is_consuming()))
        .collect();
    assert_eq!(
        choices,
        vec![
            ("GetView", false),
            ("Allocate", true),
            ("Approve", true),
            ("Execute", true),
            ("Cancel", true),
        ]
    );
}

#[test]
fn account_template_with_ensure() {
    let Some(m) = load("Account/V4/Account.daml") else {
        return;
    };
    let t = template(&m, "Account");
    assert_eq!(t.fields.len(), 8);
    assert_eq!(
        expr_texts(&t.signatory_exprs),
        vec!["custodian", "owner", "Lockable.getLockers this"]
    );
    let ensure = t.ensure_clause.as_ref().expect("Account has ensure");
    // `ensure isValidLock lock && (not . Set.null $ controllers.outgoing)`
    assert!(matches!(&ensure.expr, Expr::BinOp { op, .. } if op == "&&"));
    let instances: Vec<&str> = t
        .interface_instances
        .iter()
        .map(|i| i.interface_name.as_str())
        .collect();
    assert_eq!(instances, vec!["Account.I", "Lockable.I", "Disclosure.I"]);
    // Module also declares the account Factory template.
    assert_eq!(template(&m, "Factory").fields.len(), 2);
}

#[test]
fn interface_account_reference_template() {
    let Some(m) = load("Interface/Account/V4/Account.daml") else {
        return;
    };
    let i = interface(&m, "Account");
    assert_eq!(i.requires, vec!["Disclosure.I"]);
    let names: Vec<&str> = i.choices.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["GetView", "Credit", "Debit", "Remove"]);
    let remove = i.choices.iter().find(|c| c.name == "Remove").unwrap();
    assert_eq!(expr_texts(&remove.controller_exprs), vec!["signatory this"]);

    // The Reference helper template: choices controlled by `signatory this`.
    let r = template(&m, "Reference");
    assert_eq!(r.fields.len(), 3);
    assert_eq!(con_name(r.key_type.as_ref()), Some("AccountKey"));
    let by_name = |n: &str| r.choices.iter().find(|c| c.name == n).unwrap();
    assert!(!by_name("GetCid").consuming.is_consuming());
    assert_eq!(
        expr_texts(&by_name("GetCid").controller_exprs),
        vec!["viewer"]
    );
    assert_eq!(
        expr_texts(&by_name("SetCid").controller_exprs),
        vec!["signatory this"]
    );
    assert_eq!(
        expr_texts(&by_name("SetObservers").controller_exprs),
        vec!["signatory this"]
    );
    // Structured controller expression: App of Var "signatory" to "this".
    match &by_name("SetCid").controller_exprs[0] {
        Expr::App { func, args, .. } => {
            assert!(matches!(&**func, Expr::Var { name, .. } if name == "signatory"));
            assert!(matches!(&args[0], Expr::Var { name, .. } if name == "this"));
        }
        other => panic!("expected App for 'signatory this', got {other:?}"),
    }
}

#[test]
fn holding_fungible_template() {
    let Some(m) = load("Holding/V4/Fungible.daml") else {
        return;
    };
    let t = template(&m, "Fungible");
    assert_eq!(t.fields.len(), 5);
    assert!(t.ensure_clause.is_some());
    assert_eq!(
        expr_texts(&t.signatory_exprs),
        vec![
            "account.custodian",
            "account.owner",
            "Lockable.getLockers this"
        ]
    );
    assert_eq!(t.interface_instances.len(), 4);
}

#[test]
fn interface_holding_requires() {
    let Some(m) = load("Interface/Holding/V4/Holding.daml") else {
        return;
    };
    let i = interface(&m, "Holding");
    assert_eq!(i.requires, vec!["Lockable.I", "Disclosure.I"]);
    assert_eq!(i.viewtype.as_deref(), Some("V"));
}

#[test]
fn settlement_batch_module() {
    let Some(m) = load("Settlement/V4/Batch.daml") else {
        return;
    };
    let t = template(&m, "Batch");
    assert_eq!(t.fields.len(), 8);
    assert_eq!(
        expr_texts(&t.signatory_exprs),
        vec!["instructor", "consenters"]
    );
    let fns: Vec<&str> = m.functions.iter().map(|f| f.name.as_str()).collect();
    for expected in ["routedSteps", "instructionIds", "buildKey"] {
        assert!(fns.contains(&expected), "function {expected} missing");
    }
}

#[test]
fn interface_disclosure_module() {
    let Some(m) = load("Interface/Util/V3/Disclosure.daml") else {
        return;
    };
    let i = interface(&m, "Disclosure");
    assert_eq!(i.viewtype.as_deref(), Some("V"));
    let methods: Vec<&str> = i.methods.iter().map(|m| m.name.as_str()).collect();
    assert_eq!(
        methods,
        vec!["setObservers", "addObservers", "removeObservers"]
    );
    let choices: Vec<&str> = i.choices.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        choices,
        vec!["GetView", "SetObservers", "AddObservers", "RemoveObservers"]
    );
    assert!(m.functions.iter().any(|f| f.name == "flattenObservers"));
}

#[test]
fn token_instrument_template() {
    let Some(m) = load("Instrument/Token/V4/Instrument.daml") else {
        return;
    };
    let t = template(&m, "Instrument");
    assert_eq!(t.fields.len(), 8);
    assert_eq!(expr_texts(&t.signatory_exprs), vec!["depository", "issuer"]);
    assert_eq!(t.interface_instances.len(), 3);
}

#[test]
fn lifecycle_distribution_rule_template() {
    let Some(m) = load("Lifecycle/V4/Rule/Distribution.daml") else {
        return;
    };
    let t = template(&m, "Rule");
    assert_eq!(t.fields.len(), 5);
    assert_eq!(expr_texts(&t.signatory_exprs), vec!["providers"]);
    let instances: Vec<&str> = t
        .interface_instances
        .iter()
        .map(|i| i.interface_name.as_str())
        .collect();
    assert_eq!(instances, vec!["Lifecycle.I"]);
}

/// Whole-corpus phase gate at the parser level: every file parses with
/// zero diagnostics.
#[test]
fn corpus_parses_clean() {
    if !corpus_present() {
        return;
    }
    let root = corpus_root();
    let mut files = Vec::new();
    collect(&root, &mut files);
    assert!(files.len() > 600, "corpus incomplete: {}", files.len());
    let mut diag_count = 0;
    for f in &files {
        let src = std::fs::read_to_string(f).unwrap();
        let result = parse_daml_with_diagnostics(&src, f);
        if !result.diagnostics.is_empty() {
            eprintln!("{}: {:?}", f.display(), result.diagnostics);
        }
        diag_count += result.diagnostics.len();
    }
    assert_eq!(diag_count, 0, "parse diagnostics across corpus");
}

/// Lossless-trivia gate over a corpus: every file that lexes clean must
/// reconstruct byte-for-byte from token + trivia spans. Returns
/// (`files_checked`, `files_with_lex_errors`); panics on any round-trip failure.
fn round_trip_corpus(root: &Path) -> (usize, usize) {
    let mut files = Vec::new();
    collect(root, &mut files);
    let mut checked = 0;
    let mut lex_error_files = 0;
    for f in &files {
        // Non-UTF8 fixtures never reach the lexer (it takes &str), so they
        // are outside the losslessness promise too.
        let Ok(src) = std::fs::read_to_string(f) else {
            lex_error_files += 1;
            continue;
        };
        let lexed = daml_parser::lexer::lex_with_trivia(&src);
        let tokens = lexed.tokens;
        let trivia = lexed.trivia;
        let errors = lexed.errors;
        if !errors.is_empty() {
            // A lex error drops bytes by design (e.g. stray character);
            // losslessness is only promised for lexable files.
            eprintln!("lex errors (round trip exempt): {}", f.display());
            lex_error_files += 1;
            continue;
        }
        checked += 1;
        if let Err(e) = daml_parser::lexer::render_lossless(&src, &tokens, &trivia) {
            panic!("round trip failed for {}: {e}", f.display());
        }
    }
    (checked, lex_error_files)
}

/// Formatter phase gate: the linting corpus is fully lossless — every one of
/// the 634 vendored daml-finance files reconstructs byte-for-byte from token +
/// trivia spans, none excused by lex errors. Runs in CI over the vendored
/// corpus; skips gracefully when absent (a published crate off the workspace).
#[test]
fn finance_corpus_round_trips_byte_identical() {
    if !corpus_present() {
        return;
    }
    let root = corpus_root();
    let (checked, lex_error_files) = round_trip_corpus(&root);
    assert!(checked > 600, "corpus incomplete: {checked}");
    assert_eq!(lex_error_files, 0, "finance corpus must lex clean");
}

/// Same gate over the full, hostile Daml SDK corpus (stdlib internals, broken
/// fixtures). That corpus is NOT vendored (too large), so this is an opt-in
/// LOCAL check: clone the SDK to /tmp/daml-repo to run it; it skips in CI.
#[test]
fn sdk_corpus_round_trips_byte_identical() {
    let root = Path::new("/tmp/daml-repo");
    if !root.exists() {
        return;
    }
    let (checked, exempt) = round_trip_corpus(root);
    assert!(checked > 1000, "corpus incomplete: {checked}");
    // The single exempt file is the deliberately invalid
    // compiler/damlc/tests/daml-test-files/BadUTF8.daml fixture.
    assert_eq!(exempt, 1, "exempt files beyond BadUTF8.daml");
}

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let p = entry.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|e| e == "daml") {
            out.push(p);
        }
    }
}
