//! Tests for structured `data`/`newtype`/`type` declaration parsing: the
//! `constructors`/`synonym`/`deriving` fields on [`Decl::TypeDef`]. These are
//! the analysis-truth payload that lets detectors reason about user-defined
//! types; the parser fills them additively beside the lossless spans, so the
//! span/render oracles in `span_tests` keep proving nothing was lost.

#![cfg(test)]

use crate::ast::*;
use crate::parse::parse_module;
use std::path::PathBuf;

fn parse(src: &str) -> Module {
    parse_module(src).0
}

/// Parse a snippet under a module header (the corpus always has one).
fn wrap(src: &str) -> Module {
    parse(&format!("module M where\n{src}\n"))
}

/// The `data`/`type`/... declaration named `name`.
fn typedef<'a>(m: &'a Module, name: &str) -> &'a Decl {
    m.decls
        .iter()
        .find(|d| matches!(d, Decl::TypeDef { name: n, .. } if n == name))
        .unwrap_or_else(|| panic!("typedef {name} not found"))
}

fn con(name: &str) -> Type {
    Type::Con {
        qualifier: None,
        name: name.to_string(),
        span: Span::default(),
    }
}

fn app(head: Type, args: Vec<Type>) -> Type {
    Type::App(Box::new(head), args, Span::default())
}

fn var(name: &str) -> Type {
    Type::Var(name.to_string(), Span::default())
}

#[test]
fn record_data_decl_exposes_fields() {
    // The core analyzer payload: a `with` record's fields and their types must
    // be visible, not collapsed into opaque text.
    let m = wrap("data Asset = Asset with\n    amount : Decimal\n    owner : Party\n");
    let Decl::TypeDef {
        constructors,
        synonym,
        deriving,
        ..
    } = typedef(&m, "Asset")
    else {
        panic!("expected TypeDef");
    };
    assert!(synonym.is_none());
    assert!(deriving.is_empty());
    assert_eq!(constructors.len(), 1);
    let c = &constructors[0];
    assert_eq!(c.name, "Asset");
    assert!(c.arg_types.is_empty());
    assert_eq!(c.fields.len(), 2);
    assert_eq!(c.fields[0].name, "amount");
    assert_eq!(c.fields[0].ty, Some(con("Decimal")));
    assert_eq!(c.fields[1].name, "owner");
    assert_eq!(c.fields[1].ty, Some(con("Party")));
}

#[test]
fn single_line_record() {
    let m = wrap("data P = P with amount : Decimal");
    let Decl::TypeDef { constructors, .. } = typedef(&m, "P") else {
        panic!()
    };
    assert_eq!(constructors.len(), 1);
    assert_eq!(constructors[0].fields.len(), 1);
    assert_eq!(constructors[0].fields[0].name, "amount");
    assert_eq!(constructors[0].fields[0].ty, Some(con("Decimal")));
}

#[test]
fn enum_constructors_and_deriving() {
    let m = wrap("data MyEnum = MyEnum1 | MyEnum2 deriving (Show, Eq)");
    let Decl::TypeDef {
        constructors,
        deriving,
        ..
    } = typedef(&m, "MyEnum")
    else {
        panic!()
    };
    let names: Vec<&str> = constructors.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, ["MyEnum1", "MyEnum2"]);
    assert!(constructors
        .iter()
        .all(|c| c.fields.is_empty() && c.arg_types.is_empty()));
    assert_eq!(deriving, &["Show".to_string(), "Eq".to_string()]);
}

#[test]
fn multiline_enum_constructors() {
    // The finance-corpus shape: `data Lockers = CustodianOnly | ... ` spread
    // across continuation lines (no layout block, just indentation).
    let m = wrap("data Lockers\n  = CustodianOnly\n  | RegulatorOnly\n  | CustodianAndRegulator\n");
    let Decl::TypeDef { constructors, .. } = typedef(&m, "Lockers") else {
        panic!()
    };
    let names: Vec<&str> = constructors.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(
        names,
        ["CustodianOnly", "RegulatorOnly", "CustodianAndRegulator"]
    );
}

#[test]
fn sum_with_positional_args() {
    let m = wrap("data Tree = Leaf | Node Int Text");
    let Decl::TypeDef { constructors, .. } = typedef(&m, "Tree") else {
        panic!()
    };
    assert_eq!(constructors.len(), 2);
    assert_eq!(constructors[0].name, "Leaf");
    assert!(constructors[0].arg_types.is_empty());
    assert_eq!(constructors[1].name, "Node");
    assert_eq!(constructors[1].arg_types, vec![con("Int"), con("Text")]);
}

#[test]
fn newtype_wraps_a_type() {
    let m = wrap("newtype Money = Money Decimal");
    let Decl::TypeDef {
        keyword,
        constructors,
        ..
    } = typedef(&m, "Money")
    else {
        panic!()
    };
    assert_eq!(keyword, "newtype");
    assert_eq!(constructors.len(), 1);
    assert_eq!(constructors[0].name, "Money");
    assert_eq!(constructors[0].arg_types, vec![con("Decimal")]);
}

#[test]
fn type_synonym_keeps_aliased_type() {
    let m = wrap("type Name = Text");
    let Decl::TypeDef {
        keyword,
        synonym,
        constructors,
        ..
    } = typedef(&m, "Name")
    else {
        panic!()
    };
    assert_eq!(keyword, "type");
    assert!(constructors.is_empty());
    assert_eq!(*synonym, Some(con("Text")));
}

#[test]
fn type_synonym_application() {
    let m = wrap("type Amounts = Map Party Decimal");
    let Decl::TypeDef { synonym, .. } = typedef(&m, "Amounts") else {
        panic!()
    };
    assert_eq!(
        *synonym,
        Some(app(con("Map"), vec![con("Party"), con("Decimal")]))
    );
}

#[test]
fn type_parameters_are_skipped() {
    // `a` is a type parameter on the LHS, not a constructor; the field type is
    // the type variable `a`.
    let m = wrap("data Box a = Box with\n    value : a\n");
    let Decl::TypeDef { constructors, .. } = typedef(&m, "Box") else {
        panic!()
    };
    assert_eq!(constructors[0].name, "Box");
    assert_eq!(constructors[0].fields.len(), 1);
    assert_eq!(constructors[0].fields[0].name, "value");
    assert_eq!(constructors[0].fields[0].ty, Some(var("a")));
}

#[test]
fn multiline_record_with_dedented_deriving() {
    // `deriving` dedents out of the `with` layout block, so it belongs to the
    // decl, not to the last field's type.
    let m = wrap("data P = P\n  with\n    x : Int\n  deriving (Eq)\n");
    let Decl::TypeDef {
        constructors,
        deriving,
        ..
    } = typedef(&m, "P")
    else {
        panic!()
    };
    assert_eq!(constructors.len(), 1);
    assert_eq!(constructors[0].fields.len(), 1);
    assert_eq!(constructors[0].fields[0].name, "x");
    assert_eq!(deriving, &["Eq".to_string()]);
}

#[test]
fn class_and_instance_stay_opaque() {
    // We do not model these; they must parse without inventing constructors.
    let m = wrap("class Foo a where\n  bar : a -> Int\n");
    let Decl::TypeDef {
        keyword,
        constructors,
        synonym,
        ..
    } = typedef(&m, "Foo")
    else {
        panic!()
    };
    assert_eq!(keyword, "class");
    assert!(constructors.is_empty());
    assert!(synonym.is_none());
}

#[test]
fn deriving_strategy_keyword_still_captures_classes() {
    // `deriving stock (Show, Eq)` — the strategy word must not hide the classes.
    let m = wrap("data T = A | B deriving stock (Show, Eq)");
    let Decl::TypeDef { deriving, .. } = typedef(&m, "T") else {
        panic!()
    };
    assert_eq!(deriving, &["Show".to_string(), "Eq".to_string()]);
}

#[test]
fn multiple_deriving_clauses_are_all_captured() {
    let m = wrap("data T = A | B deriving (Show) deriving (Eq, Ord)");
    let Decl::TypeDef { deriving, .. } = typedef(&m, "T") else {
        panic!()
    };
    assert_eq!(
        deriving,
        &["Show".to_string(), "Eq".to_string(), "Ord".to_string()]
    );
}

#[test]
fn infix_constructor_stays_opaque_not_fabricated() {
    // `Int :+: Int` must NOT be recorded as a constructor literally named "Int"
    // (a wrong fact); the decl falls back to opaque (no constructors).
    let m = wrap("data T = Int :+: Int");
    let Decl::TypeDef { constructors, .. } = typedef(&m, "T") else {
        panic!()
    };
    assert!(
        constructors.is_empty(),
        "infix constructor must not fabricate a name, got {constructors:?}"
    );
}

#[test]
fn strictness_bang_constructor_stays_opaque_not_understated() {
    // `T !Int !Text` takes two args; rather than silently record it as nullary,
    // the decl falls back to opaque.
    let m = wrap("data T = T !Int !Text");
    let Decl::TypeDef { constructors, .. } = typedef(&m, "T") else {
        panic!()
    };
    assert!(constructors.is_empty());
}

#[test]
fn empty_with_block_does_not_swallow_next_decl() {
    // A constructor whose `with` fields are all commented out leaves an empty
    // (dangling) layout block. The decl must fall back to opaque WITHOUT
    // consuming the following declaration — the real-corpus `SubmitError` shape
    // (its `ContractIdInContractKey` constructor) that exposed an unbalanced
    // brace and swallowed the `instance Show SubmitError` that followed it.
    let m = wrap(
        "data E\n  = First with\n      x : Int\n  | Empty with\n      -- all fields commented out\n  | Last with\n      y : Int\n\nafter : Int\nafter = 1\n",
    );
    // The data decl is still present (now opaque) ...
    assert!(m
        .decls
        .iter()
        .any(|d| matches!(d, Decl::TypeDef { name, .. } if name == "E")));
    // ... and crucially the following declaration survived.
    assert!(m
        .decls
        .iter()
        .any(|d| matches!(d, Decl::Function(f) if f.name == "after")));
}

/// Phase 2.4: data/record fields from representative finance files are visible
/// in the AST. Aggregate over the vendored corpus so the assertion survives file
/// renames, plus one concrete spot-check on a stable core type.
#[test]
fn finance_corpus_exposes_record_fields() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/daml-finance/daml");
    if !root.exists() {
        // Off-workspace (published crate) the vendored corpus is absent and the
        // test skips — but under CI a missing corpus must fail loud, never read
        // as a silent pass (matches the span_tests corpus oracles).
        assert!(
            std::env::var_os("CI").is_none(),
            "vendored corpus missing under CI (was it committed?): {}",
            root.display()
        );
        eprintln!("corpus absent (published crate?), skipping");
        return;
    }
    let mut files = Vec::new();
    collect(&root, &mut files);
    assert!(
        files.len() > 600,
        "finance corpus incomplete: {}",
        files.len()
    );

    let mut decls_with_fields = 0usize;
    let mut decls_with_ctors = 0usize;
    let mut lock_lockers_seen = false;
    for f in &files {
        let Ok(src) = std::fs::read_to_string(f) else {
            continue;
        };
        for d in &parse(&src).decls {
            let Decl::TypeDef {
                name, constructors, ..
            } = d
            else {
                continue;
            };
            if !constructors.is_empty() {
                decls_with_ctors += 1;
            }
            if constructors.iter().any(|c| !c.fields.is_empty()) {
                decls_with_fields += 1;
            }
            // Concrete spot-check: the core `Lock` record exposes `lockers`.
            if name == "Lock"
                && constructors
                    .iter()
                    .any(|c| c.fields.iter().any(|fl| fl.name == "lockers"))
            {
                lock_lockers_seen = true;
            }
        }
    }
    assert!(
        decls_with_ctors > 100,
        "expected many constructor-bearing decls, got {decls_with_ctors}"
    );
    assert!(
        decls_with_fields > 50,
        "expected many record-field-bearing decls, got {decls_with_fields}"
    );
    assert!(
        lock_lockers_seen,
        "expected a `Lock` record exposing a `lockers` field"
    );
}

fn collect(dir: &std::path::Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            collect(&p, out);
        } else if p.extension().is_some_and(|x| x == "daml") {
            out.push(p);
        }
    }
}
