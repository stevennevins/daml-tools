//! AST-driven canonical layout.
//!
//! This is OUR OWN pattern, NOT a port of the LimeChain heuristic: no LimeChain
//! code is consulted; design is from the daml-parser AST.
//! We do NOT aim to match `expected/`: the formatter makes its own consistent
//! layout decisions and may diverge from the LimeChain baseline). This is the
//! shipping backend behind `format_source`.
//!
//! Uniform mechanism (the architecture the prototype validated):
//!   1. Walk the AST; every node carries a byte `Span`.
//!   2. For a layout block, reindent ONLY the block's child lines so the anchor
//!      construct lands at `anchor_col + 2`: a `do`-block's statements to
//!      `do_col + 2` (including a `do` that opens with `let`), an
//!      `if`/`then`/`else`'s `then`/`else` clauses to `if_col + 2`, a
//!      `case … of`'s alternatives to `case_col + 2`, a `let … in` expression's
//!      bindings to `let_col + 2`, and a `Con with` construction's fields to
//!      `con_col + 2`. The anchor line itself is never moved, which makes each
//!      rule a fixpoint: a second pass computes delta 0. Children shift by one
//!      uniform delta, so the block's internal structure (and any nested blocks)
//!      ride along. A `template`/`interface` body is the one STRUCTURED rule: a
//!      template's `with`/`where` keywords go to `head_col + 2` and the field /
//!      signatory-choice-decl blocks to `head_col + 4` (two different deltas),
//!      so a 4-space ladder collapses to the canonical 2-space one; an
//!      interface's inline-`where` body sits at `head_col + 2`.
//!   3. Comments are sacred: comment lines are never measured-from and never
//!      shifted (CLAUDE.md). Block-comment interiors are trivia, untouched.
//!   4. Gate the whole candidate on `same_tokens` = identical LAID-OUT token
//!      stream (offside virtuals included) ⇒ identical parse ⇒ identical
//!      desugar. Any accepted reindent is desugar-safe BY CONSTRUCTION.
//!   5. Fall back to the input (verbatim) when the gate rejects or a node is
//!      not modeled. Unmodeled constructs (guards, record UPDATES (`expr with`),
//!      `data` declarations, TypeDef, expression continuations) pass through
//!      verbatim — desugar-safe and lets us land one construct at a time.
//!
//! On top of the structural pass we compose the proven, token-gated
//! whitespace + colon-spacing normalization (`crate::normalize_gaps`).

use daml_parser::ast::*;
use daml_parser::layout::resolve_layout;
use daml_parser::lexer::{lex_with_trivia, TriviaKind};
use daml_parser::parse::parse_module;

const INDENT: i64 = 2;

/// Upper bound on structural-reindent iterations. The do-pass and if-pass can
/// unblock one another (the if-pass's `else` shift can remove a collision that
/// made the do-pass's gate reject), so they are iterated to a fixpoint for
/// single-call idempotence. Real inputs converge in 1-2; the cap only guards a
/// pathological non-convergence (the last output is still gate-safe).
const MAX_STRUCTURAL_PASSES: usize = 6;

/// Format with the AST-driven structural reindents, then the gap normalization;
/// every step is gated for desugar-safety.
pub fn format_ast(src: &str) -> String {
    // Step 1: structural reindent (do-blocks, then if/then/else), each its own
    // gated pass. Iterate to a fixpoint so a later pass that unblocks an earlier
    // one's gate still converges within a single call (idempotence).
    let mut base = src.to_string();
    for _ in 0..MAX_STRUCTURAL_PASSES {
        let next = gated_template_pass(&gated_con_with_pass(&gated_letin_pass(&gated_case_pass(
            &gated_if_pass(&gated_do_pass(&base)),
        ))));
        if next == base {
            break;
        }
        base = next;
    }

    // Step 2: whitespace + colon normalization on top, gated vs `base`.
    // same_tokens is transitive, so gating each step against its own input
    // keeps the final output token-equivalent (and desugar-equivalent) to src.
    let full = crate::normalize_gaps(&base, true);
    if same_tokens(&base, &full) {
        return full;
    }
    let ws_only = crate::normalize_gaps(&base, false);
    if same_tokens(&base, &ws_only) {
        return ws_only;
    }
    base
}

/// Do-block reindent of `src`, accepted only if it passes the `same_tokens`
/// gate; otherwise `src` unchanged.
fn gated_do_pass(src: &str) -> String {
    let (module, _) = parse_module(src);
    let r = reindent_do_blocks(src, &module);
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// if/then/else clause reindent of `src`, gated like the do-pass. Re-parses its
/// own input so spans match the (possibly already do-reindented) bytes.
fn gated_if_pass(src: &str) -> String {
    let (module, _) = parse_module(src);
    let r = reindent_ifs(src, &module);
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// case-alternative reindent of `src`, gated like the do-pass.
fn gated_case_pass(src: &str) -> String {
    let (module, _) = parse_module(src);
    let r = reindent_cases(src, &module);
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// `let … in` binding-block reindent of `src`, gated like the do-pass.
fn gated_letin_pass(src: &str) -> String {
    let (module, _) = parse_module(src);
    let r = reindent_letins(src, &module);
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// `Con with` construction field-block reindent of `src`, gated like the do-pass.
fn gated_con_with_pass(src: &str) -> String {
    let (module, _) = parse_module(src);
    let r = reindent_con_with(src, &module);
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// Structured template-body reindent of `src`, gated like the do-pass.
fn gated_template_pass(src: &str) -> String {
    let (module, _) = parse_module(src);
    let r = reindent_templates(src, &module);
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// Count structural edit candidates over modeled AST constructs.
///
/// This powers `bin/coverage`; unlike the original do-only metric, it covers
/// every current AST layout family: do, if, case, let-in, constructor `with`,
/// and template/interface bodies. This is not a normalized coverage ratio: one
/// construct can produce multiple edits.
pub fn coverage(src: &str) -> (usize, usize) {
    let (module, _diags) = parse_module(src);
    let candidates = do_block_edits(src, &module).len()
        + if_edits(src, &module).len()
        + case_edits(src, &module).len()
        + letin_edits(src, &module).len()
        + con_with_edits(src, &module).len()
        + template_edits(src, &module).len();
    (candidates, modeled_construct_count(&module))
}

fn modeled_construct_count(module: &Module) -> usize {
    let mut count = 0usize;
    walk_module_exprs(module, &mut |e| match e {
        Expr::Do { .. } | Expr::If { .. } | Expr::Case { .. } | Expr::LetIn { .. } => count += 1,
        Expr::Record { base, fields, .. }
            if matches!(base.as_ref(), Expr::Con { .. }) && !fields.is_empty() =>
        {
            count += 1
        }
        _ => {}
    });
    for d in &module.decls {
        match d {
            Decl::Template(t) if !t.fields.is_empty() || !t.body.is_empty() => count += 1,
            Decl::Interface(i) if !i.methods.is_empty() || !i.choices.is_empty() => count += 1,
            _ => {}
        }
    }
    count
}

/// True iff `a` and `b` share the same LAID-OUT token stream (offside virtuals
/// included) — the desugar-safety gate.
fn same_tokens(a: &str, b: &str) -> bool {
    let la = resolve_layout(lex_with_trivia(a).0);
    let lb = resolve_layout(lex_with_trivia(b).0);
    la.len() == lb.len() && la.iter().zip(&lb).all(|(x, y)| x.tok == y.tok)
}

/// One reindent: shift every child line in `[child_start, block_end)` by `delta`.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Edit {
    child_start: usize,
    block_end: usize,
    delta: i64,
}

/// Apply every do-block edit: shift child-line indentation so each accepted
/// block's first real statement lands at `do_col + 2`.
fn reindent_do_blocks(src: &str, module: &Module) -> String {
    let edits = do_block_edits(src, module);
    if edits.is_empty() {
        return src.to_string();
    }
    apply_shifts(src, &edits)
}

/// Compute the (non-zero) shift for each OUTERMOST, eligible do-block. Nested
/// do-blocks are skipped — they ride along inside their parent's child region.
fn do_block_edits(src: &str, module: &Module) -> Vec<Edit> {
    let mut dos: Vec<Span> = Vec::new();
    collect_do_spans(module, &mut dos);
    // Outermost first (smaller start, then larger end).
    dos.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));

    let line_starts = line_start_table(src);
    let comments = comment_spans(src);

    let mut edits: Vec<Edit> = Vec::new();
    let mut accepted: Vec<Span> = Vec::new();
    for d in dos {
        // Skip a do-block nested in one we already accepted (it rides along).
        if accepted
            .iter()
            .any(|a| a.start <= d.start && d.end <= a.end && *a != d)
        {
            continue;
        }
        // VERBATIM guards (plan): try/catch and do-`let` bodies are not modeled.
        if do_block_is_verbatim(d, module) {
            accepted.push(d); // claim the region so nested do-blocks stay verbatim too
            continue;
        }
        let do_line = line_of(&line_starts, d.start);
        let do_indent = indent_of(src, &line_starts, do_line);
        // First real (non-blank, non-comment) statement line after the do line.
        let Some(first_stmt_line) =
            first_code_line_after(src, &line_starts, &comments, do_line, d.end)
        else {
            continue; // inline `do stmt` — nothing on its own line; leave it
        };
        accepted.push(d);
        // Tab-indented bodies are left verbatim: we measure/emit only spaces,
        // so shifting would prepend spaces in front of tabs (silent mangling).
        if leading_has_tab(src, line_starts[first_stmt_line]) {
            continue;
        }
        let cur = indent_of(src, &line_starts, first_stmt_line);
        let delta = (do_indent + INDENT) - cur;
        if delta != 0 {
            edits.push(Edit {
                child_start: line_starts[first_stmt_line],
                block_end: d.end,
                delta,
            });
        }
    }
    edits
}

/// Is this do-block one of the shapes we deliberately leave verbatim?
/// - try/catch body (`do ... try ... catch ...`): handler layout is its own
///   problem; the plan keeps it verbatim. (`code-snippets-dev/Exceptions.daml`)
///
/// A `do` whose first statement is a `let` is NOT verbatim: the uniform-delta
/// reindent shifts the whole block (the `let` line and its continuation
/// bindings alike) by one amount, so the let-block's internal offside alignment
/// rides along unchanged — exactly as nested do-blocks do. The `same_tokens`
/// gate still rejects anything that would alter the laid-out token stream, and
/// the full desugar sweep confirms no new non-equivalence. (Corpus citation:
/// `sdk/compiler/damlc/tests/daml-test-files/ApplicativeDo.daml`.)
fn do_block_is_verbatim(do_span: Span, module: &Module) -> bool {
    let mut found = false;
    let mut verbatim = false;
    visit_do(module, &mut |span, stmts| {
        if span == do_span && !found {
            found = true;
            let has_try = stmts.iter().any(|s| match s {
                DoStmt::Bind { expr, .. } | DoStmt::Expr { expr, .. } => expr_contains_try(expr),
                // A `try` buried in a let-binding RHS counts too, so the
                // "try/catch stays verbatim" rule holds uniformly rather than
                // resting on the gate to catch it after the fact.
                DoStmt::Let { bindings, .. } => bindings.iter().any(|b| expr_contains_try(&b.expr)),
            });
            verbatim = has_try;
        }
    });
    verbatim
}

fn expr_contains_try(e: &Expr) -> bool {
    match e {
        Expr::Try { .. } => true,
        Expr::App { func, args, .. } => {
            expr_contains_try(func) || args.iter().any(expr_contains_try)
        }
        Expr::BinOp { lhs, rhs, .. } => expr_contains_try(lhs) || expr_contains_try(rhs),
        Expr::Neg { expr, .. } | Expr::Lambda { body: expr, .. } => expr_contains_try(expr),
        _ => false,
    }
}

/// Shift the leading-space indentation of every code line whose first content
/// byte lies in some edit's child region, by that edit's delta. Blank lines and
/// comment lines are never touched (comments are sacred). do-edits never
/// overlap (nested do-blocks are skipped), so any line matches at most one edit.
fn apply_shifts(src: &str, edits: &[Edit]) -> String {
    let line_starts = line_start_table(src);
    let comments = comment_spans(src);
    let mut out = String::with_capacity(src.len());
    for (li, &ls) in line_starts.iter().enumerate() {
        let le = *line_starts.get(li + 1).unwrap_or(&src.len());
        let line = &src[ls..le];
        let trimmed = line.trim_start_matches(' ');
        let cur = line.len() - trimmed.len();
        let content_byte = ls + cur;

        let delta = edits
            .iter()
            .find(|e| e.child_start <= content_byte && content_byte < e.block_end)
            .map(|e| e.delta)
            .unwrap_or(0);

        if delta == 0
            || line.trim().is_empty()
            || is_comment_line(&comments, content_byte)
            || leading_has_tab(src, ls)
        {
            out.push_str(line);
            continue;
        }
        let new = (cur as i64 + delta).max(0) as usize;
        out.push_str(&" ".repeat(new));
        out.push_str(&line[cur..]);
    }
    out
}

// ---- comment-line awareness ------------------------------------------------

/// Byte spans of every comment (line + block); sorted by start.
fn comment_spans(src: &str) -> Vec<(usize, usize)> {
    let (_t, trivia, _e) = lex_with_trivia(src);
    let mut v: Vec<(usize, usize)> = trivia
        .iter()
        .filter(|t| matches!(t.kind, TriviaKind::LineComment | TriviaKind::BlockComment))
        .map(|t| (t.start, t.end))
        .collect();
    v.sort_by_key(|&(s, _)| s);
    v
}

/// True if the line's first content byte falls inside a comment span.
fn is_comment_line(comments: &[(usize, usize)], content_byte: usize) -> bool {
    comments
        .iter()
        .any(|&(s, e)| s <= content_byte && content_byte < e)
}

// ---- line-table helpers ----------------------------------------------------

fn line_start_table(src: &str) -> Vec<usize> {
    std::iter::once(0)
        .chain(src.match_indices('\n').map(|(i, _)| i + 1))
        .collect()
}
fn line_of(line_starts: &[usize], byte: usize) -> usize {
    match line_starts.binary_search(&byte) {
        Ok(i) => i,
        Err(i) => i - 1,
    }
}
fn indent_of(src: &str, line_starts: &[usize], line: usize) -> i64 {
    src[line_starts[line]..]
        .chars()
        .take_while(|&c| c == ' ')
        .count() as i64
}
/// True if the line beginning at `line_start` has a tab anywhere in its leading
/// whitespace. Such lines are left verbatim (we measure/emit spaces only).
fn leading_has_tab(src: &str, line_start: usize) -> bool {
    src[line_start..]
        .chars()
        .take_while(|&c| c == ' ' || c == '\t')
        .any(|c| c == '\t')
}

/// First line strictly after `do_line` (and before `block_end`) that is neither
/// blank nor a comment line — i.e. the first real statement.
fn first_code_line_after(
    src: &str,
    line_starts: &[usize],
    comments: &[(usize, usize)],
    do_line: usize,
    block_end: usize,
) -> Option<usize> {
    let mut l = do_line + 1;
    while l < line_starts.len() && line_starts[l] < block_end {
        let ls = line_starts[l];
        let le = *line_starts.get(l + 1).unwrap_or(&src.len());
        let line = &src[ls..le];
        let cur = line.len() - line.trim_start_matches(' ').len();
        if !line.trim().is_empty() && !is_comment_line(comments, ls + cur) {
            return Some(l);
        }
        l += 1;
    }
    None
}

// ---- AST walks -------------------------------------------------------------

/// Visit every `Expr::Do` with (span, &stmts).
fn visit_do(m: &Module, f: &mut impl FnMut(Span, &[DoStmt])) {
    for d in &m.decls {
        match d {
            Decl::Function(fun) => {
                for eq in &fun.equations {
                    visit_do_expr(&eq.body, f);
                    for (g, b) in &eq.guards {
                        visit_do_expr(g, f);
                        visit_do_expr(b, f);
                    }
                    for wb in &eq.where_bindings {
                        visit_do_expr(&wb.expr, f);
                    }
                }
            }
            Decl::Template(t) => {
                for b in &t.body {
                    match b {
                        TemplateBodyDecl::Choice(c) => {
                            if let Some(body) = &c.body {
                                visit_do_expr(body, f);
                            }
                        }
                        TemplateBodyDecl::Ensure { expr, .. }
                        | TemplateBodyDecl::Key { expr, .. }
                        | TemplateBodyDecl::Maintainer { expr, .. } => visit_do_expr(expr, f),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn visit_do_expr(e: &Expr, f: &mut impl FnMut(Span, &[DoStmt])) {
    if let Expr::Do { span, stmts, .. } = e {
        f(*span, stmts);
    }
    match e {
        Expr::App { func, args, .. } => {
            visit_do_expr(func, f);
            args.iter().for_each(|a| visit_do_expr(a, f));
        }
        Expr::BinOp { lhs, rhs, .. } => {
            visit_do_expr(lhs, f);
            visit_do_expr(rhs, f);
        }
        Expr::Neg { expr, .. } | Expr::Lambda { body: expr, .. } => visit_do_expr(expr, f),
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            visit_do_expr(cond, f);
            visit_do_expr(then_branch, f);
            visit_do_expr(else_branch, f);
        }
        Expr::Case {
            scrutinee, alts, ..
        } => {
            visit_do_expr(scrutinee, f);
            alts.iter().for_each(|a| visit_do_expr(&a.body, f));
        }
        Expr::Do { stmts, .. } => {
            for s in stmts {
                match s {
                    DoStmt::Bind { expr, .. } | DoStmt::Expr { expr, .. } => visit_do_expr(expr, f),
                    DoStmt::Let { bindings, .. } => {
                        bindings.iter().for_each(|b| visit_do_expr(&b.expr, f))
                    }
                }
            }
        }
        Expr::LetIn { bindings, body, .. } => {
            bindings.iter().for_each(|b| visit_do_expr(&b.expr, f));
            visit_do_expr(body, f);
        }
        Expr::Record { base, fields, .. } => {
            visit_do_expr(base, f);
            for fa in fields {
                if let Some(v) = &fa.value {
                    visit_do_expr(v, f);
                }
            }
        }
        Expr::Tuple { items, .. } | Expr::List { items, .. } => {
            items.iter().for_each(|it| visit_do_expr(it, f))
        }
        Expr::Try { body, handlers, .. } => {
            visit_do_expr(body, f);
            handlers.iter().for_each(|h| visit_do_expr(&h.body, f));
        }
        Expr::Section {
            operand: Some(o), ..
        } => visit_do_expr(o, f),
        _ => {}
    }
}

fn collect_do_spans(m: &Module, out: &mut Vec<Span>) {
    visit_do(m, &mut |span, _stmts| out.push(span));
}

/// Visit every expression in the module, pre-order. The generic walker behind
/// construct-specific rules (if/then/else, …); mirrors `visit_do`'s reach but
/// yields every node, not just `Do`.
fn walk_module_exprs(m: &Module, f: &mut impl FnMut(&Expr)) {
    for d in &m.decls {
        match d {
            Decl::Function(fun) => {
                for eq in &fun.equations {
                    walk_expr(&eq.body, f);
                    for (g, b) in &eq.guards {
                        walk_expr(g, f);
                        walk_expr(b, f);
                    }
                    for wb in &eq.where_bindings {
                        walk_expr(&wb.expr, f);
                    }
                }
            }
            Decl::Template(t) => {
                for b in &t.body {
                    match b {
                        TemplateBodyDecl::Choice(c) => {
                            if let Some(body) = &c.body {
                                walk_expr(body, f);
                            }
                        }
                        TemplateBodyDecl::Ensure { expr, .. }
                        | TemplateBodyDecl::Key { expr, .. }
                        | TemplateBodyDecl::Maintainer { expr, .. } => walk_expr(expr, f),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn walk_expr(e: &Expr, f: &mut impl FnMut(&Expr)) {
    f(e);
    match e {
        Expr::App { func, args, .. } => {
            walk_expr(func, f);
            args.iter().for_each(|a| walk_expr(a, f));
        }
        Expr::BinOp { lhs, rhs, .. } => {
            walk_expr(lhs, f);
            walk_expr(rhs, f);
        }
        Expr::Neg { expr, .. } | Expr::Lambda { body: expr, .. } => walk_expr(expr, f),
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            walk_expr(cond, f);
            walk_expr(then_branch, f);
            walk_expr(else_branch, f);
        }
        Expr::Case {
            scrutinee, alts, ..
        } => {
            walk_expr(scrutinee, f);
            alts.iter().for_each(|a| walk_expr(&a.body, f));
        }
        Expr::Do { stmts, .. } => {
            for s in stmts {
                match s {
                    DoStmt::Bind { expr, .. } | DoStmt::Expr { expr, .. } => walk_expr(expr, f),
                    DoStmt::Let { bindings, .. } => {
                        bindings.iter().for_each(|b| walk_expr(&b.expr, f))
                    }
                }
            }
        }
        Expr::LetIn { bindings, body, .. } => {
            bindings.iter().for_each(|b| walk_expr(&b.expr, f));
            walk_expr(body, f);
        }
        Expr::Record { base, fields, .. } => {
            walk_expr(base, f);
            for fa in fields {
                if let Some(v) = &fa.value {
                    walk_expr(v, f);
                }
            }
        }
        Expr::Tuple { items, .. } | Expr::List { items, .. } => {
            items.iter().for_each(|it| walk_expr(it, f))
        }
        Expr::Try { body, handlers, .. } => {
            walk_expr(body, f);
            handlers.iter().for_each(|h| walk_expr(&h.body, f));
        }
        Expr::Section {
            operand: Some(o), ..
        } => walk_expr(o, f),
        _ => {}
    }
}

/// Byte offset of the standalone keyword `kw` in `src[from..to)`, skipping any
/// match that falls inside a comment. The region between two sibling expression
/// spans is only layout + the keyword, so a word-boundary scan is safe.
fn find_keyword(
    src: &str,
    from: usize,
    to: usize,
    kw: &str,
    comments: &[(usize, usize)],
) -> Option<usize> {
    let hay = &src[from..to.min(src.len())];
    let bytes = hay.as_bytes();
    let mut i = 0;
    while let Some(rel) = hay[i..].find(kw) {
        let at = i + rel;
        let before_ok = at == 0 || !bytes[at - 1].is_ascii_alphanumeric();
        let after = at + kw.len();
        let after_ok = after >= hay.len() || !bytes[after].is_ascii_alphanumeric();
        let abs = from + at;
        if before_ok && after_ok && !is_comment_line(comments, abs) {
            return Some(abs);
        }
        i = at + 1;
    }
    None
}

/// Reindent the `then` and `else` clauses of multi-line `if`/`then`/`else` so
/// each keyword lands at `if_col + 2`. Only a clause whose keyword starts its
/// own line is moved, and the whole clause (keyword line + its branch's
/// continuation lines) shifts by ONE uniform delta — the let-block trick — so
/// the branch's internal layout is preserved. `same_tokens` still gates it.
fn if_edits(src: &str, module: &Module) -> Vec<Edit> {
    let line_starts = line_start_table(src);
    let comments = comment_spans(src);

    // (if_span, if_byte, cond_end, then_span, else_span), outermost first.
    let mut ifs: Vec<(Span, usize, usize, Span, Span)> = Vec::new();
    walk_module_exprs(module, &mut |e| {
        if let Expr::If {
            span,
            cond,
            then_branch,
            else_branch,
            ..
        } = e
        {
            ifs.push((
                *span,
                span.start,
                cond.span().end,
                then_branch.span(),
                else_branch.span(),
            ));
        }
    });
    ifs.sort_by(|a, b| a.0.start.cmp(&b.0.start).then(b.0.end.cmp(&a.0.end)));

    let mut edits: Vec<Edit> = Vec::new();
    let mut accepted: Vec<Span> = Vec::new();
    for (if_span, if_byte, cond_end, then_span, else_span) in ifs {
        // Skip an if nested in one we already claimed (it rides the outer shift).
        if accepted
            .iter()
            .any(|a| a.start <= if_span.start && if_span.end <= a.end && *a != if_span)
        {
            continue;
        }
        accepted.push(if_span);

        let if_line = line_of(&line_starts, if_byte);
        if leading_has_tab(src, line_starts[if_line]) {
            continue;
        }
        // Visual column of the `if` keyword on its line — count CHARACTERS, not
        // bytes, so a multibyte char before `if` does not over-indent (the shift
        // emits spaces and `indent_of` counts chars, so these must agree).
        let if_col = src[line_starts[if_line]..if_byte].chars().count() as i64;
        let target = if_col + INDENT;

        let then_byte = find_keyword(src, cond_end, then_span.start, "then", &comments);
        let else_byte = find_keyword(src, then_span.end, else_span.start, "else", &comments);

        for (kw_byte, branch_end) in [(then_byte, then_span.end), (else_byte, else_span.end)] {
            let Some(kw_byte) = kw_byte else { continue };
            let kw_line = line_of(&line_starts, kw_byte);
            let ls = line_starts[kw_line];
            // Only move a clause whose keyword STARTS its line (leading spaces
            // only). An inline `if c then x else y` is left alone.
            if src[ls..kw_byte].chars().any(|c| c != ' ') {
                continue;
            }
            if leading_has_tab(src, ls) {
                continue;
            }
            let cur = indent_of(src, &line_starts, kw_line);
            let delta = target - cur;
            if delta != 0 {
                edits.push(Edit {
                    child_start: ls,
                    block_end: branch_end,
                    delta,
                });
            }
        }
    }
    edits
}

fn reindent_ifs(src: &str, module: &Module) -> String {
    let edits = if_edits(src, module);
    if edits.is_empty() {
        return src.to_string();
    }
    apply_shifts(src, &edits)
}

/// Reindent the alternative block of a multi-line `case … of` so the alts land
/// at `case_line_indent + 2` (the same anchor convention as a `do`-block). The
/// whole alt block shifts by ONE uniform delta, so each alt's body — including
/// nested do/case/if — rides along; `same_tokens` rejects any shift that would
/// dedent the block below its offside requirement (e.g. a `case` hanging in a
/// `where` binding). Inline `case x of A -> …` alts and tab-indented blocks are
/// left verbatim. Mirrors `do_block_edits`.
fn case_edits(src: &str, module: &Module) -> Vec<Edit> {
    let line_starts = line_start_table(src);

    // (case_span, first_alt_start, last_alt_end), outermost first.
    let mut cases: Vec<(Span, usize, usize)> = Vec::new();
    walk_module_exprs(module, &mut |e| {
        if let Expr::Case { span, alts, .. } = e {
            if let (Some(first), Some(last)) = (alts.first(), alts.last()) {
                cases.push((*span, first.span.start, last.span.end));
            }
        }
    });
    cases.sort_by(|a, b| a.0.start.cmp(&b.0.start).then(b.0.end.cmp(&a.0.end)));

    let mut edits: Vec<Edit> = Vec::new();
    let mut accepted: Vec<Span> = Vec::new();
    for (case_span, first_alt, last_alt_end) in cases {
        // Skip a case nested in one we already claimed (it rides the outer shift).
        if accepted
            .iter()
            .any(|a| a.start <= case_span.start && case_span.end <= a.end && *a != case_span)
        {
            continue;
        }
        accepted.push(case_span);

        let case_line = line_of(&line_starts, case_span.start);
        let alt_line = line_of(&line_starts, first_alt);
        // Inline `case x of A -> …` (alts share the case line): leave verbatim.
        if alt_line <= case_line {
            continue;
        }
        if leading_has_tab(src, line_starts[case_line])
            || leading_has_tab(src, line_starts[alt_line])
        {
            continue;
        }
        let case_indent = indent_of(src, &line_starts, case_line);
        let cur = indent_of(src, &line_starts, alt_line);
        let delta = (case_indent + INDENT) - cur;
        if delta != 0 {
            edits.push(Edit {
                child_start: line_starts[alt_line],
                block_end: last_alt_end,
                delta,
            });
        }
    }
    edits
}

fn reindent_cases(src: &str, module: &Module) -> String {
    let edits = case_edits(src, module);
    if edits.is_empty() {
        return src.to_string();
    }
    apply_shifts(src, &edits)
}

/// Reindent the binding block of a `let … in` EXPRESSION so the bindings land at
/// `let_line_indent + 2` (the do/case convention). The bindings form a layout
/// block opened after `let`; the whole block shifts by ONE uniform delta, so
/// multi-line / multi-binding bodies ride along. `in` and the let body are left
/// alone; `same_tokens` rejects any shift whose result would relayout the block
/// (e.g. moving bindings off the `in` keyword's offside). Inline `let x = … in`
/// (binding shares the `let` line) and tab-indented blocks stay verbatim.
fn letin_edits(src: &str, module: &Module) -> Vec<Edit> {
    let line_starts = line_start_table(src);

    // (letin_span, first_binding_start, last_binding_end), outermost first.
    let mut lets: Vec<(Span, usize, usize)> = Vec::new();
    walk_module_exprs(module, &mut |e| {
        if let Expr::LetIn { span, bindings, .. } = e {
            if let (Some(first), Some(last)) = (bindings.first(), bindings.last()) {
                lets.push((*span, first.span.start, last.span.end));
            }
        }
    });
    lets.sort_by(|a, b| a.0.start.cmp(&b.0.start).then(b.0.end.cmp(&a.0.end)));

    let mut edits: Vec<Edit> = Vec::new();
    let mut accepted: Vec<Span> = Vec::new();
    for (let_span, first_bind, last_bind_end) in lets {
        if accepted
            .iter()
            .any(|a| a.start <= let_span.start && let_span.end <= a.end && *a != let_span)
        {
            continue;
        }
        accepted.push(let_span);

        let let_line = line_of(&line_starts, let_span.start);
        let bind_line = line_of(&line_starts, first_bind);
        // Inline `let x = … in …` (binding shares the let line): leave verbatim.
        if bind_line <= let_line {
            continue;
        }
        // Only canonicalize a LINE-LEADING `let`. For a mid-line `let` (`= let`,
        // `$ let`, a guard's `let`) the `in` keyword stays at the let-keyword
        // column while the bindings would anchor on the (smaller) line indent —
        // a mismatch. Unlike do/case (whose `name = do`/`= case` line-indent
        // convention is idiomatic), let-in needs `let` at line start for the
        // `bindings = let_indent + 2`, `in = let_indent` shape to line up.
        if src[line_starts[let_line]..let_span.start]
            .chars()
            .any(|c| c != ' ')
        {
            continue;
        }
        if leading_has_tab(src, line_starts[let_line])
            || leading_has_tab(src, line_starts[bind_line])
        {
            continue;
        }
        let let_indent = indent_of(src, &line_starts, let_line);
        let cur = indent_of(src, &line_starts, bind_line);
        let delta = (let_indent + INDENT) - cur;
        if delta != 0 {
            edits.push(Edit {
                child_start: line_starts[bind_line],
                block_end: last_bind_end,
                delta,
            });
        }
    }
    edits
}

fn reindent_letins(src: &str, module: &Module) -> String {
    let edits = letin_edits(src, module);
    if edits.is_empty() {
        return src.to_string();
    }
    apply_shifts(src, &edits)
}

/// Reindent the field block of a `Con with …` record CONSTRUCTION so the fields
/// land at `construction_line_indent + 2`. Only constructions (base is a bare
/// constructor `Con`) are touched — record UPDATES (`expr with …`) hang-align
/// inconsistently in the corpus and are left verbatim, as are inline and
/// tab-indented blocks. The field block shifts by ONE uniform delta (so nested
/// values ride along) and `same_tokens` gates it. Mirrors the case rule.
fn con_with_edits(src: &str, module: &Module) -> Vec<Edit> {
    let line_starts = line_start_table(src);
    let comments = comment_spans(src);

    // (record_span, base_end, first_field_start, last_field_end), outermost first.
    let mut recs: Vec<(Span, usize, usize, usize)> = Vec::new();
    walk_module_exprs(module, &mut |e| {
        if let Expr::Record {
            span, base, fields, ..
        } = e
        {
            // Construction only: base is a bare constructor.
            if !matches!(base.as_ref(), Expr::Con { .. }) {
                return;
            }
            if let (Some(first), Some(last)) = (fields.first(), fields.last()) {
                recs.push((*span, base.span().end, first.span.start, last.span.end));
            }
        }
    });
    recs.sort_by(|a, b| a.0.start.cmp(&b.0.start).then(b.0.end.cmp(&a.0.end)));

    let mut edits: Vec<Edit> = Vec::new();
    let mut accepted: Vec<Span> = Vec::new();
    for (rec_span, base_end, first_field, last_field_end) in recs {
        if accepted
            .iter()
            .any(|a| a.start <= rec_span.start && rec_span.end <= a.end && *a != rec_span)
        {
            continue;
        }
        accepted.push(rec_span);

        let rec_line = line_of(&line_starts, rec_span.start);
        let field_line = line_of(&line_starts, first_field);
        // Inline `Con with a = 1` (first field shares the line): leave verbatim.
        if field_line <= rec_line {
            continue;
        }
        // Only when `with` sits on the base (`Con`) line — then anchoring the
        // fields at base_line_indent + 2 lines them up under the construction.
        // A split `Con\n  with\n    fields` (with on its own line) would put the
        // fields left of `with`, so leave it verbatim.
        match find_keyword(src, base_end, first_field, "with", &comments) {
            Some(w) if line_of(&line_starts, w) == rec_line => {}
            _ => continue,
        }
        if leading_has_tab(src, line_starts[rec_line])
            || leading_has_tab(src, line_starts[field_line])
        {
            continue;
        }
        let rec_indent = indent_of(src, &line_starts, rec_line);
        let cur = indent_of(src, &line_starts, field_line);
        let delta = (rec_indent + INDENT) - cur;
        if delta != 0 {
            edits.push(Edit {
                child_start: line_starts[field_line],
                block_end: last_field_end,
                delta,
            });
        }
    }
    edits
}

fn reindent_con_with(src: &str, module: &Module) -> String {
    let edits = con_with_edits(src, module);
    if edits.is_empty() {
        return src.to_string();
    }
    apply_shifts(src, &edits)
}

/// Shift a SINGLE line to `target` indent (for a `with`/`where` keyword line).
fn push_line_edit(edits: &mut Vec<Edit>, ls: &[usize], src: &str, line: usize, target: i64) {
    if leading_has_tab(src, ls[line]) {
        return;
    }
    let delta = target - indent_of(src, ls, line);
    if delta != 0 {
        let end = *ls.get(line + 1).unwrap_or(&src.len());
        edits.push(Edit {
            child_start: ls[line],
            block_end: end,
            delta,
        });
    }
}

/// Shift a block `[first_byte .. end_byte)` to `target`, anchored on its first
/// line — but only when that first line is its own line (line-leading, and below
/// `head_line`), so an inline `with f : T` / `template X with` is left alone.
fn push_block_edit(
    edits: &mut Vec<Edit>,
    ls: &[usize],
    src: &str,
    first_byte: usize,
    end_byte: usize,
    target: i64,
    head_line: usize,
) {
    let first_line = line_of(ls, first_byte);
    if first_line <= head_line {
        return;
    }
    // The first element must start its line (nothing but spaces before it).
    if src[ls[first_line]..first_byte].chars().any(|c| c != ' ') {
        return;
    }
    if leading_has_tab(src, ls[first_line]) {
        return;
    }
    let delta = target - indent_of(src, ls, first_line);
    if delta != 0 {
        edits.push(Edit {
            child_start: ls[first_line],
            block_end: end_byte,
            delta,
        });
    }
}

/// Byte span of a template body declaration (the enum's variants each carry
/// their own span).
const fn body_decl_span(d: &TemplateBodyDecl) -> Span {
    match d {
        TemplateBodyDecl::Signatory { span, .. }
        | TemplateBodyDecl::Observer { span, .. }
        | TemplateBodyDecl::Ensure { span, .. }
        | TemplateBodyDecl::Key { span, .. }
        | TemplateBodyDecl::Maintainer { span, .. }
        | TemplateBodyDecl::Other { span, .. } => *span,
        TemplateBodyDecl::Choice(c) => c.span,
        TemplateBodyDecl::InterfaceInstance(i) => i.span,
    }
}

/// Structured `template` body reindent: the `with`/`where` keyword lines to
/// `template_indent + 2` and the field / signatory-choice-decl blocks to
/// `template_indent + 4`. Unlike a single uniform shift, the two DIFFERENT
/// deltas turn a 4-space ladder into the canonical 2-space one; choice bodies
/// (and any nested do-blocks) ride the decl-block shift and are then
/// canonicalized by the do-pass. `same_tokens` gates the whole candidate.
/// Reindent one keyword-introduced block (`with …` / `where …`): move the
/// keyword line to `kw_target` when it is on its OWN line (inline keywords like
/// `template X with` / `interface X where` stay put), and shift the block to
/// `body_target`. The two targets are passed in, not derived — a template is a
/// 2-level ladder (keywords at head + 2, contents at head + 4 even when the
/// keyword is inline, since the sibling block's keyword closes it), whereas an
/// interface's lone `where`-block sits at head + 2.
#[allow(clippy::too_many_arguments)]
fn reindent_keyword_block(
    edits: &mut Vec<Edit>,
    ls: &[usize],
    src: &str,
    comments: &[(usize, usize)],
    head_line: usize,
    kw_target: i64,
    body_target: i64,
    kw: &str,
    kw_from: usize,
    block_first: usize,
    block_last_end: usize,
) {
    if let Some(w) = find_keyword(src, kw_from, block_first, kw, comments) {
        if line_of(ls, w) > head_line {
            push_line_edit(edits, ls, src, line_of(ls, w), kw_target);
        }
    }
    push_block_edit(
        edits,
        ls,
        src,
        block_first,
        block_last_end,
        body_target,
        head_line,
    );
}

fn template_edits(src: &str, module: &Module) -> Vec<Edit> {
    let line_starts = line_start_table(src);
    let comments = comment_spans(src);
    let mut edits = Vec::new();
    for d in &module.decls {
        let head_byte = match d {
            Decl::Template(t) => t.span.start,
            Decl::Interface(i) => i.span.start,
            _ => continue,
        };
        let head_line = line_of(&line_starts, head_byte);
        if leading_has_tab(src, line_starts[head_line]) {
            continue;
        }
        let head_indent = indent_of(src, &line_starts, head_line);
        let kw_target = head_indent + INDENT;

        match d {
            Decl::Template(t) => {
                // A template is a 2-level ladder: with/where at head + 2, their
                // contents at head + 4 (even an inline `template X with`, since
                // the `where` keyword at + 2 must close the with-block).
                let body_target = head_indent + 2 * INDENT;
                // with-block: field block, anchored on the `with` keyword.
                if let (Some(f0), Some(fl)) = (t.fields.first(), t.fields.last()) {
                    reindent_keyword_block(
                        &mut edits,
                        &line_starts,
                        src,
                        &comments,
                        head_line,
                        kw_target,
                        body_target,
                        "with",
                        t.span.start,
                        f0.span.start,
                        fl.span.end,
                    );
                }
                // where-block: signatory/choice decls, anchored on `where`.
                if let (Some(b0), Some(bl)) = (t.body.first(), t.body.last()) {
                    let b0_start = body_decl_span(b0).start;
                    let bl_end = body_decl_span(bl).end;
                    let where_from = t.fields.last().map(|f| f.span.end).unwrap_or(t.span.start);
                    reindent_keyword_block(
                        &mut edits,
                        &line_starts,
                        src,
                        &comments,
                        head_line,
                        kw_target,
                        body_target,
                        "where",
                        where_from,
                        b0_start,
                        bl_end,
                    );
                }
            }
            Decl::Interface(i) => {
                // Interface body = viewtype + methods + choices. `viewtype`
                // carries no span and sits first, so anchor the block on the
                // first CODE LINE after the head (which includes it) rather than
                // on the first method/choice — otherwise the viewtype would be
                // left behind and break the offside (the gate would just reject).
                let mut last_end = None;
                for s in i
                    .methods
                    .iter()
                    .map(|m| m.span)
                    .chain(i.choices.iter().map(|c| c.span))
                {
                    last_end = Some(last_end.map_or(s.end, |e: usize| e.max(s.end)));
                }
                if let Some(last_end) = last_end {
                    if let Some(fbl) =
                        first_code_line_after(src, &line_starts, &comments, head_line, last_end)
                    {
                        // An interface's lone `where`-block (where is inline on
                        // the head line) sits at head + 2.
                        reindent_keyword_block(
                            &mut edits,
                            &line_starts,
                            src,
                            &comments,
                            head_line,
                            kw_target,
                            head_indent + INDENT,
                            "where",
                            i.span.start,
                            line_starts[fbl],
                            last_end,
                        );
                    }
                }
            }
            _ => {}
        }
    }
    edits
}

fn reindent_templates(src: &str, module: &Module) -> String {
    let edits = template_edits(src, module);
    if edits.is_empty() {
        return src.to_string();
    }
    apply_shifts(src, &edits)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn comment_line_detection() {
        let src = "x\n-- hi\ny\n";
        let comments = comment_spans(src);
        // "-- hi" starts at byte 2.
        assert!(is_comment_line(&comments, 2));
        assert!(!is_comment_line(&comments, 0)); // 'x'
        assert!(!is_comment_line(&comments, 7)); // 'y'
    }

    #[test]
    fn indent_and_line_helpers() {
        let src = "a\n    b\n";
        let ls = line_start_table(src);
        assert_eq!(indent_of(src, &ls, 0), 0);
        assert_eq!(indent_of(src, &ls, 1), 4);
        assert_eq!(line_of(&ls, 0), 0);
        assert_eq!(line_of(&ls, 6), 1);
    }

    #[test]
    fn do_body_reindented_to_anchor_plus_two() {
        // do at col 0; body stmt at col 6 -> should move to col 2.
        let src = "f = do\n      pure ()\n";
        let out = format_ast(src);
        assert_eq!(out, "f = do\n  pure ()\n");
    }

    #[test]
    fn idempotent_on_reindent() {
        let src = "f = do\n      pure ()\n";
        let once = format_ast(src);
        let twice = format_ast(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn leading_comment_not_measured_or_moved() {
        // The first body line is a col-0 comment; the real stmt is at col 6.
        // The comment must stay at col 0; the stmt moves to col 2.
        let src = "f = do\n-- note\n      pure ()\n";
        let out = format_ast(src);
        assert_eq!(out, "f = do\n-- note\n  pure ()\n");
        assert_eq!(format_ast(&out), out); // idempotent
    }

    #[test]
    fn inline_do_left_alone() {
        let src = "f = do pure ()\n";
        assert_eq!(format_ast(src), src);
    }

    #[test]
    fn tab_indented_body_left_verbatim() {
        // Tabs in indentation must never get spaces prepended in front of them.
        let src = "f = do\n\t\tpure ()\n";
        assert_eq!(format_ast(src), src);
        assert_eq!(format_ast(&format_ast(src)), format_ast(src));
    }

    #[test]
    fn do_block_starting_with_let_is_reindented() {
        // A `do` whose first statement is a `let` is no longer verbatim. The
        // whole block shifts by ONE uniform delta to land the first stmt at
        // do_col + 2, so the `let` line, its continuation binding, and the
        // following statement all move together — the bindings stay aligned
        // (x and y both end at col 6) without a separate let-internal rule.
        let src = "f = do\n      let x = 1\n          y = 2\n      pure (x + y)\n";
        let out = format_ast(src);
        assert_eq!(out, "f = do\n  let x = 1\n      y = 2\n  pure (x + y)\n");
        assert_eq!(format_ast(&out), out); // idempotent
    }

    #[test]
    fn do_block_with_try_stays_verbatim() {
        // try/catch handler layout is still deliberately left verbatim.
        let src = "f = do\n      _ <- try foo catch _ -> bar\n      pure ()\n";
        assert_eq!(format_ast(src), src);
    }

    #[test]
    fn if_then_else_reindented_to_if_col_plus_two() {
        // `if` at col 2; then/else lines move to col 4 (if_col + 2).
        let src = "f x =\n  if x > 0\n      then 1\n      else 2\n";
        let out = format_ast(src);
        assert_eq!(out, "f x =\n  if x > 0\n    then 1\n    else 2\n");
        assert_eq!(format_ast(&out), out); // idempotent
    }

    #[test]
    fn if_then_else_already_aligned_is_a_fixpoint() {
        let src = "f x =\n  if x > 0\n    then 1\n    else 2\n";
        assert_eq!(format_ast(src), src);
    }

    #[test]
    fn single_line_if_is_untouched() {
        // Inline then/else are not line-leading, so the rule leaves them alone.
        let src = "g x = if x then 1 else 2\n";
        assert_eq!(format_ast(src), src);
    }

    #[test]
    fn do_then_if_passes_reach_a_single_call_fixpoint() {
        // Regression: a do-block as the `then`-branch where `then`/`else` are at
        // different columns. In pass 1 the do-pass's body shift collides with
        // the not-yet-moved `else` (offside VSemi) so its gate rejects; the
        // if-pass then moves `else`, removing the collision. The structural
        // passes must iterate to a fixpoint so a SINGLE format call is already
        // idempotent — format(format(x)) == format(x).
        let src = "f =\n  if c\n    then do\n       a\n       b\n      else d\n";
        let once = format_ast(src);
        let twice = format_ast(&once);
        assert_eq!(once, twice, "single-call output must be a fixpoint");
    }

    #[test]
    fn if_then_else_multiline_branch_rides_uniform_shift() {
        // A then-branch spanning extra lines shifts by ONE uniform delta, so the
        // branch's own indentation structure is preserved (8->6, 10->8).
        let src = "f x =\n  if x > 0\n      then g\n             a\n      else h\n";
        let out = format_ast(src);
        assert_eq!(
            out,
            "f x =\n  if x > 0\n    then g\n           a\n    else h\n"
        );
        assert_eq!(format_ast(&out), out); // idempotent
    }

    #[test]
    fn case_alts_reindented_to_case_indent_plus_two() {
        // case-line indent 0; alts at col 6 move to col 2.
        let src = "f x = case x of\n      None -> 1\n      Some y -> y\n";
        let out = format_ast(src);
        assert_eq!(out, "f x = case x of\n  None -> 1\n  Some y -> y\n");
        assert_eq!(format_ast(&out), out); // idempotent
    }

    #[test]
    fn case_alts_already_aligned_is_a_fixpoint() {
        let src = "f x = case x of\n  None -> 1\n  Some y -> y\n";
        assert_eq!(format_ast(src), src);
    }

    #[test]
    fn inline_case_is_untouched() {
        // alts share the `case` line — left verbatim.
        let src = "f x = case x of None -> 1; Some y -> y\n";
        assert_eq!(format_ast(src), src);
    }

    #[test]
    fn nested_case_rides_outer_shift() {
        // Inner case (an alt body) rides the outer alt block's uniform shift; the
        // inner alts stay aligned relative to their own `case`.
        let src = "f x = case x of\n      A -> case y of\n             P -> 1\n             Q -> 2\n      B -> 0\n";
        let out = format_ast(src);
        // Outer alts to col 2; inner alts ride the same -4 shift (13 -> 9).
        let want =
            "f x = case x of\n  A -> case y of\n         P -> 1\n         Q -> 2\n  B -> 0\n";
        assert_eq!(out, want);
        assert_eq!(format_ast(&out), out); // idempotent
    }

    #[test]
    fn letin_bindings_reindented_to_let_indent_plus_two() {
        // `let` on its own line at col 2; bindings at col 6 move to col 4; `in`
        // is left at col 2.
        let src = "f =\n  let\n      x = 1\n      y = 2\n  in x + y\n";
        let out = format_ast(src);
        assert_eq!(out, "f =\n  let\n    x = 1\n    y = 2\n  in x + y\n");
        assert_eq!(format_ast(&out), out); // idempotent
    }

    #[test]
    fn letin_already_aligned_is_a_fixpoint() {
        let src = "f =\n  let\n    x = 1\n    y = 2\n  in x + y\n";
        assert_eq!(format_ast(src), src);
    }

    #[test]
    fn inline_letin_is_untouched() {
        // binding shares the `let` line — left verbatim.
        let src = "f = let x = 1 in x\n";
        assert_eq!(format_ast(src), src);
    }

    #[test]
    fn con_with_fields_reindented_to_indent_plus_two() {
        // `create Asset with` at line indent 0; fields at col 6 move to col 2.
        let src = "f = create Asset with\n      issuer = a\n      owner = b\n";
        let out = format_ast(src);
        assert_eq!(out, "f = create Asset with\n  issuer = a\n  owner = b\n");
        assert_eq!(format_ast(&out), out); // idempotent
    }

    #[test]
    fn record_update_stays_verbatim() {
        // base is an expression (`this`), not a bare constructor: an update,
        // which hangs-aligns inconsistently in the corpus — leave it alone.
        let src = "f this p = this with\n      owner = p\n";
        assert_eq!(format_ast(src), src);
    }

    #[test]
    fn split_with_on_own_line_stays_verbatim() {
        // `with` is on its own line, not the `Con` line: reindenting the fields
        // to the Con line's indent + 2 would put them left of `with`, so the
        // rule leaves it verbatim.
        let src = "f = WithField\n    with\n        f1 = 10\n";
        assert_eq!(format_ast(src), src);
    }

    #[test]
    fn inline_con_with_is_untouched() {
        let src = "f = Asset with issuer = a; owner = b\n";
        assert_eq!(format_ast(src), src);
    }

    #[test]
    fn template_four_space_ladder_canonicalized_to_two() {
        // The case the uniform shift could NOT fix: a 4-space ladder. The
        // structured reindent uses different deltas for keywords (-> +2) and
        // fields/decls (-> +4), so it becomes the canonical 2-space ladder, and
        // the choice's internal 2-space ladder rides the decl-block shift.
        let src = "template Coin\n    with\n        issuer : Party\n    where\n        signatory issuer\n        choice Burn : ()\n          controller issuer\n          do pure ()\n";
        let out = format_ast(src);
        let want = "template Coin\n  with\n    issuer: Party\n  where\n    signatory issuer\n    choice Burn: ()\n      controller issuer\n      do pure ()\n";
        assert_eq!(out, want);
        assert_eq!(format_ast(&out), out); // idempotent
    }

    #[test]
    fn interface_body_canonicalized_to_two() {
        // `interface X where` has `where` inline, so the body (viewtype +
        // methods + choices) sits at head + 2, and a choice's internals ride to
        // head + 4.
        let src = "interface Asset where\n    viewtype V\n    getOwner : Party\n    choice Xfer : ()\n      controller getOwner this\n      do pure ()\n";
        let out = format_ast(src);
        let want = "interface Asset where\n  viewtype V\n  getOwner: Party\n  choice Xfer: ()\n    controller getOwner this\n    do pure ()\n";
        assert_eq!(out, want);
        assert_eq!(format_ast(&out), out); // idempotent
    }

    #[test]
    fn inline_with_template_keeps_fields_at_head_plus_four() {
        // Regression: `template T with` (with inline on the head line) is still
        // a 2-level ladder — fields at head + 4, NOT head + 2, because the
        // `where` at + 2 must close the with-block. (Sending them to + 2 made
        // the SDK reject the output.)
        let src = "template T with\n    p: Party\n  where\n    signatory p\n";
        assert_eq!(format_ast(src), src);
    }

    #[test]
    fn canonical_template_is_a_fixpoint() {
        let src = "template Coin\n  with\n    issuer: Party\n  where\n    signatory issuer\n";
        assert_eq!(format_ast(src), src);
    }

    #[test]
    fn under_indented_template_body_canonicalized() {
        // where-decls at the `where` column (2) move to template_indent + 4 = 4.
        let src = "template Coin\n  with\n    issuer: Party\n  where\n  signatory issuer\n";
        let out = format_ast(src);
        assert_eq!(
            out,
            "template Coin\n  with\n    issuer: Party\n  where\n    signatory issuer\n"
        );
        assert_eq!(format_ast(&out), out); // idempotent
    }

    #[test]
    fn mid_line_let_is_left_verbatim() {
        // A `let` that does not start its line: the `in` stays at the keyword
        // column while the bindings would anchor on the (smaller) line indent,
        // which mismatches — so the rule leaves it alone rather than dedent the
        // bindings left of `let`/`in`.
        let src = "f x = let\n        a = 1\n        b = 2\n      in a + b\n";
        assert_eq!(format_ast(src), src);
    }

    #[test]
    fn duplicate_space_after_colon_collapsed() {
        // The formatter owns type-annotation colon spacing, so `x:  T` must
        // canonicalize to `x: T` symmetrically with `x : T` -> `x: T`.
        let src = "module M where\nfoo:  Int -> Int\nfoo x = x\n";
        let out = format_ast(src);
        assert_eq!(out, "module M where\nfoo: Int -> Int\nfoo x = x\n");
        assert_eq!(format_ast(&out), out); // idempotent
    }

    #[test]
    fn space_around_colon_canonicalized_both_sides() {
        let src = "module M where\nfoo  :  Int\nfoo = 1\n";
        assert_eq!(format_ast(src), "module M where\nfoo: Int\nfoo = 1\n");
    }

    #[test]
    fn after_colon_collapse_skips_braces_and_parens() {
        // At brace/paren depth the convention keeps the surrounding space, so
        // the after-colon collapse must NOT fire (same gate as before-colon).
        let braced = "module M where\nx = { a :  Int }\n";
        assert_eq!(format_ast(braced), braced);
        let parened = "module M where\nf (n :  Int) = n\n";
        assert_eq!(format_ast(parened), parened);
    }

    #[test]
    fn crlf_final_newline_not_mixed() {
        // A CRLF file must not end up with a lone LF on its last line.
        let src = "module M where\r\nx = 1   \r\n";
        let out = format_ast(src);
        assert!(out.ends_with("\r\n"), "got: {:?}", out);
        assert!(!out.ends_with("\n\n"));
        assert_eq!(format_ast(&out), out); // idempotent
    }
}
