//! Lowering: typed AST (from the `daml-parser` crate) → rule-facing IR
//! (src/ir.rs).
//!
//! This replaces the old line-based keyword shim. The IR shapes are the
//! stable contract with rule scripts; structured `Expr` and `TypeNode`
//! payloads carry the actual parse tree.

use crate::ir::{
    BranchArm, CaseAlt, Choice, Consuming, DamlModule, EnsureClause, Expr, Field, Function, Import,
    ImportStyle, Interface, InterfaceInstance, InterfaceMethod, LetBinding, LiteralKind,
    RecordField, Span, SrcPos, Statement, Template, TypeNode,
};
use daml_parser::ast::{
    self, Consuming as ParserConsuming, Decl, DiagnosticCategory as ParserDiagnosticCategory,
    DoStmt, ImportStyle as ParserImportStyle, TemplateBodyDecl,
};
use daml_syntax::{Coordinate, SourceFile};
use std::path::Path;

#[cfg(all(test, feature = "js-runtime"))]
pub(crate) fn parse_daml(source: &str, file: &Path) -> DamlModule {
    parse_daml_with_diagnostics(source, file).module
}

/// A parse diagnostic for the caller to report.
///
/// `end_column` is present when the offending span sits on a single line (most
/// tokens); `category` is the parser's recovery classification.
#[derive(Debug)]
#[non_exhaustive]
pub struct ParseDiagnostic {
    pub line: usize,
    pub column: usize,
    pub end_column: Option<usize>,
    pub message: String,
    pub category: ParseDiagnosticCategory,
}

/// Stable, machine-readable parse-diagnostic categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseDiagnosticCategory {
    /// A declaration could not be parsed and was skipped to the next item.
    SkippedDeclaration,
    /// A malformed expression, pattern, or expected-token error inside an
    /// otherwise-recognized construct.
    Malformed,
    /// A construct the parser intentionally does not support, e.g. legacy
    /// `controller ... can` choice syntax.
    UnsupportedSyntax,
    /// Expression/pattern nesting exceeded the recursion bound and was degraded
    /// to raw text.
    RecursionLimit,
    /// A lexical error (unterminated string/comment, stray character).
    LexicalError,
    /// The parser reported an unknown or forward-compatible diagnostic category.
    Unknown,
}

impl ParseDiagnosticCategory {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SkippedDeclaration => "skipped-declaration",
            Self::Malformed => "malformed",
            Self::UnsupportedSyntax => "unsupported-syntax",
            Self::RecursionLimit => "recursion-limit",
            Self::LexicalError => "lexical-error",
            Self::Unknown => "unknown",
        }
    }

    #[must_use]
    pub const fn from_parser_category(category: ParserDiagnosticCategory) -> Self {
        match category {
            ParserDiagnosticCategory::SkippedDecl => Self::SkippedDeclaration,
            ParserDiagnosticCategory::Malformed => Self::Malformed,
            ParserDiagnosticCategory::UnsupportedSyntax => Self::UnsupportedSyntax,
            ParserDiagnosticCategory::RecursionLimit => Self::RecursionLimit,
            ParserDiagnosticCategory::Lex => Self::LexicalError,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub struct ParseResult {
    pub module: DamlModule,
    pub diagnostics: Vec<ParseDiagnostic>,
}

/// Parse DAML source into the lint IR plus parse diagnostics.
///
/// Parsing is loss-tolerant: a `DamlModule` is returned even when `diags` is
/// non-empty.
///
/// # API note
///
/// `ParseResult` is the supported return type (`{ module, diagnostics }`) and
/// replaces the previous tuple-shaped return. This is a breaking API shape
/// change in favor of clearer, named-field access.
#[must_use]
pub fn parse_daml_with_diagnostics(source: &str, file: &Path) -> ParseResult {
    let source_file = SourceFile::parse(source);
    let module = source_file.module();
    let imports = module
        .imports
        .iter()
        .map(|i| Import {
            module_name: i.module_name.to_string(),
            qualified: if i.style == ParserImportStyle::Qualified {
                ImportStyle::Qualified
            } else {
                ImportStyle::Unqualified
            },
            alias: i.alias.clone().map(|alias| alias.to_string()),
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

    let module = DamlModule {
        ir_version: 4,
        name: module.name.to_string(),
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
        .map(|d| ParseDiagnostic {
            line: d.line().get(),
            column: d.column().get(),
            end_column: d.end_column().map(Coordinate::get),
            message: d.message().to_owned(),
            category: ParseDiagnosticCategory::from_parser_category(d.category()),
        })
        .collect();
    ParseResult {
        module,
        diagnostics: diags,
    }
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
            name: name.to_string(),
            qualifier: qualifier.to_owned().map(String::from),
            span,
        },
        ast::Expr::Con {
            qualifier, name, ..
        } => Expr::Con {
            name: name.to_string(),
            qualifier: qualifier.to_owned().map(String::from),
            span,
        },
        ast::Expr::Lit { kind, text, .. } => Expr::Lit {
            kind: match kind {
                ast::LitKind::Int => LiteralKind::Int,
                ast::LitKind::Decimal => LiteralKind::Decimal,
                ast::LitKind::Char => LiteralKind::Char,
                _ => LiteralKind::Text,
            },
            value: text.clone(),
            span,
        },
        ast::Expr::App { func, args, .. } => Expr::App {
            func: Box::new(lower_expr(func)),
            args: args.iter().map(lower_expr).collect(),
            span,
        },
        ast::Expr::BinOp { op, lhs, rhs, .. } => Expr::BinOp {
            op: op.to_string(),
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
                .map(|f| match f {
                    ast::FieldAssign::Assign { name, value, .. } => RecordField {
                        name: name.to_string(),
                        value: Some(lower_expr(value)),
                    },
                    ast::FieldAssign::Pun { name, .. } => RecordField {
                        name: name.to_string(),
                        value: None,
                    },
                    ast::FieldAssign::Wildcard { .. } => RecordField {
                        name: "..".to_string(),
                        value: None,
                    },
                    _ => RecordField {
                        name: String::new(),
                        value: None,
                    },
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
        _ => Expr::Unknown {
            raw: e.render(),
            span,
        },
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
            name: f.name.to_string(),
            type_: f
                .ty
                .as_type()
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
                    .as_type()
                    .map(|ty| TypeNode::from_type(ty, file, source_file));
            }
            TemplateBodyDecl::Maintainer { expr, .. } => {
                maintainer_exprs.push(lower_expr(expr));
            }
            TemplateBodyDecl::Choice(c) => choices.push(lower_choice(c, file, source_file)),
            TemplateBodyDecl::InterfaceInstance(ii) => {
                interface_instances.push(InterfaceInstance {
                    interface_name: ii.interface_name.to_string(),
                    methods: ii.methods.iter().map(binding_name).collect(),
                    span: span_at(file, ii.pos),
                });
            }
            _ => {}
        }
    }

    Template {
        name: t.name.to_string(),
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
        name: i.name.to_string(),
        requires: i.requires.iter().map(ToString::to_string).collect(),
        viewtype: i.viewtype.to_owned().map(String::from),
        methods: i
            .methods
            .iter()
            .map(|m| InterfaceMethod {
                name: m.name.to_string(),
                type_: m
                    .ty
                    .as_type()
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
            name: f.name.to_string(),
            type_: f
                .ty
                .as_type()
                .map(|ty| TypeNode::from_type(ty, file, source_file)),
            span: span_at(file, f.pos),
        })
        .collect();

    let body = c.body.as_ref().map_or_else(Vec::new, statements_of_expr);

    Choice {
        name: c.name.to_string(),
        // pre/postconsuming choices archive the contract just like the default
        // consuming form; only NonConsuming leaves it live.
        consuming: if c.consuming == ParserConsuming::NonConsuming {
            Consuming::NonConsuming
        } else {
            Consuming::Consuming
        },
        controller_exprs: c.controllers.iter().map(lower_expr).collect(),
        observer_exprs: c.observers.iter().map(lower_expr).collect(),
        parameters,
        return_type: c
            .return_ty
            .as_type()
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
        name: f.name.to_string(),
        type_signature: f
            .ty
            .as_type()
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
            _ => {}
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
            ast::Pat::Var { name, .. } => Some(name.to_string()),
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
    let Some(helper) = helpers.get(name.as_str()) else {
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
                .map(|f| match f {
                    ast::FieldAssign::Assign {
                        name,
                        value,
                        pos,
                        span,
                    } => ast::FieldAssign::Assign {
                        name: name.clone(),
                        value: subst_expr(value, subst, call_pos),
                        pos: *pos,
                        span: *span,
                    },
                    ast::FieldAssign::Pun { name, pos, span } => ast::FieldAssign::Pun {
                        name: name.clone(),
                        pos: *pos,
                        span: *span,
                    },
                    ast::FieldAssign::Wildcard { pos, span } => ast::FieldAssign::Wildcard {
                        pos: *pos,
                        span: *span,
                    },
                    _ => f.clone(),
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
        | E::OperatorRef { pos, .. }
        | E::LeftSection { pos, .. }
        | E::RightSection { pos, .. }
        | E::Error { pos, .. } => *pos = call_pos,
        _ => {}
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
            if op.as_str() == "$" {
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
            .map_or_else(|| name.to_string(), |q| format!("{q}.{name}")),
        Some(ast::Expr::Var { name, .. }) if name.as_str() == "this" => "this".to_string(),
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
            .map_or_else(|| name.to_string(), |q| format!("{q}.{name}")),
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
