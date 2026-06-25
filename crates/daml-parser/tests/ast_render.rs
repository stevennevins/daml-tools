//! Integration tests for [`Expr::render`] and [`Pat::render`] on parsed trees.

use daml_parser::ast::*;

const fn pos() -> Pos {
    Pos { line: 1, column: 1 }
}

const fn span(start: usize, end: usize) -> Span {
    Span::new(start, end)
}

#[test]
fn expr_render_keeps_normalized_application_and_projection_shape() {
    let projection = Expr::BinOp {
        op: ".".into(),
        lhs: Box::new(Expr::Var {
            qualifier: None,
            name: "this".into(),
            pos: pos(),
            span: span(0, 4),
        }),
        rhs: Box::new(Expr::Var {
            qualifier: None,
            name: "note".into(),
            pos: pos(),
            span: span(5, 9),
        }),
        pos: pos(),
        span: span(0, 9),
    };

    let expr = Expr::App {
        func: Box::new(Expr::Var {
            qualifier: None,
            name: "length".into(),
            pos: pos(),
            span: span(0, 6),
        }),
        args: vec![projection],
        pos: pos(),
        span: span(0, 16),
    };

    assert_eq!(expr.render(), "length (this.note)");
}

#[test]
fn section_render_depends_on_section_side() {
    let expr_left = Expr::Section {
        op: "+".into(),
        operand: Some(Box::new(Expr::Var {
            qualifier: None,
            name: "x".into(),
            pos: pos(),
            span: span(0, 1),
        })),
        side: SectionSide::Left,
        pos: pos(),
        span: span(0, 4),
    };
    let expr_right = Expr::Section {
        op: "+".into(),
        operand: Some(Box::new(Expr::Lit {
            kind: LitKind::Int,
            text: "1".to_string(),
            pos: pos(),
            span: span(0, 1),
        })),
        side: SectionSide::Right,
        pos: pos(),
        span: span(0, 4),
    };

    assert_eq!(expr_left.render(), "(x +)");
    assert_eq!(expr_right.render(), "(+ 1)");
}

#[test]
fn pat_render_preserves_collection_shape() {
    let pat = Pat::Tuple {
        items: vec![
            Pat::Var {
                name: "owner".into(),
                pos: pos(),
                span: span(1, 6),
            },
            Pat::List {
                items: Vec::new(),
                pos: pos(),
                span: span(8, 10),
            },
        ],
        pos: pos(),
        span: span(0, 11),
    };

    assert_eq!(pat.render(), "(owner, [])");
}
