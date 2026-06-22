//! Tests for record-projection precedence (`.`).
//!
//! A *tight* dot (`this.note`, no surrounding spaces) is record projection and
//! binds tighter than application: `length this.note` is `length (this.note)`,
//! not `(length this).note`. A *spaced* dot (`f . g`) is function composition
//! and is left to the binary-operator layer. Qualified names (`Map.lookup`) are
//! a single lexer token and never reach the projection path at all.

#![cfg(test)]

use crate::ast::*;
use crate::ast_span::render_from_ast;
use crate::lexer::lex_with_trivia;
use crate::parse::parse_module;

fn parse(src: &str) -> Module {
    let (module, diagnostics) = parse_module(src);
    // Every fixture here is well-formed Daml; assert it so a typo in a fixture
    // fails with a clear cause instead of a baffling structural panic later.
    assert!(
        diagnostics.is_empty(),
        "fixture should parse clean, got {diagnostics:?}"
    );
    module
}

fn text(src: &str, span: Span) -> &str {
    &src[span.start..span.end]
}

/// Body expression of `f = <expr>` in a tiny module.
fn body_of(src: &str) -> Expr {
    let m = parse(src);
    let f = m
        .decls
        .iter()
        .find_map(|d| match d {
            Decl::Function(f) if f.name == "f" => Some(f),
            _ => None,
        })
        .expect("function f");
    f.equations[0].body.clone()
}

/// A projection is `BinOp(".", lhs, rhs)`; return (lhs, rhs).
fn as_proj(e: &Expr) -> (&Expr, &Expr) {
    match e {
        Expr::BinOp { op, lhs, rhs, .. } if op == "." => (lhs, rhs),
        other => panic!("expected projection BinOp, got {other:?}"),
    }
}

#[test]
fn projection_binds_tighter_than_application() {
    // The whole point of Phase 3: `length this.note` must be
    // `length (this.note)`, so the projection is the *argument*, not the call.
    let src = "module M where\nf = length this.note\n";
    let body = body_of(src);
    match &body {
        Expr::App { func, args, .. } => {
            assert_eq!(text(src, func.span()), "length");
            assert_eq!(args.len(), 1, "expected one argument, got {args:?}");
            // The single argument is the projection `this.note`.
            assert_eq!(text(src, args[0].span()), "this.note");
            let (lhs, rhs) = as_proj(&args[0]);
            assert!(matches!(lhs, Expr::Var { name, .. } if name == "this"));
            assert!(matches!(rhs, Expr::Var { name, .. } if name == "note"));
        }
        other => panic!("expected application, got {other:?}"),
    }
}

#[test]
fn bare_projection_is_a_projection() {
    let src = "module M where\nf = this.note\n";
    let body = body_of(src);
    assert_eq!(text(src, body.span()), "this.note");
    let (lhs, rhs) = as_proj(&body);
    assert!(matches!(lhs, Expr::Var { name, .. } if name == "this"));
    assert!(matches!(rhs, Expr::Var { name, .. } if name == "note"));
}

#[test]
fn chained_projection_left_nests() {
    // `a.b.c` is `(a.b).c`.
    let src = "module M where\nf = a.b.c\n";
    let body = body_of(src);
    assert_eq!(text(src, body.span()), "a.b.c");
    let (lhs, rhs) = as_proj(&body);
    assert!(matches!(rhs, Expr::Var { name, .. } if name == "c"));
    assert_eq!(text(src, lhs.span()), "a.b");
    let (a, b) = as_proj(lhs);
    assert!(matches!(a, Expr::Var { name, .. } if name == "a"));
    assert!(matches!(b, Expr::Var { name, .. } if name == "b"));
}

#[test]
fn qualified_name_is_not_a_projection() {
    // `Map.lookup k` — `Map.lookup` is a single qualified token, so the head is
    // a qualified Var, never a projection BinOp.
    let src = "module M where\nf = Map.lookup k\n";
    let body = body_of(src);
    match &body {
        Expr::App { func, args, .. } => {
            match func.as_ref() {
                Expr::Var {
                    qualifier, name, ..
                } => {
                    assert_eq!(qualifier.as_deref(), Some("Map"));
                    assert_eq!(name, "lookup");
                }
                other => panic!("expected qualified Var head, got {other:?}"),
            }
            assert_eq!(args.len(), 1);
            assert!(matches!(&args[0], Expr::Var { name, .. } if name == "k"));
        }
        other => panic!("expected application, got {other:?}"),
    }
}

#[test]
fn spaced_dot_stays_composition_not_projection() {
    // `f . g` with spaces is composition: a BinOp whose sides are the two
    // *bare* names, NOT a projection folded into an application argument.
    let src = "module M where\nf = compose g h\ncompose g h = g . h\n";
    let m = parse(src);
    let compose = m
        .decls
        .iter()
        .find_map(|d| match d {
            Decl::Function(f) if f.name == "compose" => Some(f),
            _ => None,
        })
        .expect("compose");
    let body = &compose.equations[0].body;
    match body {
        Expr::BinOp { op, lhs, rhs, .. } if op == "." => {
            // Tight projection would have made the dot abut its neighbours; a
            // spaced dot keeps `g` and `h` as the operands. Pin each operand's
            // byte-extent to exactly the bare name: that is the evidence the dot
            // did NOT fold a neighbour into a projection argument.
            assert!(matches!(lhs.as_ref(), Expr::Var { name, .. } if name == "g"));
            assert!(matches!(rhs.as_ref(), Expr::Var { name, .. } if name == "h"));
            assert_eq!(text(src, lhs.span()), "g");
            assert_eq!(text(src, rhs.span()), "h");
        }
        other => panic!("expected composition BinOp, got {other:?}"),
    }
}

#[test]
fn newline_separated_dot_stays_composition() {
    // A dot reached across a line break is NOT tight: the layout token(s) and
    // whitespace between `g` and `.` mean the left-adjacency check fails, so it
    // stays composition. Guards against the tight-projection fold mistaking a
    // dedented `.` (e.g. a multi-line composition pipeline) for a projection.
    let src = "module M where\ncompose g h = g\n  . h\n";
    let m = parse(src);
    let compose = m
        .decls
        .iter()
        .find_map(|d| match d {
            Decl::Function(f) if f.name == "compose" => Some(f),
            _ => None,
        })
        .expect("compose");
    let body = &compose.equations[0].body;
    match body {
        Expr::BinOp { op, lhs, rhs, .. } if op == "." => {
            assert!(matches!(lhs.as_ref(), Expr::Var { name, .. } if name == "g"));
            assert!(matches!(rhs.as_ref(), Expr::Var { name, .. } if name == "h"));
            // The operands stay the bare names across the line break — the
            // dedented `.` was not mistaken for a tight projection.
            assert_eq!(text(src, lhs.span()), "g");
            assert_eq!(text(src, rhs.span()), "h");
        }
        other => panic!("expected composition BinOp across newline, got {other:?}"),
    }
    // And still byte-lossless.
    let (_, trivia, _) = lex_with_trivia(src);
    assert_eq!(render_from_ast(src, &m, &trivia).as_deref(), Ok(src));
}

#[test]
fn projection_inside_assertion_guard() {
    // Projection must work where detectors look: inside a guard/comparison.
    // `x.amount > 0.0` — the left side of `>` is the projection.
    let src = "module M where\nf x = assertMsg \"pos\" (x.amount > 0.0)\n";
    let body = body_of(src);
    // body is `assertMsg "pos" (x.amount > 0.0)` — dig to the parenthesised
    // comparison and assert its lhs is the projection.
    let cmp = find_binop(&body, ">").expect("comparison > present");
    let (lhs, _rhs) = match cmp {
        Expr::BinOp { lhs, rhs, .. } => (lhs.as_ref(), rhs.as_ref()),
        _ => unreachable!(),
    };
    assert_eq!(text(src, lhs.span()), "x.amount");
    let (base, field) = as_proj(lhs);
    assert!(matches!(base, Expr::Var { name, .. } if name == "x"));
    assert!(matches!(field, Expr::Var { name, .. } if name == "amount"));
}

/// First `BinOp` with operator `op` found anywhere in `e` (pre-order).
fn find_binop<'a>(e: &'a Expr, op: &str) -> Option<&'a Expr> {
    if let Expr::BinOp { op: o, .. } = e {
        if o == op {
            return Some(e);
        }
    }
    for child in children(e) {
        if let Some(found) = find_binop(child, op) {
            return Some(found);
        }
    }
    None
}

fn children(e: &Expr) -> Vec<&Expr> {
    match e {
        Expr::App { func, args, .. } => {
            let mut v = vec![func.as_ref()];
            v.extend(args.iter());
            v
        }
        Expr::BinOp { lhs, rhs, .. } => vec![lhs.as_ref(), rhs.as_ref()],
        Expr::Neg { expr, .. } => vec![expr.as_ref()],
        _ => Vec::new(),
    }
}

#[test]
fn projection_span_is_tight() {
    let src = "module M where\nf = length this.note\n";
    let body = body_of(src);
    let arg = match &body {
        Expr::App { args, .. } => args[0].clone(),
        other => panic!("expected app, got {other:?}"),
    };
    // The projection span covers exactly `this.note`, nothing more.
    assert_eq!(text(src, arg.span()), "this.note");
}

#[test]
fn projection_roundtrips_through_oracle() {
    // The lossless span oracle must still reconstruct byte-for-byte: the
    // precedence change only re-nests the tree, it must not move any span.
    let cases = [
        "module M where\nf = length this.note\n",
        "module M where\nf = a.b.c\n",
        "module M where\nf = Map.lookup k\n",
        "module M where\nf x = assertMsg \"pos\" (x.amount > 0.0)\n",
        "module M where\nf = (g x).note\n",
    ];
    for src in cases {
        let (_, trivia, _) = lex_with_trivia(src);
        match render_from_ast(src, &parse(src), &trivia) {
            Ok(out) => assert_eq!(out, src, "roundtrip mismatch for {src:?}"),
            Err(e) => panic!("oracle failed for {src:?}: {e}"),
        }
    }
}
