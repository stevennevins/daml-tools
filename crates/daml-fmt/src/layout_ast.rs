//! AST-driven canonical layout — OUR OWN pattern, NOT a port of the LimeChain
//! heuristic (no LimeChain code is consulted; design is from the daml-parser AST.
//! We do NOT aim to match `expected/`: the formatter makes its own consistent
//! layout decisions and may diverge from the LimeChain baseline). This is the
//! shipping backend behind `format_source`.
//!
//! Uniform mechanism (the architecture the prototype validated):
//!   1. Walk the AST; every node carries a byte `Span`.
//!   2. For a layout block (currently `do`), reindent ONLY the block's child
//!      lines so the first real statement lands at `anchor_col + 2`. The anchor
//!      (`do`) line itself is never moved, which makes the rule a fixpoint:
//!      a second pass computes delta 0. Children shift by one uniform delta, so
//!      the block's internal structure (and any nested blocks) ride along.
//!   3. Comments are sacred: comment lines are never measured-from and never
//!      shifted (CLAUDE.md). Block-comment interiors are trivia, untouched.
//!   4. Gate the whole candidate on `same_tokens` = identical LAID-OUT token
//!      stream (offside virtuals included) ⇒ identical parse ⇒ identical
//!      desugar. Any accepted reindent is desugar-safe BY CONSTRUCTION.
//!   5. Fall back to the input (verbatim) when the gate rejects or a node is
//!      not modeled. Unmodeled constructs (case, with, where, guards, let-in,
//!      record updates, TypeDef, expression continuations) pass through
//!      verbatim — desugar-safe and lets us land one construct at a time.
//!
//! On top of the structural pass we compose the proven, token-gated
//! whitespace + colon-spacing normalization (`crate::normalize_gaps`).

use daml_parser::ast::*;
use daml_parser::layout::resolve_layout;
use daml_parser::lexer::{lex_with_trivia, TriviaKind};
use daml_parser::parse::parse_module;

const INDENT: i64 = 2;

/// Format with the AST-driven `do`-block rule, then the gap normalization;
/// every step is gated for desugar-safety.
pub fn format_ast(src: &str) -> String {
    let (module, _diags) = parse_module(src);

    // Step 1: structural reindent (do-blocks). Gate vs the input.
    let reindented = reindent_do_blocks(src, &module);
    let base = if reindented != src && same_tokens(src, &reindented) {
        reindented
    } else {
        src.to_string()
    };

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

/// Count of do-blocks reindented vs total do-blocks — the coverage metric
/// (`bin/coverage`). Reports how much of a file our rules canonically lay out.
pub fn coverage(src: &str) -> (usize, usize) {
    let (module, _diags) = parse_module(src);
    let mut dos: Vec<Span> = Vec::new();
    collect_do_spans(&module, &mut dos);
    let total = dos.len();
    let edits = do_block_edits(src, &module);
    (edits.len(), total)
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
/// - `do let ...` (first statement a `let` binding block): let layout unmodeled.
fn do_block_is_verbatim(do_span: Span, module: &Module) -> bool {
    let mut found = false;
    let mut verbatim = false;
    visit_do(module, &mut |span, stmts| {
        if span == do_span && !found {
            found = true;
            let first_is_let = matches!(stmts.first(), Some(DoStmt::Let { .. }));
            let has_try = stmts.iter().any(|s| match s {
                DoStmt::Bind { expr, .. } | DoStmt::Expr { expr, .. } => expr_contains_try(expr),
                DoStmt::Let { .. } => false,
            });
            verbatim = first_is_let || has_try;
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
