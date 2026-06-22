//! Lowering: typed AST (from the `daml-parser` crate) → rule-facing IR
//! (src/ir.rs).
//!
//! This replaces the old line-based keyword shim. The IR shapes are the
//! stable contract with rule scripts; structured `Expr` and `TypeNode`
//! payloads carry the actual parse tree.

use crate::ir::*;
use daml_syntax::{
    ast::{self, Consuming, Decl, DoStmt, TemplateBodyDecl},
    SourceFile,
};
use std::path::Path;

#[cfg(test)]
pub(crate) fn parse_daml(source: &str, file: &Path) -> DamlModule {
    parse_daml_with_diagnostics(source, file).0
}

/// A parse diagnostic for the caller to report.
///
/// `end_column` is present when the offending span sits on a single line (most
/// tokens); `category` is the parser's recovery classification
/// (`skipped-declaration`, `malformed`, `unsupported-syntax`, `recursion-limit`,
/// `lexical-error`).
#[derive(Debug)]
pub struct Diagnostic {
    pub line: usize,
    pub column: usize,
    pub end_column: Option<usize>,
    pub message: String,
    pub category: &'static str,
}

pub fn parse_daml_with_diagnostics(source: &str, file: &Path) -> (DamlModule, Vec<Diagnostic>) {
    let source_file = SourceFile::parse(source);
    let module = source_file.module();
    let imports = module
        .imports
        .iter()
        .map(|i| Import {
            module_name: i.module_name.clone(),
            qualified: i.qualified,
            alias: i.alias.clone(),
            span: span_at(file, i.pos),
        })
        .collect();

    let mut templates = Vec::new();
    let mut interfaces = Vec::new();
    let mut functions = Vec::new();

    for decl in &module.decls {
        match decl {
            Decl::Template(t) => templates.push(lower_template(t, file, &source_file)),
            Decl::Interface(i) => interfaces.push(lower_interface(i, file, &source_file)),
            Decl::Function(f) => {
                if f.equations.is_empty() {
                    continue; // type signature without a body
                }
                functions.push(lower_function(f, file, &source_file));
            }
            _ => {}
        }
    }

    let ir = DamlModule {
        ir_version: 3,
        name: module.name.clone(),
        file: file.to_path_buf(),
        source: source.to_string(),
        imports,
        templates,
        interfaces,
        functions,
    };
    let diags = source_file
        .diagnostics()
        .iter()
        .map(|d| Diagnostic {
            line: d.line,
            column: d.column,
            end_column: d.end_column,
            message: d.message.clone(),
            category: d.category,
        })
        .collect();
    (ir, diags)
}

fn span_at(file: &Path, pos: ast::Pos) -> Span {
    Span {
        file: file.to_path_buf(),
        line: pos.line,
        column: pos.column,
    }
}

const fn src_pos(pos: ast::Pos) -> SrcPos {
    SrcPos {
        line: pos.line,
        column: pos.column,
    }
}

// ----- expressions -------------------------------------------------------

fn lower_expr(e: &ast::Expr) -> Expr {
    let span = src_pos(e.pos());
    match e {
        ast::Expr::Var {
            qualifier, name, ..
        } => Expr::Var {
            name: name.clone(),
            qualifier: qualifier.clone(),
            span,
        },
        ast::Expr::Con {
            qualifier, name, ..
        } => Expr::Con {
            name: name.clone(),
            qualifier: qualifier.clone(),
            span,
        },
        ast::Expr::Lit { kind, text, .. } => Expr::Lit {
            kind: match kind {
                ast::LitKind::Int => "Int",
                ast::LitKind::Decimal => "Decimal",
                ast::LitKind::Text => "Text",
                ast::LitKind::Char => "Char",
            }
            .to_string(),
            value: text.clone(),
            span,
        },
        ast::Expr::App { func, args, .. } => Expr::App {
            func: Box::new(lower_expr(func)),
            args: args.iter().map(lower_expr).collect(),
            span,
        },
        ast::Expr::BinOp { op, lhs, rhs, .. } => Expr::BinOp {
            op: op.clone(),
            lhs: Box::new(lower_expr(lhs)),
            rhs: Box::new(lower_expr(rhs)),
            span,
        },
        ast::Expr::Neg { expr, .. } => Expr::Neg {
            expr: Box::new(lower_expr(expr)),
            span,
        },
        ast::Expr::Lambda { params, body, .. } => Expr::Lambda {
            params: params.iter().map(|p| p.render()).collect(),
            body: Box::new(lower_expr(body)),
            span,
        },
        ast::Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => Expr::If {
            cond: Box::new(lower_expr(cond)),
            then_branch: Box::new(lower_expr(then_branch)),
            else_branch: Box::new(lower_expr(else_branch)),
            span,
        },
        ast::Expr::Case {
            scrutinee, alts, ..
        } => Expr::Case {
            scrutinee: Box::new(lower_expr(scrutinee)),
            alts: alts
                .iter()
                .map(|a| CaseAlt {
                    pattern: a.pat.render(),
                    body: lower_expr(&a.body),
                })
                .collect(),
            span,
        },
        ast::Expr::Do { stmts, .. } => Expr::DoBlock {
            statements: lower_do(stmts),
            span,
        },
        ast::Expr::LetIn { bindings, body, .. } => Expr::LetIn {
            bindings: bindings
                .iter()
                .map(|b| LetBinding {
                    name: binding_name(b),
                    value: lower_expr(&b.expr),
                })
                .collect(),
            body: Box::new(lower_expr(body)),
            span,
        },
        ast::Expr::Record { base, fields, .. } => Expr::Record {
            base: Box::new(lower_expr(base)),
            fields: fields
                .iter()
                .map(|f| RecordField {
                    name: f.name.clone(),
                    value: f.value.as_ref().map(lower_expr),
                })
                .collect(),
            span,
        },
        ast::Expr::Tuple { items, .. } => Expr::Tuple {
            items: items.iter().map(lower_expr).collect(),
            span,
        },
        ast::Expr::List { items, .. } => Expr::List {
            items: items.iter().map(lower_expr).collect(),
            span,
        },
        // No structured rule-facing encoding (yet): sections, try-in-
        // expression-position, recovered parse errors.
        ast::Expr::Try { .. } | ast::Expr::Section { .. } | ast::Expr::Error { .. } => {
            Expr::Unknown {
                raw: e.render(),
                span,
            }
        }
    }
}

fn binding_name(b: &ast::Binding) -> String {
    let mut name = b.pat.render();
    for p in &b.params {
        name.push(' ');
        name.push_str(&p.render());
    }
    name
}

// ----- declarations ------------------------------------------------------

fn lower_template(t: &ast::TemplateDecl, file: &Path, source_file: &SourceFile) -> Template {
    let fields = t
        .fields
        .iter()
        .map(|f| Field {
            name: f.name.clone(),
            type_: f
                .ty
                .as_ref()
                .map(|ty| TypeNode::from_type(ty, file, source_file)),
            span: span_at(file, f.pos),
        })
        .collect();

    let mut signatory_exprs = Vec::new();
    let mut observer_exprs = Vec::new();
    let mut ensure_clause = None;
    let mut key_expr = None;
    let mut key_type = None;
    let mut maintainer_exprs = Vec::new();
    let mut choices = Vec::new();
    let mut interface_instances = Vec::new();

    for item in &t.body {
        match item {
            TemplateBodyDecl::Signatory { parties, .. } => {
                signatory_exprs.extend(parties.iter().map(lower_expr));
            }
            TemplateBodyDecl::Observer { parties, .. } => {
                observer_exprs.extend(parties.iter().map(lower_expr));
            }
            TemplateBodyDecl::Ensure { expr, pos, .. } => {
                ensure_clause = Some(EnsureClause {
                    expr: lower_expr(expr),
                    span: span_at(file, *pos),
                });
            }
            TemplateBodyDecl::Key { expr, ty, .. } => {
                key_expr = Some(lower_expr(expr));
                key_type = ty
                    .as_ref()
                    .map(|ty| TypeNode::from_type(ty, file, source_file));
            }
            TemplateBodyDecl::Maintainer { expr, .. } => {
                maintainer_exprs.push(lower_expr(expr));
            }
            TemplateBodyDecl::Choice(c) => choices.push(lower_choice(c, file, source_file)),
            TemplateBodyDecl::InterfaceInstance(ii) => {
                interface_instances.push(InterfaceInstance {
                    interface_name: ii.interface_name.clone(),
                    methods: ii.methods.iter().map(binding_name).collect(),
                    span: span_at(file, ii.pos),
                });
            }
            TemplateBodyDecl::Other { .. } => {}
        }
    }

    Template {
        name: t.name.clone(),
        fields,
        signatory_exprs,
        observer_exprs,
        ensure_clause,
        key_expr,
        key_type,
        maintainer_exprs,
        choices,
        interface_instances,
        span: span_at(file, t.pos),
    }
}

fn lower_interface(i: &ast::InterfaceDecl, file: &Path, source_file: &SourceFile) -> Interface {
    Interface {
        name: i.name.clone(),
        requires: i.requires.clone(),
        viewtype: i.viewtype.clone(),
        methods: i
            .methods
            .iter()
            .map(|m| InterfaceMethod {
                name: m.name.clone(),
                type_: m
                    .ty
                    .as_ref()
                    .map(|ty| TypeNode::from_type(ty, file, source_file)),
                span: span_at(file, m.pos),
            })
            .collect(),
        choices: i
            .choices
            .iter()
            .map(|c| lower_choice(c, file, source_file))
            .collect(),
        span: span_at(file, i.pos),
    }
}

fn lower_choice(c: &ast::ChoiceDecl, file: &Path, source_file: &SourceFile) -> Choice {
    let parameters = c
        .params
        .iter()
        .map(|f| Field {
            name: f.name.clone(),
            type_: f
                .ty
                .as_ref()
                .map(|ty| TypeNode::from_type(ty, file, source_file)),
            span: span_at(file, f.pos),
        })
        .collect();

    let body = c.body.as_ref().map_or_else(Vec::new, statements_of_expr);

    Choice {
        name: c.name.clone(),
        // pre/postconsuming choices archive the contract just like the default
        // consuming form; only NonConsuming leaves it live. The boolean means
        // "archives the contract".
        consuming: c.consuming != Consuming::NonConsuming,
        controller_exprs: c.controllers.iter().map(lower_expr).collect(),
        observer_exprs: c.observers.iter().map(lower_expr).collect(),
        parameters,
        return_type: c
            .return_ty
            .as_ref()
            .map(|ty| TypeNode::from_type(ty, file, source_file)),
        body,
        span: span_at(file, c.pos),
    }
}

fn lower_function(f: &ast::FunctionDecl, file: &Path, source_file: &SourceFile) -> Function {
    let mut body = Vec::new();
    for eq in &f.equations {
        if eq.guards.is_empty() {
            body.extend(statements_of_expr(&eq.body));
        } else {
            for (_, guard_body) in &eq.guards {
                body.extend(statements_of_expr(guard_body));
            }
        }
        // `where` helpers can perform ledger actions when invoked; surface
        // their actions like the line shim did.
        for b in &eq.where_bindings {
            let mut acts = Vec::new();
            collect_actions(&b.expr, None, &mut acts);
            body.extend(acts);
        }
    }

    Function {
        name: f.name.clone(),
        type_signature: f
            .ty
            .as_ref()
            .map(|ty| TypeNode::from_type(ty, file, source_file)),
        body,
        span: span_at(file, f.pos),
    }
}

// ----- statements --------------------------------------------------------

/// Statements of a choice/function body expression: a do block yields its
/// statements; any other expression is a single statement.
fn statements_of_expr(expr: &ast::Expr) -> Vec<Statement> {
    match expr {
        ast::Expr::Do { stmts, .. } => lower_do(stmts),
        other => {
            let mut acts = Vec::new();
            if collect_actions(other, None, &mut acts) {
                acts
            } else {
                vec![other_statement(other, None)]
            }
        }
    }
}

fn other_statement(expr: &ast::Expr, binder: Option<&ast::Pat>) -> Statement {
    let raw = binder.map_or_else(
        || expr.render(),
        |p| format!("{} <- {}", p.render(), expr.render()),
    );
    Statement::Other {
        raw,
        expr: lower_expr(expr),
        binder: binder.map(|p| p.render()),
        span: src_pos(expr.pos()),
    }
}

fn lower_do(stmts: &[DoStmt]) -> Vec<Statement> {
    let mut out = Vec::new();
    let mut helpers: std::collections::HashMap<String, Helper<'_>> =
        std::collections::HashMap::new();
    for stmt in stmts {
        match stmt {
            DoStmt::Let { bindings, .. } => {
                for b in bindings {
                    let name = binding_name(b);
                    out.push(Statement::Let {
                        name: name.clone(),
                        value: lower_expr(&b.expr),
                        span: src_pos(b.pos),
                    });
                    // A let-bound local helper (`let go x = archive x`) only
                    // DEFINES its actions; they execute at the CALL site (or
                    // never), not at the definition. Record it under its bare
                    // name (without the parameters that `binding_name` appends)
                    // for expansion there, and do NOT surface its archive at the
                    // `let` line.
                    if let Some(params) = formal_param_names(b) {
                        helpers.insert(
                            b.pat.render(),
                            Helper {
                                params,
                                body: &b.expr,
                            },
                        );
                    }
                }
            }
            DoStmt::Bind { pat, expr, .. } => {
                let mut acts = Vec::new();
                if expand_helper_call(expr, Some(pat), &helpers, &mut out) {
                    // expanded in place at the call site
                } else if collect_actions(expr, Some(pat), &mut acts) {
                    out.extend(acts);
                } else {
                    out.push(other_statement(expr, Some(pat)));
                }
            }
            DoStmt::Expr { expr, .. } => {
                let mut acts = Vec::new();
                if expand_helper_call(expr, None, &helpers, &mut out) {
                    // expanded in place at the call site
                } else if collect_actions(expr, None, &mut acts) {
                    out.extend(acts);
                } else {
                    out.push(other_statement(expr, None));
                }
            }
        }
    }
    out
}

/// A let-bound local helper (`let f x = archive x`): its formal parameter names
/// and body. Its ledger actions run at each CALL site (or never), so they are
/// expanded where it is invoked, not recorded at the definition.
struct Helper<'a> {
    params: Vec<String>,
    body: &'a ast::Expr,
}

/// The simple variable names of a function binding's formal parameters
/// (`let f x y = ...` → `["x", "y"]`). None when the binding takes no parameters
/// (a plain value, not a helper) or any parameter is a non-trivial pattern we
/// cannot substitute by name.
fn formal_param_names(b: &ast::Binding) -> Option<Vec<String>> {
    if b.params.is_empty() {
        return None;
    }
    b.params
        .iter()
        .map(|p| match p {
            ast::Pat::Var { name, .. } => Some(name.clone()),
            _ => None,
        })
        .collect()
}

/// If `expr` invokes a known local helper with a matching argument count, expand
/// the helper body with the actual arguments substituted for its formal
/// parameters, re-point every node to the CALL site, and lower the result there
/// (so the archive/effect lands on the invocation line with the real argument).
/// Returns true when an action was expanded. A partial application (arity
/// mismatch) is left for normal lowering.
fn expand_helper_call(
    expr: &ast::Expr,
    binder: Option<&ast::Pat>,
    helpers: &std::collections::HashMap<String, Helper<'_>>,
    out: &mut Vec<Statement>,
) -> bool {
    let ast::Expr::App { func, args, .. } = expr else {
        return false;
    };
    let ast::Expr::Var {
        qualifier: None,
        name,
        ..
    } = func.as_ref()
    else {
        return false;
    };
    let Some(helper) = helpers.get(name) else {
        return false;
    };
    if helper.params.len() != args.len() {
        return false;
    }
    let subst: std::collections::HashMap<&str, &ast::Expr> = helper
        .params
        .iter()
        .map(|p| p.as_str())
        .zip(args.iter())
        .collect();
    let call_pos = expr.pos();
    let expanded = subst_expr(helper.body, &subst, call_pos);
    collect_actions(&expanded, binder, out)
}

/// Substitute actual arguments for a helper's formal parameters throughout
/// `expr`, re-pointing every rebuilt node to `call_pos` (the invocation site) so
/// a finding cites the call, not the definition. A bare `Var` naming a formal is
/// replaced by the actual argument; everything else is rebuilt structurally, and
/// any node we do not rebuild is repointed as-is. The realistic helper is a
/// single ledger action over its arguments.
fn subst_expr(
    expr: &ast::Expr,
    subst: &std::collections::HashMap<&str, &ast::Expr>,
    call_pos: ast::Pos,
) -> ast::Expr {
    use ast::Expr as E;
    match expr {
        E::Var {
            qualifier: None,
            name,
            span,
            ..
        } => subst.get(name.as_str()).map_or_else(
            || E::Var {
                qualifier: None,
                name: name.clone(),
                pos: call_pos,
                span: *span,
            },
            |arg| repoint(arg, call_pos),
        ),
        E::App {
            func, args, span, ..
        } => E::App {
            func: Box::new(subst_expr(func, subst, call_pos)),
            args: args
                .iter()
                .map(|a| subst_expr(a, subst, call_pos))
                .collect(),
            pos: call_pos,
            span: *span,
        },
        E::BinOp {
            op, lhs, rhs, span, ..
        } => E::BinOp {
            op: op.clone(),
            lhs: Box::new(subst_expr(lhs, subst, call_pos)),
            rhs: Box::new(subst_expr(rhs, subst, call_pos)),
            pos: call_pos,
            span: *span,
        },
        E::Neg { expr, span, .. } => E::Neg {
            expr: Box::new(subst_expr(expr, subst, call_pos)),
            pos: call_pos,
            span: *span,
        },
        E::Record {
            base, fields, span, ..
        } => E::Record {
            base: Box::new(subst_expr(base, subst, call_pos)),
            fields: fields
                .iter()
                .map(|f| ast::FieldAssign {
                    value: f.value.as_ref().map(|v| subst_expr(v, subst, call_pos)),
                    ..f.clone()
                })
                .collect(),
            pos: call_pos,
            span: *span,
        },
        E::Tuple { items, span, .. } => E::Tuple {
            items: items
                .iter()
                .map(|i| subst_expr(i, subst, call_pos))
                .collect(),
            pos: call_pos,
            span: *span,
        },
        E::List { items, span, .. } => E::List {
            items: items
                .iter()
                .map(|i| subst_expr(i, subst, call_pos))
                .collect(),
            pos: call_pos,
            span: *span,
        },
        // A helper body that is itself a do-block / conditional / lambda is
        // beyond this targeted substitution; repoint what we can and lower it
        // as-is.
        other => repoint(other, call_pos),
    }
}

/// Shallowly re-point an expression's top-level position to `call_pos` so a
/// substituted argument or unhandled body reports the invocation line. Nested
/// positions are left as-is.
fn repoint(expr: &ast::Expr, call_pos: ast::Pos) -> ast::Expr {
    use ast::Expr as E;
    let mut e = expr.clone();
    match &mut e {
        E::Var { pos, .. }
        | E::Con { pos, .. }
        | E::Lit { pos, .. }
        | E::App { pos, .. }
        | E::BinOp { pos, .. }
        | E::Neg { pos, .. }
        | E::Lambda { pos, .. }
        | E::If { pos, .. }
        | E::Case { pos, .. }
        | E::Do { pos, .. }
        | E::LetIn { pos, .. }
        | E::Record { pos, .. }
        | E::Tuple { pos, .. }
        | E::List { pos, .. }
        | E::Try { pos, .. }
        | E::Section { pos, .. }
        | E::Error { pos, .. } => *pos = call_pos,
    }
    e
}

/// Walk an expression collecting ledger-action statements (create,
/// exercise, fetch, archive, assert, try/catch). Returns true if anything
/// was collected. Only unqualified applications count: `Lifecycle.exercise`
/// is a user function, not the ledger action.
///
/// External callers are all statement-position, where an `if`/`case` is its own
/// set of mutually-exclusive scopes and an `assert` is an unconditional guard.
fn collect_actions(expr: &ast::Expr, binder: Option<&ast::Pat>, out: &mut Vec<Statement>) -> bool {
    collect_actions_inner(expr, binder, out, true, false)
}

/// `stmt_pos` is true when `expr` is in statement position (a do-statement or an
/// `if`/`case` arm). A statement-position `if`/`case` is lowered to a single
/// `Statement::Branch` whose arms are independent scopes; an `if`/`case` reached
/// by descending into a sub-expression (an application argument, an operand, a
/// lambda body) is left in the `Expr` tree for the enclosing statement to carry,
/// so a value-level guard like `if denom /= 0 then x / denom` stays analyzable.
///
/// `conditional` is true once we are inside a branch / iteration / lambda
/// argument that runs only on some paths or zero-or-more times. An `assert`
/// lifted from there is not a guarantee, so it is recorded as a plain
/// `Statement::Other` (scannable, but never a guard) rather than a
/// `Statement::Assert`.
fn collect_actions_inner(
    expr: &ast::Expr,
    binder: Option<&ast::Pat>,
    out: &mut Vec<Statement>,
    stmt_pos: bool,
    conditional: bool,
) -> bool {
    let before = out.len();
    match expr {
        ast::Expr::Do { stmts, .. } => {
            out.extend(lower_do(stmts));
        }
        ast::Expr::Try { body, handlers, .. } => {
            let try_body = statements_of_expr(body);
            let mut catch_body = Vec::new();
            for h in handlers {
                catch_body.extend(statements_of_expr(&h.body));
            }
            out.push(Statement::TryCatch {
                try_body,
                catch_body,
                span: src_pos(expr.pos()),
            });
        }
        ast::Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            if stmt_pos {
                // The condition rides along as the Branch scrutinee so a defensive
                // guard (`if amount <= 0 then abort`) stays analyzable; the arms
                // (then, else) carry no pattern.
                push_branch(
                    Some(cond),
                    &[then_branch, else_branch],
                    &[None, None],
                    expr,
                    out,
                );
            } else {
                // Expression-position `if`: surface only ledger actions inside,
                // leaving the `Expr::If` (and its condition guard) for the
                // enclosing statement.
                collect_actions_inner(then_branch, None, out, false, conditional);
                collect_actions_inner(else_branch, None, out, false, conditional);
            }
        }
        ast::Expr::Case {
            scrutinee, alts, ..
        } => {
            if stmt_pos {
                let bodies: Vec<&ast::Expr> = alts.iter().map(|a| &a.body).collect();
                let patterns: Vec<Option<String>> =
                    alts.iter().map(|a| Some(a.pat.render())).collect();
                push_branch(Some(scrutinee), &bodies, &patterns, expr, out);
            } else {
                for a in alts {
                    collect_actions_inner(&a.body, None, out, false, conditional);
                }
            }
        }
        ast::Expr::LetIn { body, .. } => {
            collect_actions_inner(body, None, out, stmt_pos, conditional);
        }
        ast::Expr::Lambda { body, .. } => {
            // A lambda body runs in a deferred / zero-or-more context, so an
            // assert inside it is not an unconditional guarantee.
            collect_actions_inner(body, None, out, false, true);
        }
        ast::Expr::Neg { expr, .. } => {
            collect_actions_inner(expr, None, out, false, conditional);
        }
        ast::Expr::BinOp {
            op,
            lhs,
            rhs,
            pos,
            span,
        } => {
            // `create $ Foo with ...` — `$` is application.
            if op == "$" {
                let as_app = ast::Expr::App {
                    func: lhs.clone(),
                    args: vec![(**rhs).clone()],
                    pos: *pos,
                    span: *span,
                };
                if classify_app(&as_app, binder, out, conditional) {
                    return out.len() > before;
                }
            }
            collect_actions_inner(lhs, None, out, false, conditional);
            collect_actions_inner(rhs, None, out, false, conditional);
        }
        ast::Expr::App { args, .. } => {
            if !classify_app(expr, binder, out, conditional) {
                // `when c act` / `forA_ xs f` run their action argument only on
                // some paths or zero-or-more times: an assert lifted from inside
                // is conditional, not a guard.
                let arg_conditional =
                    conditional || is_conditional_combinator(expr.application_head());
                for a in args {
                    collect_actions_inner(a, None, out, false, arg_conditional);
                }
            }
        }
        ast::Expr::Tuple { items, .. } | ast::Expr::List { items, .. } => {
            for i in items {
                collect_actions_inner(i, None, out, false, conditional);
            }
        }
        _ => {}
    }
    out.len() > before
}

/// Lower each branch of a statement-position `if`/`case` into its own scope and
/// push one `Statement::Branch`. `scrutinee` is the case scrutinee (None for
/// `if`); `patterns[i]` is arm i's source pattern (None for the `if` then/else
/// arms). Always pushes the Branch: the case shape (scrutinee + patterns) must
/// survive even when the arms perform no ledger action, so a detector can read
/// it structurally.
fn push_branch(
    scrutinee: Option<&ast::Expr>,
    branches: &[&ast::Expr],
    patterns: &[Option<String>],
    whole: &ast::Expr,
    out: &mut Vec<Statement>,
) {
    let arms = branches
        .iter()
        .enumerate()
        .map(|(i, b)| BranchArm {
            pattern: patterns.get(i).cloned().flatten(),
            body: statements_of_expr(b),
        })
        .collect();
    out.push(Statement::Branch {
        scrutinee: scrutinee.map(lower_expr),
        arms,
        span: src_pos(whole.pos()),
    });
}

/// True if `head` is a combinator that runs its action argument conditionally or
/// zero-or-more times (`when`, `unless`, `forA_`, `mapA_`, …), so an `assert`
/// lifted from that argument is not an unconditional guard. Matched on the
/// trailing identifier at any qualifier.
fn is_conditional_combinator(head: &ast::Expr) -> bool {
    matches!(
        head,
        ast::Expr::Var { name, .. } if matches!(
            name.as_str(),
            "when"
                | "unless"
                | "forA_"
                | "forA"
                | "forM_"
                | "forM"
                | "mapA_"
                | "mapA"
                | "mapM_"
                | "mapM"
        )
    )
}

/// If `expr` is an application of a ledger-action head, push the matching
/// statement(s) and return true.
fn classify_app(
    expr: &ast::Expr,
    binder: Option<&ast::Pat>,
    out: &mut Vec<Statement>,
    conditional: bool,
) -> bool {
    let args = expr.application_args();
    if args.is_empty() {
        return false;
    }
    let (head_name, qualified) = match expr.application_head() {
        ast::Expr::Var {
            qualifier, name, ..
        } => (name.as_str(), qualifier.is_some()),
        _ => return false,
    };
    // The ledger actions are the UNQUALIFIED primitives (`Lifecycle.exercise` is
    // a user function, not the ledger action). The one exception is the assert
    // guard, which is routinely written qualified (`DA.Assert.assertMsg`).
    if qualified && head_name != "assert" && head_name != "assertMsg" {
        return false;
    }
    let arg_expr = |i: usize| {
        args.get(i)
            .map(lower_expr)
            .unwrap_or_else(|| Expr::Unknown {
                raw: String::new(),
                span: src_pos(expr.pos()),
            })
    };
    let binder_name = binder.map(|p| p.render());
    let span = src_pos(expr.pos());
    match head_name {
        "create" | "createCmd" => {
            out.push(Statement::Create {
                template_name: template_name_of(args.first()),
                argument: arg_expr(0),
                binder: binder_name,
                span,
            });
            true
        }
        "exercise" | "exerciseByKey" | "exerciseCmd" | "exerciseByKeyCmd" => {
            out.push(Statement::Exercise {
                choice_name: choice_name_of(args.get(1)),
                cid: arg_expr(0),
                argument: choice_argument_of(args.get(1)),
                binder: binder_name,
                span,
            });
            true
        }
        "createAndExerciseCmd" => {
            out.push(Statement::Create {
                template_name: template_name_of(args.first()),
                argument: arg_expr(0),
                binder: binder_name.clone(),
                span,
            });
            out.push(Statement::Exercise {
                choice_name: choice_name_of(args.get(1)),
                cid: arg_expr(0),
                argument: choice_argument_of(args.get(1)),
                binder: binder_name,
                span,
            });
            true
        }
        "fetch" => {
            out.push(Statement::Fetch {
                cid: arg_expr(0),
                binder: binder_name,
                span,
            });
            true
        }
        "fetchAndArchive" => {
            out.push(Statement::Archive {
                cid: arg_expr(0),
                span,
            });
            out.push(Statement::Fetch {
                cid: arg_expr(0),
                binder: binder_name,
                span,
            });
            true
        }
        "archive" => {
            out.push(Statement::Archive {
                cid: arg_expr(0),
                span,
            });
            true
        }
        "assert" | "assertMsg" => {
            if conditional {
                // An assert that runs only on some paths (inside an `if`/`case`
                // arm, a `when`, or a `forA_` lambda) guarantees nothing. Keep it
                // scannable but deny it guard status: record it as a plain Other.
                out.push(Statement::Other {
                    raw: expr.render(),
                    expr: lower_expr(expr),
                    binder: None,
                    span,
                });
            } else {
                // The condition is the assert's argument (after the message for
                // assertMsg), not the whole call.
                let cond_idx = if head_name == "assertMsg" { 1 } else { 0 };
                let condition_expr = args
                    .get(cond_idx)
                    .map(lower_expr)
                    .unwrap_or_else(|| lower_expr(expr));
                out.push(Statement::Assert {
                    condition_expr,
                    span,
                });
            }
            true
        }
        _ => false,
    }
}

fn template_name_of(arg: Option<&ast::Expr>) -> String {
    match arg {
        Some(ast::Expr::Record { base, .. }) => template_name_of(Some(base)),
        Some(ast::Expr::Con {
            qualifier, name, ..
        }) => qualifier
            .as_ref()
            .map_or_else(|| name.clone(), |q| format!("{}.{}", q, name)),
        Some(ast::Expr::Var { name, .. }) if name == "this" => "this".to_string(),
        _ => String::new(),
    }
}

fn choice_name_of(arg: Option<&ast::Expr>) -> String {
    match arg {
        Some(ast::Expr::Record { base, .. }) => choice_name_of(Some(base)),
        Some(ast::Expr::Con {
            qualifier, name, ..
        }) => qualifier
            .as_ref()
            .map_or_else(|| name.clone(), |q| format!("{}.{}", q, name)),
        Some(ast::Expr::App { func, .. }) => choice_name_of(Some(func)),
        _ => String::new(),
    }
}

fn choice_argument_of(arg: Option<&ast::Expr>) -> Option<Expr> {
    match arg {
        Some(expr @ ast::Expr::Record { .. }) => Some(lower_expr(expr)),
        _ => None,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

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
        let module = parse_daml(source, Path::new("Test.daml"));
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
        assert!(t.choices[0].body.iter().any(
            |s| matches!(s, Statement::Create { template_name, .. } if template_name == "this")
        ));
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
        let module = parse_daml(source, Path::new("Round.daml"));
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
        let module = parse_daml(source, Path::new("Foo.daml"));
        assert_eq!(module.templates[0].choices.len(), 1);
        assert!(!module.templates[0].choices[0].consuming);
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
        let module = parse_daml(source, Path::new("Foo.daml"));
        let by = |n: &str| {
            module.templates[0]
                .choices
                .iter()
                .find(|c| c.name == n)
                .unwrap_or_else(|| panic!("choice {} not found", n))
        };
        assert!(by("Drain").consuming, "preconsuming archives -> consuming");
        assert!(by("Close").consuming, "postconsuming archives -> consuming");
        assert!(by("Normal").consuming, "default choice is consuming");
        assert!(!by("Peek").consuming, "nonconsuming is not consuming");
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
        let module = parse_daml(source, Path::new("Foo.daml"));
        let body = &module.templates[0].choices[0].body;
        assert!(
            !body.iter().any(|s| matches!(s, Statement::Exercise { .. })),
            "comment text must not become an Exercise statement: {:?}",
            body
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
        let module = parse_daml(source, Path::new("Foo.daml"));
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
        let module = parse_daml(source, Path::new("Foo.daml"));
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
        let module = parse_daml(source, Path::new("Foo.daml"));
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
        let module = parse_daml(source, Path::new("Base.daml"));
        assert!(
            module.functions.is_empty(),
            "interface methods must not be extracted as top-level functions: {:?}",
            module.functions.iter().map(|f| &f.name).collect::<Vec<_>>()
        );
    }
}
