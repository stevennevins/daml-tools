//! Integration tests for parser lowering into rule-facing IR via the public API.

#![allow(clippy::unwrap_used)]

use daml_lint::ir::{Expr, Statement, TypeNode};
use daml_lint::parser::{parse_daml_with_diagnostics, ParseDiagnosticCategory};
use std::path::Path;

fn parse_module(source: &str, file: &Path) -> daml_lint::ir::DamlModule {
    parse_daml_with_diagnostics(source, file).module
}

#[test]
fn test_parse_simple_template() {
    let source = r#"module Test where

template SimpleHolding
  with
    admin : Party
    amount : Decimal
  where
    signatory admin
    ensure amount > 0.0

    choice Transfer : ContractId SimpleHolding
      with
        newOwner : Party
      controller admin
      do
        create this with admin = newOwner
"#;
    let module = parse_module(source, Path::new("Test.daml"));
    assert_eq!(module.name, "Test");
    assert_eq!(module.templates.len(), 1);

    let t = &module.templates[0];
    assert_eq!(t.name, "SimpleHolding");
    assert_eq!(t.fields.len(), 2);
    assert_eq!(t.fields[0].name, "admin");
    assert!(matches!(
        &t.fields[0].type_,
        Some(TypeNode::Con { name, .. }) if name == "Party"
    ));
    assert_eq!(t.fields[1].name, "amount");
    assert!(matches!(
        &t.fields[1].type_,
        Some(TypeNode::Con { name, .. }) if name == "Decimal"
    ));
    assert!(t.ensure_clause.is_some());
    assert!(matches!(
        &t.ensure_clause.as_ref().unwrap().expr,
        Expr::BinOp { op, .. } if op == ">"
    ));
    assert_eq!(t.choices.len(), 1);
    assert_eq!(t.choices[0].name, "Transfer");
    assert_eq!(t.choices[0].parameters.len(), 1);
    // The real parser extracts structure the shim could not:
    assert!(matches!(
        &t.choices[0].return_type,
        Some(TypeNode::App { head, .. })
            if matches!(&**head, TypeNode::Con { name, .. } if name == "ContractId")
    ));
    assert!(t.choices[0]
        .body
        .iter()
        .any(|s| matches!(s, Statement::Create { template_name, .. } if template_name == "this")));
}

#[test]
fn test_parse_template_without_ensure() {
    let source = r#"module Test where

template OpenMiningRound
  with
    admin : Party
    amuletPrice : Decimal
    tickDuration : RelTime
  where
    signatory admin
"#;
    let module = parse_module(source, Path::new("Round.daml"));
    assert_eq!(module.templates.len(), 1);
    let t = &module.templates[0];
    assert_eq!(t.name, "OpenMiningRound");
    assert!(t.ensure_clause.is_none());
    assert_eq!(t.fields.len(), 3);
    assert!(matches!(
        &t.fields[1].type_,
        Some(TypeNode::Con { name, .. }) if name == "Decimal"
    ));
}

#[test]
fn test_parse_nonconsuming_choice() {
    let source = r#"module Test where

template Foo
  with
    owner : Party
  where
    signatory owner

    nonconsuming choice GetInfo : Text
      controller owner
      do
        pure "info"
"#;
    let module = parse_module(source, Path::new("Foo.daml"));
    assert_eq!(module.templates[0].choices.len(), 1);
    assert!(!module.templates[0].choices[0].consuming.is_consuming());
}

// Regression (audit F2): preconsuming and postconsuming choices DO archive
// the contract, so the IR `consuming` flag must be true for them — only
// nonconsuming is false. Rules that branch on `consuming` depend on this.
#[test]
fn test_pre_and_post_consuming_choices_are_consuming() {
    let source = r#"module Test where

template Foo
  with
    owner : Party
  where
    signatory owner

    preconsuming choice Drain : ()
      controller owner
      do
        pure ()

    postconsuming choice Close : ()
      controller owner
      do
        pure ()

    nonconsuming choice Peek : ()
      controller owner
      do
        pure ()

    choice Normal : ()
      controller owner
      do
        pure ()
"#;
    let module = parse_module(source, Path::new("Foo.daml"));
    let by = |n: &str| {
        module.templates[0]
            .choices
            .iter()
            .find(|c| c.name == n)
            .unwrap_or_else(|| panic!("choice {n} not found"))
    };
    assert!(
        by("Drain").consuming.is_consuming(),
        "preconsuming archives -> consuming"
    );
    assert!(
        by("Close").consuming.is_consuming(),
        "postconsuming archives -> consuming"
    );
    assert!(
        by("Normal").consuming.is_consuming(),
        "default choice is consuming"
    );
    assert!(
        !by("Peek").consuming.is_consuming(),
        "nonconsuming is not consuming"
    );
}

#[test]
fn test_comment_with_exercise_keyword_is_not_a_statement() {
    let source = r#"module Test where

template Foo
  with
    owner : Party
  where
    signatory owner

    choice Go : ()
      controller owner
      do
        -- electing to exercise the option
        pure ()
"#;
    let module = parse_module(source, Path::new("Foo.daml"));
    let body = &module.templates[0].choices[0].body;
    assert!(
        !body.iter().any(|s| matches!(s, Statement::Exercise { .. })),
        "comment text must not become an Exercise statement: {body:?}"
    );
}

#[test]
fn test_exercise_extracts_cid_and_choice() {
    let source = r#"module Test where

template Foo
  with
    owner : Party
  where
    signatory owner

    choice Go : ()
      controller owner
      do
        result <- exercise optionCid Elect with electorParty = owner
        pure ()
"#;
    let module = parse_module(source, Path::new("Foo.daml"));
    let body = &module.templates[0].choices[0].body;
    let ex = body
        .iter()
        .find_map(|s| match s {
            Statement::Exercise {
                cid,
                choice_name,
                argument,
                ..
            } => Some((cid.clone(), choice_name.clone(), argument.clone())),
            _ => None,
        })
        .expect("exercise statement");
    assert!(matches!(ex.0, Expr::Var { name, .. } if name == "optionCid"));
    assert_eq!(ex.1, "Elect");
    assert!(matches!(
        ex.2,
        Some(Expr::Record { base, fields, .. })
            if matches!(base.as_ref(), Expr::Con { name, .. } if name == "Elect")
                && fields.len() == 1
                && fields[0].name == "electorParty"
    ));
}

#[test]
fn test_exercise_without_payload_has_no_argument() {
    let source = r#"module Test where

template Foo
  with
    owner : Party
  where
    signatory owner

    choice Go : ()
      controller owner
      do
        result <- exercise optionCid Elect
        pure ()
"#;
    let module = parse_module(source, Path::new("Foo.daml"));
    let body = &module.templates[0].choices[0].body;
    let ex = body
        .iter()
        .find_map(|s| match s {
            Statement::Exercise {
                cid,
                choice_name,
                argument,
                ..
            } => Some((cid, choice_name, argument)),
            _ => None,
        })
        .expect("exercise statement");
    assert!(matches!(ex.0, Expr::Var { name, .. } if name == "optionCid"));
    assert_eq!(ex.1, "Elect");
    assert!(ex.2.is_none());
}

#[test]
fn test_signatory_list_flattened() {
    let source = r#"module Test where

template Foo
  with
    a : Party
    b : Party
  where
    signatory [a, b]
"#;
    let module = parse_module(source, Path::new("Foo.daml"));
    assert!(matches!(
        &module.templates[0].signatory_exprs[0],
        Expr::List { items, .. }
            if matches!(&items[0], Expr::Var { name, .. } if name == "a")
                && matches!(&items[1], Expr::Var { name, .. } if name == "b")
    ));
}

#[test]
fn test_interface_methods_are_not_functions() {
    let source = r#"module Test where

interface Base where
  viewtype View
  getOwner : Party

  nonconsuming choice GetView : View
    with
      viewer : Party
    controller viewer
    do
      pure (view this)
"#;
    let module = parse_module(source, Path::new("Base.daml"));
    assert!(
        module.functions.is_empty(),
        "interface methods must not be extracted as top-level functions: {:?}",
        module.functions.iter().map(|f| &f.name).collect::<Vec<_>>()
    );
}

#[test]
fn parse_result_carries_named_fields() {
    let source = "module Hostile where\nf = \"unterminated";
    let result = parse_daml_with_diagnostics(source, Path::new("Hostile.daml"));

    assert_eq!(result.module.name, "Hostile");
    assert!(!result.diagnostics.is_empty());
    assert!(matches!(
        result.diagnostics[0].category,
        ParseDiagnosticCategory::LexicalError
    ));
}

#[test]
fn type_string_literal_projects_into_lint_ir() {
    let source = r#"module Test where

template T
  with
    ref : HasField "cid" t (ContractId Asset)
  where
    signatory ref
"#;
    let module = parse_module(source, Path::new("Test.daml"));
    assert_eq!(module.templates.len(), 1);
    let field_ty = module.templates[0].fields[0]
        .type_
        .as_ref()
        .expect("field type");
    assert!(matches!(
        field_ty,
        TypeNode::App { head, args, .. }
            if matches!(&**head, TypeNode::Con { name, .. } if name == "HasField")
                && matches!(
                    &args[..],
                    [
                        TypeNode::Lit { value, .. },
                        TypeNode::Var { .. },
                        TypeNode::App { .. },
                    ] if value == "cid"
                )
    ));
}
