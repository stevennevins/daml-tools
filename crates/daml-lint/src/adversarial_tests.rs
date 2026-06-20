//! Boundary smoke for adversarial syntax after parser-owned coverage.
//!
//! `daml-parser` owns lexer/layout/recovery stress tests. This module keeps
//! linter-specific assertions that those parser shapes lower to rule-facing IR
//! without phantom templates, functions, choices, or statements.

#![cfg(test)]

use crate::ir::*;
use crate::parser::parse_daml_with_diagnostics;
use std::path::Path;
use std::time::Instant;

fn parse(source: &str) -> DamlModule {
    parse_daml_with_diagnostics(source, Path::new("hostile.daml")).0
}

fn single_var(exprs: &[Expr], expected: &str) -> bool {
    matches!(exprs, [Expr::Var { name, .. }] if name == expected)
}

#[test]
fn comments_and_strings_create_no_phantom_linter_ir() {
    let m = parse(
        "module M where\n\
         -- template Fake with choice Evil : () controller attacker\n\
         {- template Hidden\n\
              with\n\
                x : Party\n\
         -}\n\
         f = \"template Fake with x : Party where signatory attacker\"\n\
         g = \"exercise cid Evil\"\n",
    );
    assert!(m.templates.is_empty());
    assert_eq!(m.functions.len(), 2);
    for func in &m.functions {
        assert!(
            !func
                .body
                .iter()
                .any(|s| matches!(s, Statement::Exercise { .. })),
            "string literal must not become an Exercise"
        );
    }
}

#[test]
fn operator_that_looks_like_comment_lowers_as_binop() {
    let m = parse("module M where\nf = a --> b\ng = c --- this is a comment\n");
    assert_eq!(m.functions.len(), 2);
    assert!(m.functions[0].body.iter().any(|s| matches!(
        s,
        Statement::Other {
            expr: Expr::BinOp { op, .. },
            ..
        } if op == "-->"
    )));
}

#[test]
fn compact_template_layout_lowers_fields_and_signatories() {
    let m = parse(concat!(
        "module M where\n",
        "template T with { x : Party } where { signatory x }\n",
    ));
    assert_eq!(m.templates.len(), 1);
    assert_eq!(m.templates[0].fields.len(), 1);
    assert!(single_var(&m.templates[0].signatory_exprs, "x"));
}

#[test]
fn large_source_still_lowers_many_templates_through_linter_boundary() {
    let mut src = String::from("module Big where\n\n");
    for i in 0..1000 {
        src.push_str(&format!(
            "template T{i}\n  with\n    owner : Party\n    amount : Decimal\n  where\n    signatory owner\n    ensure amount > 0.0\n\n    choice C{i} : ()\n      controller owner\n      do\n        pure ()\n\n"
        ));
    }
    assert!(src.lines().count() > 10_000);

    let start = Instant::now();
    let (module, diagnostics) = parse_daml_with_diagnostics(&src, Path::new("big.daml"));

    assert!(diagnostics.is_empty());
    assert_eq!(module.templates.len(), 1000);
    assert!(
        start.elapsed().as_secs() < 5,
        "10k-line linter lowering took {:?}",
        start.elapsed()
    );
}

#[test]
fn compact_and_empty_choice_headers_lower_to_linter_choices() {
    let m = parse(concat!(
        "module M where\n",
        "template T\n",
        "  with\n",
        "    owner : Party\n",
        "  where\n",
        "    signatory owner\n",
        "    choice F : ()\n",
        "      with -- superfluous, no fields\n",
        "      controller owner\n",
        "      do pure ()\n",
        "    choice G : ContractId T with\n",
        "      controller owner\n",
        "      do pure self\n",
        "    choice H : () with\n",
        "        extra : Party\n",
        "      controller owner\n",
        "      do pure ()\n",
    ));
    let choices = &m.templates[0].choices;
    let names: Vec<&str> = choices.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["F", "G", "H"]);
    assert!(single_var(&choices[0].controller_exprs, "owner"));
    assert!(choices[0].parameters.is_empty());
    assert!(choices[1].parameters.is_empty());
    assert_eq!(choices[2].parameters.len(), 1);
    assert_eq!(choices[2].parameters[0].name, "extra");
}
