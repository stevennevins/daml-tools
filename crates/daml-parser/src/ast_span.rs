//! AST losslessness oracle for daml-fmt.
//!
//! Every AST node carries a byte `Span` (see `ast::Span`). This module proves
//! those spans are faithful: it collects every node's span, checks they form a
//! *laminar family* (each child contained in its parent, siblings ordered and
//! disjoint — `ast::Span` invariants), and reconstructs the file by tiling the
//! span forest with the verbatim source bytes that fall between sibling spans.
//!
//! If the spans nest correctly and the module span covers the file, the
//! reconstruction is byte-identical to the source. A parser that dropped a
//! token's bytes from every node span, or produced an overlap, fails here.

use crate::ast::*;
use crate::lexer::{Trivia, TriviaKind};

/// Reconstruct `source` from the AST's byte spans plus the lexer's `trivia`.
///
/// The AST-level mirror of `lexer::render_lossless`: it checks the spans nest
/// (V2), then tiles the file from every *content* node span merged with the
/// non-blank trivia spans (V1/V3). `Ok(reconstruction)` is byte-identical to
/// `source`; `Err` names the first nesting violation or the first run of bytes
/// no span covers (content the AST dropped).
///
/// Obtain `trivia` from [`crate::lexer::lex_with_trivia`].
pub fn render_from_ast(source: &str, module: &Module, trivia: &[Trivia]) -> Result<String, String> {
    check_nesting(module)?;
    tile(source, module, trivia)
}

/// V2 — every child span is contained in its parent, and sibling spans are
/// ordered and disjoint. Validates the whole node set (module container
/// included) as a laminar family via a containment stack.
pub fn check_nesting(module: &Module) -> Result<(), String> {
    let mut spans: Vec<Span> = Vec::new();
    collect_module(module, &mut spans);
    spans.retain(|s| !s.is_empty());
    // Outer-first: earlier start, then later end.
    spans.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));

    let mut stack: Vec<Span> = Vec::new();
    for span in spans {
        // Pop ancestors that end at/before this span starts (its left siblings).
        while stack.last().is_some_and(|top| top.end <= span.start) {
            stack.pop();
        }
        // Whatever remains on top must contain this span; otherwise the two
        // overlap without nesting (a sibling that starts before the previous
        // one ended, or a child that spills past its parent).
        if let Some(parent) = stack.last() {
            if !parent.contains(&span) {
                return Err(format!(
                    "span [{}, {}) overlaps [{}, {}) without nesting",
                    span.start, span.end, parent.start, parent.end
                ));
            }
        }
        stack.push(span);
    }
    Ok(())
}

/// V1/V3 — tile the file from content spans + non-blank trivia and reconstruct.
/// "Content" excludes the whole-module container (which would cover everything
/// trivially) but includes the `module … where` header. Any gap between spans
/// must be whitespace-only; a non-whitespace gap is a real token no node claims,
/// i.e. content the AST dropped.
fn tile(source: &str, module: &Module, trivia: &[Trivia]) -> Result<String, String> {
    let mut content: Vec<Span> = Vec::new();
    collect_module(module, &mut content);
    let container = module.span;
    content.retain(|s| !(s.is_empty() || (s.start == container.start && s.end == container.end)));
    if !module.header.is_empty() {
        content.push(module.header);
    }

    let mut items: Vec<(usize, usize)> = content.iter().map(|s| (s.start, s.end)).collect();
    // Blank-line trivia carry no bytes; comment/CPP trivia fill the gaps that
    // are legitimately not AST nodes.
    items.extend(
        trivia
            .iter()
            .filter(|t| !matches!(t.kind, TriviaKind::BlankLines(_)))
            .map(|t| (t.start, t.end)),
    );
    items.sort_unstable();

    let mut out = String::with_capacity(source.len());
    let mut prev = 0usize;
    for (start, end) in items {
        if start < prev {
            // Overlapping intervals can't tile; nesting check should have caught
            // structural overlaps, so this is a span/trivia inconsistency.
            continue;
        }
        let gap = &source[prev..start];
        if !gap.chars().all(char::is_whitespace) {
            return Err(format!(
                "bytes {}..{} not covered by any node or trivia span: {:?}",
                prev, start, gap
            ));
        }
        out.push_str(gap);
        out.push_str(&source[start..end]);
        prev = end;
    }
    let tail = &source[prev..];
    if !tail.chars().all(char::is_whitespace) {
        return Err(format!("bytes {}.. lost at EOF: {:?}", prev, tail));
    }
    out.push_str(tail);

    if out != source {
        return Err(format!(
            "reconstruction differs from source ({} vs {} bytes)",
            out.len(),
            source.len()
        ));
    }
    Ok(out)
}

// ----- span collection ---------------------------------------------------

fn collect_module(m: &Module, out: &mut Vec<Span>) {
    out.push(m.span);
    if !m.header.is_empty() {
        out.push(m.header);
    }
    for imp in &m.imports {
        out.push(imp.span);
    }
    for d in &m.decls {
        collect_decl(d, out);
    }
}

fn collect_decl(d: &Decl, out: &mut Vec<Span>) {
    match d {
        Decl::Template(t) => {
            out.push(t.span);
            for f in &t.fields {
                out.push(f.span);
            }
            for b in &t.body {
                collect_tbody(b, out);
            }
        }
        Decl::Interface(i) => {
            out.push(i.span);
            for m in &i.methods {
                out.push(m.span);
            }
            for c in &i.choices {
                collect_choice(c, out);
            }
        }
        Decl::Function(f) => {
            // `f.span` is the equations' extent (contiguous); the signature,
            // which may sit apart, is a separate sibling span.
            out.push(f.span);
            for eq in &f.equations {
                collect_eq(eq, out);
            }
            if let Some(sig) = f.sig_span {
                out.push(sig);
            }
        }
        Decl::TypeDef { span, .. } | Decl::Unknown { span, .. } => out.push(*span),
    }
}

fn collect_tbody(b: &TemplateBodyDecl, out: &mut Vec<Span>) {
    match b {
        TemplateBodyDecl::Signatory { parties, span, .. }
        | TemplateBodyDecl::Observer { parties, span, .. } => {
            out.push(*span);
            for e in parties {
                collect_expr(e, out);
            }
        }
        TemplateBodyDecl::Ensure { expr, span, .. }
        | TemplateBodyDecl::Maintainer { expr, span, .. }
        | TemplateBodyDecl::Key { expr, span, .. } => {
            out.push(*span);
            collect_expr(expr, out);
        }
        TemplateBodyDecl::Choice(c) => collect_choice(c, out),
        TemplateBodyDecl::InterfaceInstance(ii) => {
            out.push(ii.span);
            for m in &ii.methods {
                collect_binding(m, out);
            }
        }
        TemplateBodyDecl::Other { span, .. } => out.push(*span),
    }
}

fn collect_choice(c: &ChoiceDecl, out: &mut Vec<Span>) {
    out.push(c.span);
    for p in &c.params {
        out.push(p.span);
    }
    for e in &c.controllers {
        collect_expr(e, out);
    }
    for e in &c.observers {
        collect_expr(e, out);
    }
    if let Some(b) = &c.body {
        collect_expr(b, out);
    }
}

fn collect_eq(eq: &Equation, out: &mut Vec<Span>) {
    out.push(eq.span);
    for p in &eq.params {
        collect_pat(p, out);
    }
    collect_expr(&eq.body, out);
    for (g, b) in &eq.guards {
        collect_expr(g, out);
        collect_expr(b, out);
    }
    for wb in &eq.where_bindings {
        collect_binding(wb, out);
    }
}

fn collect_binding(b: &Binding, out: &mut Vec<Span>) {
    out.push(b.span);
    collect_pat(&b.pat, out);
    for p in &b.params {
        collect_pat(p, out);
    }
    collect_expr(&b.expr, out);
}

fn collect_pat(p: &Pat, out: &mut Vec<Span>) {
    out.push(p.span());
    match p {
        Pat::Con { args, .. } => {
            for a in args {
                collect_pat(a, out);
            }
        }
        Pat::Tuple { items, .. } | Pat::List { items, .. } => {
            for it in items {
                collect_pat(it, out);
            }
        }
        Pat::As { pat, .. } => collect_pat(pat, out),
        _ => {}
    }
}

fn collect_expr(e: &Expr, out: &mut Vec<Span>) {
    out.push(e.span());
    match e {
        Expr::App { func, args, .. } => {
            collect_expr(func, out);
            for a in args {
                collect_expr(a, out);
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_expr(lhs, out);
            collect_expr(rhs, out);
        }
        Expr::Neg { expr, .. } => collect_expr(expr, out),
        Expr::Lambda { params, body, .. } => {
            for p in params {
                collect_pat(p, out);
            }
            collect_expr(body, out);
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_expr(cond, out);
            collect_expr(then_branch, out);
            collect_expr(else_branch, out);
        }
        Expr::Case {
            scrutinee, alts, ..
        } => {
            collect_expr(scrutinee, out);
            for a in alts {
                collect_alt(a, out);
            }
        }
        Expr::Do { stmts, .. } => {
            for s in stmts {
                collect_dostmt(s, out);
            }
        }
        Expr::LetIn { bindings, body, .. } => {
            for b in bindings {
                collect_binding(b, out);
            }
            collect_expr(body, out);
        }
        Expr::Record { base, fields, .. } => {
            collect_expr(base, out);
            for f in fields {
                collect_field_assign(f, out);
            }
        }
        Expr::Tuple { items, .. } | Expr::List { items, .. } => {
            for it in items {
                collect_expr(it, out);
            }
        }
        Expr::Try { body, handlers, .. } => {
            collect_expr(body, out);
            for h in handlers {
                collect_alt(h, out);
            }
        }
        Expr::Section {
            operand: Some(o), ..
        } => collect_expr(o, out),
        _ => {}
    }
}

fn collect_alt(a: &Alt, out: &mut Vec<Span>) {
    out.push(a.span);
    collect_pat(&a.pat, out);
    collect_expr(&a.body, out);
}

fn collect_field_assign(f: &FieldAssign, out: &mut Vec<Span>) {
    out.push(f.span);
    if let Some(v) = &f.value {
        collect_expr(v, out);
    }
}

fn collect_dostmt(s: &DoStmt, out: &mut Vec<Span>) {
    match s {
        DoStmt::Bind {
            pat, expr, span, ..
        } => {
            out.push(*span);
            collect_pat(pat, out);
            collect_expr(expr, out);
        }
        DoStmt::Let { bindings, span, .. } => {
            out.push(*span);
            for b in bindings {
                collect_binding(b, out);
            }
        }
        DoStmt::Expr { expr, span, .. } => {
            out.push(*span);
            collect_expr(expr, out);
        }
    }
}
