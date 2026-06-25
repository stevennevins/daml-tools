//! Integration tests for AST byte spans and the `render_from_ast` oracle.
//!
//! Tightness tests pin specific node spans to exact source substrings so the
//! spans can't silently degrade to a useless catch-all. The corpus test runs
//! the `render_from_ast` losslessness/nesting oracle over the vendored
//! daml-finance corpus shared at the workspace root.

#![allow(clippy::unwrap_used)]

use daml_parser::ast::*;
use daml_parser::ast_span::{render_from_ast, AstSpanError};
use daml_parser::lexer::lex_with_trivia;
use daml_parser::parse::parse_module;
use std::path::{Path, PathBuf};

/// Run the oracle the way daml-fmt will: AST + the lexer's trivia.
fn render(src: &str) -> Result<String, AstSpanError> {
    let (_, trivia, _) = lex_with_trivia(src).into_parts();
    render_from_ast(src, &parse(src), &trivia)
}

fn parse(src: &str) -> Module {
    let (module, diagnostics) = parse_module(src).into_parts();
    assert!(
        diagnostics.is_empty(),
        "span test source must parse without diagnostics: {diagnostics:?}"
    );
    module
}

/// Substring of `src` covered by `span`.
fn text(src: &str, span: Span) -> &str {
    span.get(src).expect("span must be valid UTF-8 slice")
}

fn first_function<'a>(m: &'a Module, name: &str) -> &'a FunctionDecl {
    m.decls
        .iter()
        .find_map(|d| match d {
            Decl::Function(f) if f.name == name => Some(f),
            _ => None,
        })
        .unwrap_or_else(|| panic!("function {name} not found"))
}

#[test]
fn leaf_and_composite_spans_are_tight() {
    let src = "module M where\nf = g (a, b)\n";
    let m = parse(src);
    let f = first_function(&m, "f");
    let body = &f.equations[0].body;
    // `g (a, b)` is an application of g to the tuple.
    assert_eq!(text(src, body.span()), "g (a, b)");
    match body {
        Expr::App { func, args, .. } => {
            assert_eq!(text(src, func.span()), "g");
            assert_eq!(text(src, args[0].span()), "(a, b)");
            match &args[0] {
                Expr::Tuple { items, .. } => {
                    assert_eq!(text(src, items[0].span()), "a");
                    assert_eq!(text(src, items[1].span()), "b");
                }
                other => panic!("expected tuple, got {other:?}"),
            }
        }
        other => panic!("expected app, got {other:?}"),
    }
}

#[test]
fn do_block_span_covers_whole_block() {
    let src = "module M where\nf = do\n  a\n  b\n";
    let m = parse(src);
    let f = first_function(&m, "f");
    let body = &f.equations[0].body;
    assert_eq!(text(src, body.span()), "do\n  a\n  b");
}

#[test]
fn typedef_span_covers_whole_construct() {
    let src = "module M where\ndata Foo = Bar | Baz\n";
    let m = parse(src);
    let td = m
        .decls
        .iter()
        .find(|d| matches!(d, Decl::TypeDef { .. }))
        .expect("typedef");
    let Decl::TypeDef { span, .. } = td else {
        unreachable!()
    };
    assert_eq!(text(src, *span), "data Foo = Bar | Baz");
}

#[test]
fn template_field_and_signatory_spans() {
    let src =
        "module M where\ntemplate T\n  with\n    owner : Party\n  where\n    signatory owner\n";
    let m = parse(src);
    let Decl::Template(t) = m
        .decls
        .iter()
        .find(|d| matches!(d, Decl::Template(_)))
        .unwrap()
    else {
        unreachable!()
    };
    // A single-name field spans the whole `name : Type` so daml-fmt can slice
    // the field type, not just the name.
    assert_eq!(text(src, t.fields[0].span), "owner : Party");
    let sig = t
        .body
        .iter()
        .find(|b| matches!(b, TemplateBodyDecl::Signatory { .. }))
        .unwrap();
    let TemplateBodyDecl::Signatory { span, .. } = sig else {
        unreachable!()
    };
    assert_eq!(text(src, *span), "signatory owner");
}

#[test]
fn type_node_spans_are_tight() {
    let src = r#"module M where
template T
  with
    owner : Party
  where
    signatory owner
    key owner : Party
    maintainer owner
    choice Go : Optional (ContractId T)
      controller owner
      do pure None

interface I where
  method : Numeric 10

f : ContractId T -> Script ()
f cid = pure ()
"#;
    let m = parse(src);
    let Decl::Template(t) = m
        .decls
        .iter()
        .find(|d| matches!(d, Decl::Template(_)))
        .unwrap()
    else {
        unreachable!()
    };
    let field_ty = t.fields[0].ty.as_type().expect("field type");
    assert_eq!(text(src, field_ty.span()), "Party");
    let key_ty = t
        .body
        .iter()
        .find_map(|b| match b {
            TemplateBodyDecl::Key { ty, .. } => ty.as_type(),
            _ => None,
        })
        .expect("key type");
    assert_eq!(text(src, key_ty.span()), "Party");
    let choice_ty = t
        .body
        .iter()
        .find_map(|b| match b {
            TemplateBodyDecl::Choice(c) => c.return_ty.as_type(),
            _ => None,
        })
        .expect("choice return type");
    assert_eq!(text(src, choice_ty.span()), "Optional (ContractId T)");

    let Decl::Interface(i) = m
        .decls
        .iter()
        .find(|d| matches!(d, Decl::Interface(_)))
        .unwrap()
    else {
        unreachable!()
    };
    let method_ty = i.methods[0].ty.as_type().expect("method type");
    assert_eq!(text(src, method_ty.span()), "Numeric 10");

    let f = first_function(&m, "f");
    let fn_ty = f.ty.as_type().expect("function signature type");
    assert_eq!(text(src, fn_ty.span()), "ContractId T -> Script ()");

    match choice_ty {
        Type::App(head, args, _) => {
            assert_eq!(text(src, head.span()), "Optional");
            assert_eq!(text(src, args[0].span()), "(ContractId T)");
        }
        _ => panic!("expected app type for choice return"),
    }
}

#[test]
fn render_from_ast_roundtrips_small_programs() {
    let cases = [
        "module M where\nf = 1\n",
        "module M where\nf x = if x then 1 else 2\n",
        "module M where\nf = do\n  a <- step\n  pure a\n",
        "module M where\n-- a comment\nf = g (a, b) -- trailing\n",
        "module M where\ndata Foo = Bar with x : Int\nf = 1\n",
        "f = 1\ng = 2\n", // no module header
    ];
    for src in cases {
        match render(src) {
            Ok(out) => assert_eq!(out, src, "roundtrip mismatch for {src:?}"),
            Err(e) => panic!("render_from_ast failed for {src:?}: {e}"),
        }
    }
}

#[test]
fn multi_name_field_spans_are_disjoint() {
    let src = "module M where\ntemplate T\n  with\n    a, b : Int\n  where\n    signatory a\n";
    let m = parse(src);
    let Decl::Template(t) = m
        .decls
        .iter()
        .find(|d| matches!(d, Decl::Template(_)))
        .unwrap()
    else {
        unreachable!()
    };
    // Earlier names stay name-only; the last carries the shared type. The two
    // field spans must not overlap.
    assert_eq!(text(src, t.fields[0].span), "a");
    assert_eq!(text(src, t.fields[1].span), "b : Int");
    assert!(t.fields[0].span.end <= t.fields[1].span.start);
}

/// Regression: a type signature separated from its body by an unrelated decl
/// must not produce a function span that straddles that sibling (V2 nesting).
#[test]
fn separated_signature_does_not_straddle_sibling() {
    let src = "module M where\nf : Int\ng = 2\nf = 1\n";
    let m = parse(src);
    // The oracle's containment check would Err on any sibling straddle.
    assert_eq!(render(src).as_deref(), Ok(src));
    let f = first_function(&m, "f");
    let g = first_function(&m, "g");
    // f's span is its equation `f = 1`, which appears *after* sibling g in the
    // source; it must start strictly after g ends (disjoint, not straddling).
    // `contains` is the wrong guard here: g starts before f, so containment is
    // vacuously false and could never catch a straddle.
    assert_eq!(text(src, f.span), "f = 1");
    assert!(
        f.span.start > g.span.end,
        "f's equation must begin after sibling g ends: f={:?} g={:?}",
        f.span,
        g.span
    );
}

fn finance_corpus_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/daml-finance/daml")
}

/// Run the oracle over an entire corpus directory; returns (files, failures).
fn run_finance_corpus_oracle(root: &Path) -> std::io::Result<(usize, Vec<String>)> {
    let mut files = Vec::new();
    collect_daml_files(root, &mut files)?;
    let mut failures = Vec::new();
    for f in &files {
        let src = match std::fs::read_to_string(f) {
            Ok(src) => src,
            Err(e) => {
                failures.push(format!("{}: failed to read corpus file: {e}", f.display()));
                continue;
            }
        };
        if let Err(e) = render(&src) {
            failures.push(format!("{}: {}", f.display(), e));
        }
    }
    Ok((files.len(), failures))
}

/// Run the `render_from_ast` losslessness/nesting oracle over the vendored
/// daml-finance corpus (shared at the workspace root). Skips gracefully when
/// the corpus is absent (e.g. a published crate built outside the workspace),
/// so it runs in CI yet never panics off-machine.
#[test]
fn span_oracle_over_finance_corpus() {
    let root = finance_corpus_root();
    if !root.exists() {
        eprintln!("corpus absent (published crate?), skipping");
        return;
    }
    let (n, failures) =
        run_finance_corpus_oracle(&root).expect("run span oracle over finance corpus");
    assert!(n > 600, "finance corpus incomplete: {n} files");
    if !failures.is_empty() {
        let shown: Vec<_> = failures.iter().take(20).cloned().collect();
        panic!(
            "{} / {} files failed render_from_ast:\n{}",
            failures.len(),
            n,
            shown.join("\n")
        );
    }
}

/// Token-level lossless round-trip oracle (`render_lossless`) over the vendored
/// daml-finance corpus, so daml-parser self-verifies its OWN byte-identical
/// reconstruction guarantee (the one daml-fmt relies on) rather than leaving it
/// only to the downstream daml-lint corpus test. Skips gracefully off-workspace
/// (published crate), but fails loud under CI so a missing/forgotten vendored
/// corpus can never pass green.
#[test]
fn render_lossless_over_finance_corpus() {
    let root = finance_corpus_root();
    if !root.exists() {
        assert!(
            std::env::var_os("CI").is_none(),
            "vendored corpus missing under CI (was it committed?): {}",
            root.display()
        );
        eprintln!("corpus absent (published crate?), skipping");
        return;
    }
    let mut files = Vec::new();
    collect_daml_files(&root, &mut files).expect("collect finance corpus files");
    assert!(
        files.len() > 600,
        "corpus incomplete: {} files",
        files.len()
    );
    let mut checked = 0usize;
    for f in &files {
        let src = std::fs::read_to_string(f)
            .unwrap_or_else(|e| panic!("failed to read corpus file {}: {e}", f.display()));
        let (tokens, trivia, errors) = lex_with_trivia(&src).into_parts();
        if !errors.is_empty() {
            continue; // lex errors drop bytes by design; losslessness is exempt
        }
        checked += 1;
        if let Err(e) = daml_parser::lexer::render_lossless(&src, &tokens, &trivia) {
            panic!("round trip failed for {}: {}", f.display(), e);
        }
    }
    assert!(checked > 600, "too few files round-tripped: {checked}");
}

fn collect_daml_files(dir: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            collect_daml_files(&p, out)?;
        } else if p.extension().is_some_and(|e| e == "daml") {
            out.push(p);
        }
    }
    Ok(())
}
