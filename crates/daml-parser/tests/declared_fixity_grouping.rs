//! Integration tests for module-level declared fixity affecting expression grouping.

use daml_parser::ast::*;
use daml_parser::parse::parse_module;

fn parse(src: &str) -> Module {
    let (module, diagnostics) = parse_module(src).into_parts();
    assert!(
        diagnostics.is_empty(),
        "fixture should parse clean, got {diagnostics:?}"
    );
    module
}

fn body_of<'a>(module: &'a Module, name: &str) -> &'a Expr {
    let function = module
        .decls
        .iter()
        .find_map(|d| match d {
            Decl::Function(f) if f.name == name => Some(f),
            _ => None,
        })
        .unwrap_or_else(|| panic!("missing function {name}"));
    &function.equations[0].body
}

fn as_binop<'a>(e: &'a Expr, op: &str) -> (&'a Expr, &'a Expr) {
    match e {
        Expr::BinOp {
            op: o, lhs, rhs, ..
        } if o.as_str() == op => (lhs.as_ref(), rhs.as_ref()),
        other => panic!("expected BinOp {op}, got {other:?}"),
    }
}

#[test]
fn declared_lower_precedence_groups_before_default_plus() {
    // Unknown `%%` defaults to infixl 9, so `a + b %% c` = `a + (b %% c)`.
    // Declaring `infixl 6 %%` makes `+` bind tighter: `(a + b) %% c`.
    let src = "module M where\nf = a + b %% c\ninfixl 6 %%\n";
    let module = parse(src);
    let body = body_of(&module, "f");
    let (outer_lhs, outer_rhs) = as_binop(body, "%%");
    let (plus_lhs, plus_rhs) = as_binop(outer_lhs, "+");
    assert!(matches!(plus_lhs, Expr::Var { name, .. } if name.as_str() == "a"));
    assert!(matches!(plus_rhs, Expr::Var { name, .. } if name.as_str() == "b"));
    assert!(matches!(outer_rhs, Expr::Var { name, .. } if name.as_str() == "c"));
}

#[test]
fn fixity_declared_after_use_still_applies() {
    // Module-level fixity applies to all expressions regardless of decl order.
    // Without the declaration, `+` binds tighter than default-unknown `%%`.
    let src = "module M where\ng = a + b %% c\ninfixl 6 %%\n";
    let module = parse(src);
    let body = body_of(&module, "g");
    let (outer_lhs, outer_rhs) = as_binop(body, "%%");
    let (plus_lhs, plus_rhs) = as_binop(outer_lhs, "+");
    assert!(matches!(plus_lhs, Expr::Var { name, .. } if name.as_str() == "a"));
    assert!(matches!(plus_rhs, Expr::Var { name, .. } if name.as_str() == "b"));
    assert!(matches!(outer_rhs, Expr::Var { name, .. } if name.as_str() == "c"));
}

#[test]
fn backtick_fixity_target_groups_infix_application() {
    let src = "module M where\nh = a `foo` b `foo` c\ninfixr 5 `foo`\n";
    let module = parse(src);
    let body = body_of(&module, "h");
    let (lhs, rhs) = as_binop(body, "`foo`");
    assert!(matches!(lhs, Expr::Var { name, .. } if name.as_str() == "a"));
    let (mid_lhs, mid_rhs) = as_binop(rhs, "`foo`");
    assert!(matches!(mid_lhs, Expr::Var { name, .. } if name.as_str() == "b"));
    assert!(matches!(mid_rhs, Expr::Var { name, .. } if name.as_str() == "c"));
}

#[test]
fn later_fixity_declaration_overrides_earlier_for_grouping() {
    let src = "module M where\ni = a + b %% c\ninfixl 9 %%\ninfixl 6 %%\n";
    let module = parse(src);
    let body = body_of(&module, "i");
    // Second declaration wins: precedence 6, so `(a + b) %% c`.
    let (outer_lhs, outer_rhs) = as_binop(body, "%%");
    let (plus_lhs, plus_rhs) = as_binop(outer_lhs, "+");
    assert!(matches!(plus_lhs, Expr::Var { name, .. } if name.as_str() == "a"));
    assert!(matches!(plus_rhs, Expr::Var { name, .. } if name.as_str() == "b"));
    assert!(matches!(outer_rhs, Expr::Var { name, .. } if name.as_str() == "c"));
}

#[test]
fn builtin_operators_keep_default_fixity_without_declaration() {
    // `+` (7) binds tighter than default-unknown `%%` (9): `a + b %% c` = `a + (b %% c)`.
    let src = "module M where\nj = a + b %% c\n";
    let module = parse(src);
    let body = body_of(&module, "j");
    let (plus_lhs, plus_rhs) = as_binop(body, "+");
    assert!(matches!(plus_lhs, Expr::Var { name, .. } if name.as_str() == "a"));
    let (pct_lhs, pct_rhs) = as_binop(plus_rhs, "%%");
    assert!(matches!(pct_lhs, Expr::Var { name, .. } if name.as_str() == "b"));
    assert!(matches!(pct_rhs, Expr::Var { name, .. } if name.as_str() == "c"));
}
