//! AST-driven canonical layout.
//!
//! This is an own-design canonical layout built from the daml-parser AST.
//! We do NOT aim to match an external formatter baseline: the formatter makes
//! its own consistent layout decisions. This is the shipping backend behind
//! `format_source`.
//!
//! Main mechanism:
//!   1. Walk the AST; every node carries a byte `Span`.
//!   2. For a layout block, reindent ONLY the block's child lines so the anchor
//!      construct lands at its canonical column. Current passes cover
//!      module/import continuations, choices, declarations, guards/where
//!      bindings, record updates, `try`/`catch`, explicit tuple/list
//!      continuations, `do`, `if`, `case`, `let-in`, constructor `with`, and
//!      template/interface bodies. The anchor line itself is never moved, which
//!      makes each rule a fixpoint: a second pass computes delta 0. Children
//!      shift by one uniform delta, so the block's internal structure (and any
//!      nested blocks) ride along. A `template`/`interface` body is the one
//!      STRUCTURED rule: a template's `with`/`where` keywords go to
//!      `head_col + 2` and the field / signatory-choice-decl blocks to
//!      `head_col + 4` (two different deltas), so a 4-space ladder collapses to
//!      the canonical 2-space one; an interface's inline-`where` body sits at
//!      `head_col + 2`.
//!   3. Comments are sacred: comment lines are never measured-from and never
//!      shifted (CLAUDE.md). Block-comment interiors are trivia, untouched.
//!   4. Gate pure reindent candidates on `same_tokens` = identical LAID-OUT
//!      token stream (offside virtuals included) ⇒ identical parse ⇒ identical
//!      desugar. Any accepted pure reindent is desugar-safe BY CONSTRUCTION.
//!   5. Fall back to the input (verbatim) when the gate rejects or a node is
//!      not modeled.
//!
//! On top of the structural pass we compose import organization and expression
//! layout rewrites that intentionally change layout form. Those rules are
//! covered by focused tests and the desugar/idempotence corpus verification.
//! Final whitespace + colon-spacing normalization remains token-gated
//! (`crate::normalize_gaps`).

use crate::ImportOrder;
use daml_parser::ast::*;
use daml_parser::lexer::TriviaKind;
use daml_syntax::{SourceFile, SourceTokens};

const INDENT: i64 = 2;
const INDENT_WIDTH: usize = 2;

/// Upper bound on structural-reindent iterations. The do-pass and if-pass can
/// unblock one another (the if-pass's `else` shift can remove a collision that
/// made the do-pass's gate reject), so they are iterated to a fixpoint for
/// single-call idempotence. Real inputs converge in 1-2; the cap only guards a
/// pathological non-convergence (the last output is still gate-safe).
const MAX_STRUCTURAL_PASSES: usize = 6;

/// Format with AST-driven structural reindents, layout-organizing rewrites, and
/// final token-gated gap normalization.
#[must_use]
pub fn format_ast(src: &str, options: crate::FormatOptions) -> String {
    if has_source_location_expectation(src) || has_trailing_with_comment(src) {
        return src.to_string();
    }

    // Step 1: structural reindent, each family as its own gated pass.
    let mut base = run_structural_passes(src);

    // Step 2: layout-organizing rewrites that intentionally change layout
    // tokens while preserving the non-layout token stream. Import organization
    // is controlled separately because it reorders import declarations.
    if options.import_order == ImportOrder::Organize {
        base = organize_imports(&base);
    }
    base = rewrite_layout_forms(&base);
    base = run_structural_passes(&base);

    // Step 3: whitespace + colon normalization on top, gated vs `base`.
    // same_tokens keeps this final spacing step from changing `base`'s parse.
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

fn run_structural_passes(src: &str) -> String {
    let mut base = src.to_string();
    for _ in 0..MAX_STRUCTURAL_PASSES {
        let mut next = base.clone();
        next = gated_module_pass(&next);
        next = gated_template_pass(&next);
        next = gated_choice_pass(&next);
        next = gated_type_def_pass(&next);
        next = gated_guard_pass(&next);
        next = gated_record_update_pass(&next);
        next = gated_try_pass(&next);
        next = gated_continuation_pass(&next);
        next = gated_do_pass(&next);
        next = gated_if_pass(&next);
        next = gated_case_pass(&next);
        next = gated_letin_pass(&next);
        next = gated_con_with_pass(&next);
        if next == base {
            break;
        }
        base = next;
    }
    base
}

/// Do-block reindent of `src`, accepted only if it passes the `same_tokens`
/// gate; otherwise `src` unchanged.
fn gated_do_pass(src: &str) -> String {
    let source_file = SourceFile::parse(src);
    let r = reindent_do_blocks(src, source_file.module());
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// if/then/else clause reindent of `src`, gated like the do-pass. Re-parses its
/// own input so spans match the (possibly already do-reindented) bytes.
fn gated_if_pass(src: &str) -> String {
    let source_file = SourceFile::parse(src);
    let r = reindent_ifs(src, source_file.module());
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// case-alternative reindent of `src`, gated like the do-pass.
fn gated_case_pass(src: &str) -> String {
    let source_file = SourceFile::parse(src);
    let r = reindent_cases(src, source_file.module());
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// `let … in` binding-block reindent of `src`, gated like the do-pass.
fn gated_letin_pass(src: &str) -> String {
    let source_file = SourceFile::parse(src);
    let r = reindent_letins(src, source_file.module());
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// `Con with` construction field-block reindent of `src`, gated like the do-pass.
fn gated_con_with_pass(src: &str) -> String {
    let source_file = SourceFile::parse(src);
    let r = reindent_con_with(src, source_file.module());
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// Structured template-body reindent of `src`, gated like the do-pass.
fn gated_template_pass(src: &str) -> String {
    let source_file = SourceFile::parse(src);
    let r = reindent_templates(src, source_file.module());
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// Module headers and import/export-list continuations.
fn gated_module_pass(src: &str) -> String {
    let source_file = SourceFile::parse(src);
    let r = reindent_modules_and_imports(src, source_file.module());
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// Choice signature/parameter/controller/observer/body ladders.
fn gated_choice_pass(src: &str) -> String {
    let source_file = SourceFile::parse(src);
    let r = reindent_choices(src, source_file.module());
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// Top-level `data`/`type`/`class`/`instance`/`exception` declaration ladders.
fn gated_type_def_pass(src: &str) -> String {
    let source_file = SourceFile::parse(src);
    let r = reindent_type_defs(src, source_file.module());
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// Guard bars and their bodies in function equations.
fn gated_guard_pass(src: &str) -> String {
    let source_file = SourceFile::parse(src);
    let r = reindent_guards(src, source_file.module());
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// Record UPDATE field blocks (`expr with`) distinct from constructor `Con with`.
fn gated_record_update_pass(src: &str) -> String {
    let source_file = SourceFile::parse(src);
    let r = reindent_record_updates(src, source_file.module());
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// `try`/`catch` body and handler ladders.
fn gated_try_pass(src: &str) -> String {
    let source_file = SourceFile::parse(src);
    let r = reindent_tries(src, source_file.module());
    if r != src && same_tokens(src, &r) {
        r
    } else {
        src.to_string()
    }
}

/// Existing multiline expression continuations inside explicit delimiters.
fn gated_continuation_pass(src: &str) -> String {
    let source_file = SourceFile::parse(src);
    let r = reindent_continuations(src, source_file.module());
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
#[must_use]
pub fn coverage(src: &str) -> crate::FormatCoverage {
    let source_file = SourceFile::parse(src);
    let module = source_file.module();
    let formatted = do_block_edits(src, module).len()
        + module_edits(src, module).len()
        + choice_edits(src, module).len()
        + type_def_edits(src, module).len()
        + guard_edits(src, module).len()
        + record_update_edits(src, module).len()
        + try_edits(src, module).len()
        + continuation_edits(src, module).len()
        + if_edits(src, module).len()
        + case_edits(src, module).len()
        + letin_edits(src, module).len()
        + con_with_edits(src, module).len()
        + template_edits(src, module).len();
    crate::FormatCoverage {
        formatted,
        total: modeled_construct_count(module),
    }
}

fn modeled_construct_count(module: &Module) -> usize {
    let mut count = 0usize;
    walk_module_expressions(module, &mut |e| match e {
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
            Decl::TypeDef { .. } => count += 1,
            _ => {}
        }
    }
    count += module.imports.iter().filter(|i| !i.span.is_empty()).count();
    count
}

/// True iff `a` and `b` share the same LAID-OUT token stream (offside virtuals
/// included) — the desugar-safety gate.
fn same_tokens(a: &str, b: &str) -> bool {
    let a = SourceTokens::lex(a);
    let b = SourceTokens::lex(b);
    let la = a.laid_out_tokens();
    let lb = b.laid_out_tokens();
    la.len() == lb.len() && la.iter().zip(lb).all(|(x, y)| x.kind() == y.kind())
}

fn has_source_location_expectation(src: &str) -> bool {
    src.lines()
        .filter(|line| line.trim_start().starts_with("-- @"))
        .any(|line| {
            line.contains("range=")
                || line.contains(".location")
                || line.contains("start_line")
                || line.contains("start_col")
                || line.contains("end_line")
                || line.contains("end_col")
        })
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Replacement {
    start: usize,
    end: usize,
    text: String,
}

fn apply_replacements(src: &str, replacements: &[Replacement]) -> String {
    if replacements.is_empty() {
        return src.to_string();
    }
    let mut ordered = replacements.to_vec();
    ordered.sort_by_key(|r| (r.start, r.end));
    let mut out = String::with_capacity(src.len());
    let mut cursor = 0usize;
    for r in ordered {
        if r.start < cursor || r.start > r.end || r.end > src.len() {
            return src.to_string();
        }
        out.push_str(&src[cursor..r.start]);
        out.push_str(&r.text);
        cursor = r.end;
    }
    out.push_str(&src[cursor..]);
    out
}

fn rewrite_layout_forms(src: &str) -> String {
    let mut base = rewrite_line_forms(src);
    base = rewrite_lambda_bodies(&base);
    base = rewrite_infix_continuations(&base);

    let source_file = SourceFile::parse(&base);
    let mut replacements = Vec::new();
    collect_inline_expression_rewrites(&base, source_file.module(), &mut replacements);
    apply_replacements(&base, &replacements)
}

fn rewrite_line_forms(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut last_expr_indent: Option<usize> = None;
    for line in src.split_inclusive('\n') {
        let (body, ending) = split_line_ending(line);
        let leading = body.len() - body.trim_start_matches(' ').len();
        let trimmed = body[leading..].trim_end();
        if trimmed.is_empty() || trimmed.starts_with("--") {
            out.push_str(line);
            continue;
        }
        if let Some(comment_at) = body.find("--") {
            if !body[..comment_at].trim().is_empty() {
                out.push_str(line);
                continue;
            }
        }

        if starts_with_infix_operator(trimmed) {
            if let Some(prev) = last_expr_indent {
                let target = prev.saturating_add(INDENT_WIDTH);
                if leading != target {
                    out.push_str(&" ".repeat(target));
                    out.push_str(trimmed);
                    out.push_str(ending);
                    continue;
                }
            }
        }

        if let Some(rewritten) = rewrite_signature_line(body, ending) {
            out.push_str(&rewritten);
            last_expr_indent = None;
            continue;
        }
        if let Some(rewritten) = rewrite_inline_let_line(body, ending) {
            out.push_str(&rewritten);
            last_expr_indent = None;
            continue;
        }
        if let Some(rewritten) = rewrite_long_application_line(body, ending) {
            out.push_str(&rewritten);
            last_expr_indent = None;
            continue;
        }

        out.push_str(line);
        if !starts_with_infix_operator(trimmed)
            && !starts_with_word(trimmed, "module")
            && !starts_with_word(trimmed, "import")
            && !trimmed.ends_with(':')
        {
            last_expr_indent = Some(leading);
        }
    }
    out
}

fn split_line_ending(line: &str) -> (&str, &str) {
    line.strip_suffix("\r\n").map_or_else(
        || {
            line.strip_suffix('\n')
                .map_or((line, ""), |body| (body, "\n"))
        },
        |body| (body, "\r\n"),
    )
}

fn rewrite_signature_line(body: &str, ending: &str) -> Option<String> {
    let leading = body.len() - body.trim_start_matches(' ').len();
    let trimmed = body[leading..].trim_end();
    let colon = trimmed.find(':')?;
    if trimmed[..colon].contains('=') {
        return None;
    }
    let name = trimmed[..colon].trim();
    if name.is_empty() || name.contains(' ') {
        return None;
    }
    let ty = trimmed[colon + 1..].trim();
    let arrow_count = ty.matches("->").count();
    if arrow_count < 3 && trimmed.chars().count() <= 80 {
        return None;
    }
    let indent = " ".repeat(leading);
    let parts = split_top_level_arrows(ty)?;
    if parts.len() < 2 {
        return None;
    }
    let mut out = format!("{indent}{name}:");
    for (idx, part) in parts.iter().enumerate() {
        out.push_str(ending);
        out.push_str(&indent);
        out.push_str("  ");
        if idx > 0 {
            out.push_str("-> ");
        }
        out.push_str(part);
    }
    out.push_str(ending);
    Some(out)
}

fn split_top_level_arrows(ty: &str) -> Option<Vec<&str>> {
    let bytes = ty.as_bytes();
    let mut parts = Vec::new();
    let mut start = 0usize;
    let mut depth = 0i32;
    let mut i = 0usize;
    while i + 1 < bytes.len() {
        match bytes[i] {
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => depth -= 1,
            b'-' if bytes[i + 1] == b'>' && depth == 0 => {
                let part = ty[start..i].trim();
                if part.is_empty() {
                    return None;
                }
                parts.push(part);
                i += 2;
                start = i;
                continue;
            }
            _ => {}
        }
        if depth < 0 {
            return None;
        }
        i += 1;
    }
    let part = ty[start..].trim();
    if part.is_empty() {
        return None;
    }
    parts.push(part);
    Some(parts)
}

fn rewrite_inline_let_line(body: &str, ending: &str) -> Option<String> {
    let leading = body.len() - body.trim_start_matches(' ').len();
    let trimmed = body[leading..].trim_end();
    let marker = " = let ";
    let let_at = trimmed.find(marker)? + " = ".len();
    let prefix = trimmed[..let_at].trim_end();
    let rest = &trimmed[let_at + "let ".len()..];
    let in_at = rest.rfind(" in ")?;
    let bindings = &rest[..in_at];
    let body_expr = rest[in_at + " in ".len()..].trim();
    if !bindings.contains(';') {
        return None;
    }
    let indent = " ".repeat(leading.saturating_add(INDENT_WIDTH));
    let nested = " ".repeat(leading.saturating_add(2 * INDENT_WIDTH));
    let mut out = format!("{}{}{}", " ".repeat(leading), prefix, ending);
    out.push_str(&indent);
    out.push_str("let");
    out.push_str(ending);
    for binding in bindings.split(';').map(str::trim).filter(|b| !b.is_empty()) {
        out.push_str(&nested);
        out.push_str(binding);
        out.push_str(ending);
    }
    out.push_str(&indent);
    out.push_str("in ");
    out.push_str(body_expr);
    out.push_str(ending);
    Some(out)
}

fn rewrite_long_application_line(body: &str, ending: &str) -> Option<String> {
    let leading = body.len() - body.trim_start_matches(' ').len();
    let trimmed = body[leading..].trim_end();
    let marker = " = ";
    let eq_at = trimmed.find(marker)?;
    let prefix = trimmed[..eq_at + marker.len() - 1].trim_end();
    let rhs = trimmed[eq_at + marker.len()..].trim();
    if rhs.contains('"')
        || rhs.chars().any(|c| {
            matches!(
                c,
                '+' | '*'
                    | '/'
                    | '<'
                    | '>'
                    | '='
                    | ';'
                    | '\\'
                    | '('
                    | ')'
                    | '['
                    | ']'
                    | '{'
                    | '}'
                    | ','
            )
        })
    {
        return None;
    }
    if rhs.chars().any(|c| matches!(c, '\''))
        && rhs.split_whitespace().any(|part| part.starts_with('\''))
    {
        return None;
    }
    let parts: Vec<_> = rhs.split_whitespace().collect();
    if parts.len() < 7 {
        return None;
    }
    if parts[0]
        .chars()
        .next()
        .is_some_and(|c| c.is_ascii_uppercase())
    {
        return None;
    }
    let indent = " ".repeat(leading.saturating_add(INDENT_WIDTH));
    let nested = " ".repeat(leading.saturating_add(2 * INDENT_WIDTH));
    let mut out = format!("{}{}{}", " ".repeat(leading), prefix, ending);
    out.push_str(&indent);
    out.push_str(parts[0]);
    out.push_str(ending);
    for part in parts.iter().skip(1) {
        out.push_str(&nested);
        out.push_str(part);
        out.push_str(ending);
    }
    Some(out)
}

fn collect_inline_expression_rewrites(
    src: &str,
    module: &Module,
    replacements: &mut Vec<Replacement>,
) {
    for decl in &module.decls {
        let Decl::Function(fun) = decl else {
            continue;
        };
        for eq in &fun.equations {
            let line_starts = line_start_table(src);
            let eq_line = line_of(&line_starts, eq.span.start);
            if leading_has_tab(src, line_starts[eq_line]) {
                continue;
            }
            let body_indent =
                indent_of_usize(src, &line_starts, eq_line).saturating_add(INDENT_WIDTH);
            collect_expr_rewrite(
                src,
                &eq.body,
                body_indent,
                RewriteLeadMode::LeadCandidate,
                replacements,
            );
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RewriteLeadMode {
    LeadCandidate,
    InlineOnly,
}

fn collect_expr_rewrite(
    src: &str,
    expr: &Expr,
    indent: usize,
    rewrite_mode: RewriteLeadMode,
    replacements: &mut Vec<Replacement>,
) {
    let span = expr.span();
    let line_starts = line_start_table(src);
    match expr {
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } if rewrite_mode == RewriteLeadMode::LeadCandidate
            && same_line_span(src, span)
            && inline_if_parts_are_simple(src, cond, then_branch, else_branch) =>
        {
            let ind = " ".repeat(indent);
            let nested = " ".repeat(indent.saturating_add(INDENT_WIDTH));
            let mut text = String::new();
            if rewrite_mode == RewriteLeadMode::LeadCandidate {
                text.push('\n');
                text.push_str(&ind);
            }
            text.push_str("if ");
            text.push_str(src[cond.span().start..cond.span().end].trim());
            text.push('\n');
            text.push_str(&nested);
            text.push_str("then ");
            text.push_str(src[then_branch.span().start..then_branch.span().end].trim());
            text.push('\n');
            text.push_str(&nested);
            text.push_str("else ");
            text.push_str(src[else_branch.span().start..else_branch.span().end].trim());
            replacements.push(Replacement {
                start: span.start,
                end: span.end,
                text,
            });
        }
        Expr::Case {
            scrutinee, alts, ..
        } if rewrite_mode == RewriteLeadMode::LeadCandidate
            && same_line_span(src, span)
            && !alts.is_empty() =>
        {
            let ind = " ".repeat(indent);
            let mut text = String::from("case ");
            text.push_str(src[scrutinee.span().start..scrutinee.span().end].trim());
            text.push_str(" of");
            for alt in alts {
                text.push('\n');
                text.push_str(&ind);
                text.push_str(src[alt.pat.span().start..alt.pat.span().end].trim());
                text.push_str(" -> ");
                text.push_str(src[alt.body.span().start..alt.body.span().end].trim());
            }
            replacements.push(Replacement {
                start: span.start,
                end: span.end,
                text,
            });
        }
        Expr::LetIn { bindings, body, .. }
            if rewrite_mode == RewriteLeadMode::LeadCandidate
                && same_line_span(src, span)
                && !bindings.is_empty() =>
        {
            let ind = " ".repeat(indent);
            let nested = " ".repeat(indent.saturating_add(INDENT_WIDTH));
            let mut text = String::new();
            if rewrite_mode == RewriteLeadMode::LeadCandidate {
                text.push('\n');
                text.push_str(&ind);
            }
            text.push_str("let");
            for binding in bindings {
                text.push('\n');
                text.push_str(&nested);
                text.push_str(src[binding.span.start..binding.span.end].trim());
            }
            text.push('\n');
            text.push_str(&ind);
            text.push_str("in ");
            text.push_str(src[body.span().start..body.span().end].trim());
            replacements.push(Replacement {
                start: span.start,
                end: span.end,
                text,
            });
        }
        Expr::Record { base, fields, .. }
            if rewrite_mode == RewriteLeadMode::LeadCandidate
                && same_line_span(src, span)
                && src[span.start..span.end].contains(';')
                && matches!(base.as_ref(), Expr::Con { .. })
                && fields.len() > 1 =>
        {
            let ind = " ".repeat(indent);
            let mut text = String::new();
            text.push_str(src[base.span().start..base.span().end].trim());
            text.push_str(" with");
            for field in fields {
                text.push('\n');
                text.push_str(&ind);
                text.push_str(src[field.span.start..field.span.end].trim());
            }
            replacements.push(Replacement {
                start: span.start,
                end: span.end,
                text,
            });
        }
        Expr::App { func, args, .. }
            if rewrite_mode == RewriteLeadMode::LeadCandidate
                && same_line_span(src, span)
                && args.len() >= 6
                && root_app_func(func).is_some()
                && app_args_are_simple(src, args) =>
        {
            let ind = " ".repeat(indent);
            let nested = " ".repeat(indent.saturating_add(INDENT_WIDTH));
            let mut text = String::new();
            if rewrite_mode == RewriteLeadMode::LeadCandidate {
                text.push('\n');
                text.push_str(&ind);
            }
            text.push_str(src[func.span().start..func.span().end].trim());
            for arg in args {
                text.push('\n');
                text.push_str(&nested);
                text.push_str(src[arg.span().start..arg.span().end].trim());
            }
            replacements.push(Replacement {
                start: span.start,
                end: span.end,
                text,
            });
        }
        _ => {
            let expr_line = line_of(&line_starts, span.start);
            let child_indent = if expr_line < line_starts.len() {
                indent_of_usize(src, &line_starts, expr_line).saturating_add(INDENT_WIDTH)
            } else {
                indent.saturating_add(INDENT_WIDTH)
            };
            match expr {
                Expr::App { func, args, .. } => {
                    collect_expr_rewrite(
                        src,
                        func,
                        child_indent,
                        RewriteLeadMode::InlineOnly,
                        replacements,
                    );
                    for arg in args {
                        collect_expr_rewrite(
                            src,
                            arg,
                            child_indent,
                            RewriteLeadMode::InlineOnly,
                            replacements,
                        );
                    }
                }
                Expr::BinOp { lhs, rhs, .. } => {
                    collect_expr_rewrite(
                        src,
                        lhs,
                        child_indent,
                        RewriteLeadMode::InlineOnly,
                        replacements,
                    );
                    collect_expr_rewrite(
                        src,
                        rhs,
                        child_indent,
                        RewriteLeadMode::InlineOnly,
                        replacements,
                    );
                }
                Expr::If {
                    cond,
                    then_branch,
                    else_branch,
                    ..
                } => {
                    collect_expr_rewrite(
                        src,
                        cond,
                        child_indent,
                        RewriteLeadMode::InlineOnly,
                        replacements,
                    );
                    collect_expr_rewrite(
                        src,
                        then_branch,
                        child_indent,
                        RewriteLeadMode::InlineOnly,
                        replacements,
                    );
                    collect_expr_rewrite(
                        src,
                        else_branch,
                        child_indent,
                        RewriteLeadMode::InlineOnly,
                        replacements,
                    );
                }
                Expr::Case {
                    scrutinee, alts, ..
                } => {
                    collect_expr_rewrite(
                        src,
                        scrutinee,
                        child_indent,
                        RewriteLeadMode::InlineOnly,
                        replacements,
                    );
                    for alt in alts {
                        collect_expr_rewrite(
                            src,
                            &alt.body,
                            child_indent,
                            RewriteLeadMode::InlineOnly,
                            replacements,
                        );
                    }
                }
                Expr::LetIn { bindings, body, .. } => {
                    for binding in bindings {
                        collect_expr_rewrite(
                            src,
                            &binding.expr,
                            child_indent,
                            RewriteLeadMode::InlineOnly,
                            replacements,
                        );
                    }
                    collect_expr_rewrite(
                        src,
                        body,
                        child_indent,
                        RewriteLeadMode::InlineOnly,
                        replacements,
                    );
                }
                Expr::Record { base, fields, .. } => {
                    collect_expr_rewrite(
                        src,
                        base,
                        child_indent,
                        RewriteLeadMode::InlineOnly,
                        replacements,
                    );
                    for field in fields {
                        if let Some(value) = &field.value {
                            collect_expr_rewrite(
                                src,
                                value,
                                child_indent,
                                RewriteLeadMode::InlineOnly,
                                replacements,
                            );
                        }
                    }
                }
                Expr::Lambda { body, .. } | Expr::Neg { expr: body, .. } => {
                    collect_expr_rewrite(
                        src,
                        body,
                        child_indent,
                        RewriteLeadMode::InlineOnly,
                        replacements,
                    );
                }
                _ => {}
            }
        }
    }
}

fn root_app_func(expr: &Expr) -> Option<()> {
    matches!(expr, Expr::Var { .. }).then_some(())
}

fn inline_if_parts_are_simple(
    src: &str,
    cond: &Expr,
    then_branch: &Expr,
    else_branch: &Expr,
) -> bool {
    [cond.span(), then_branch.span(), else_branch.span()]
        .into_iter()
        .map(|span| src[span.start..span.end].trim())
        .all(is_simple_inline_piece)
}

fn is_simple_inline_piece(text: &str) -> bool {
    !text.is_empty()
        && !text.contains('\n')
        && !text
            .chars()
            .any(|c| matches!(c, '(' | ')' | '{' | '}' | '[' | ']' | ';' | ','))
}

fn app_args_are_simple(src: &str, args: &[Expr]) -> bool {
    args.iter()
        .map(|arg| src[arg.span().start..arg.span().end].trim())
        .all(is_simple_app_arg)
}

fn is_simple_app_arg(text: &str) -> bool {
    !text.is_empty()
        && !text.contains('\n')
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '\'' | '.' | '-'))
}

fn same_line_span(src: &str, span: Span) -> bool {
    !src[span.start..span.end].contains('\n')
}

fn has_trailing_with_comment(src: &str) -> bool {
    src.lines().any(|line| {
        let Some(comment_at) = line.find("--") else {
            return false;
        };
        line[..comment_at]
            .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '\''))
            .any(|word| word == "with")
    })
}

fn organize_imports(src: &str) -> String {
    let source_file = SourceFile::parse(src);
    let module = source_file.module();
    if module.imports.len() < 2 {
        return src.to_string();
    }
    let Some(first) = module.imports.first() else {
        return src.to_string();
    };
    let Some(last) = module.imports.last() else {
        return src.to_string();
    };

    let line_starts = line_start_table(src);
    let start_line = line_of(&line_starts, first.span.start);
    let end_line = line_of(&line_starts, last.span.end.saturating_sub(1));
    let block_start = line_starts[start_line];
    let block_end = *line_starts.get(end_line + 1).unwrap_or(&src.len());
    if src[block_start..block_end].contains("--")
        || src[block_start..block_end].contains("{-")
        || src[block_start..block_end]
            .lines()
            .any(|line| line.trim_start().starts_with('#'))
    {
        return src.to_string();
    }

    let mut imports: Vec<_> = module
        .imports
        .iter()
        .map(|imp| {
            (
                import_group(&imp.module_name),
                imp.module_name.clone(),
                src[imp.span.start..imp.span.end].trim().to_string(),
            )
        })
        .collect();
    imports.sort_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)));

    let original: Vec<_> = module
        .imports
        .iter()
        .map(|imp| src[imp.span.start..imp.span.end].trim().to_string())
        .collect();
    let sorted: Vec<_> = imports.iter().map(|(_, _, text)| text.clone()).collect();
    if original == sorted {
        return src.to_string();
    }

    let mut text = String::new();
    let mut prev_group = None;
    for (group, _, import) in imports {
        if prev_group.is_some_and(|g| g != group) {
            text.push('\n');
        }
        text.push_str(&import);
        text.push('\n');
        prev_group = Some(group);
    }

    let mut out = String::with_capacity(src.len());
    out.push_str(&src[..block_start]);
    out.push_str(&text);
    out.push_str(&src[block_end..]);
    out
}

fn import_group(module_name: &str) -> u8 {
    if module_name.starts_with("Daml.") {
        0
    } else if module_name.starts_with("DA.") {
        1
    } else {
        2
    }
}

fn rewrite_lambda_bodies(src: &str) -> String {
    let source_file = SourceFile::parse(src);
    let module = source_file.module();
    let line_starts = line_start_table(src);
    let mut edits = Vec::new();
    walk_module_expressions(module, &mut |expr| {
        let Expr::Lambda { span, body, .. } = expr else {
            return;
        };
        let lambda_line = line_of(&line_starts, span.start);
        let body_line = line_of(&line_starts, body.span().start);
        if body_line <= lambda_line || leading_has_tab(src, line_starts[body_line]) {
            return;
        }
        let target = indent_of(src, &line_starts, lambda_line) + INDENT;
        let delta = target - indent_of(src, &line_starts, body_line);
        if delta != 0 {
            edits.push(Edit {
                child_start: line_starts[body_line],
                block_end: body.span().end,
                delta,
            });
        }
    });
    if edits.is_empty() {
        return src.to_string();
    }
    let shifted = apply_shifts(src, &edits);
    if same_tokens(src, &shifted) {
        shifted
    } else {
        src.to_string()
    }
}

fn rewrite_infix_continuations(src: &str) -> String {
    let source_file = SourceFile::parse(src);
    let module = source_file.module();
    let line_starts = line_start_table(src);
    let comments = comment_spans(src);
    let mut edits = Vec::new();
    for decl in &module.decls {
        let Decl::Function(fun) = decl else {
            continue;
        };
        for eq in &fun.equations {
            let body_span = eq.body.span();
            let first_line = line_of(&line_starts, body_span.start);
            let target = indent_of(src, &line_starts, first_line) + INDENT;
            let mut line = first_line + 1;
            while line < line_starts.len() && line_starts[line] < body_span.end {
                let Some(trimmed) = code_line_trimmed(src, &line_starts, &comments, line) else {
                    line += 1;
                    continue;
                };
                if starts_with_infix_operator(trimmed) {
                    push_code_line_edit(&mut edits, src, &line_starts, &comments, line, target);
                }
                line += 1;
            }
        }
    }
    if edits.is_empty() {
        return src.to_string();
    }
    let shifted = apply_shifts(src, &edits);
    if same_tokens(src, &shifted) {
        shifted
    } else {
        src.to_string()
    }
}

fn starts_with_infix_operator(trimmed: &str) -> bool {
    if trimmed.starts_with("->")
        || trimmed == ":"
        || trimmed
            .strip_prefix('|')
            .is_some_and(|rest| rest.is_empty() || rest.starts_with(char::is_whitespace))
    {
        return false;
    }
    trimmed
        .chars()
        .next()
        .is_some_and(|c| matches!(c, '&' | '|' | '+' | '-' | '*' | '/' | '<' | '>' | '=' | ':'))
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
    let mut do_block_spans: Vec<Span> = Vec::new();
    collect_do_block_spans(module, &mut do_block_spans);
    // Outermost first (smaller start, then larger end).
    do_block_spans.sort_by(|a, b| a.start.cmp(&b.start).then(b.end.cmp(&a.end)));

    let line_starts = line_start_table(src);
    let comments = comment_spans(src);

    let mut edits: Vec<Edit> = Vec::new();
    let mut accepted: Vec<Span> = Vec::new();
    for do_span in do_block_spans {
        // Skip a do-block nested in one we already accepted (it rides along).
        if accepted
            .iter()
            .any(|a| a.start <= do_span.start && do_span.end <= a.end && *a != do_span)
        {
            continue;
        }
        let do_line = line_of(&line_starts, do_span.start);
        let do_indent = indent_of(src, &line_starts, do_line);
        // First real (non-blank, non-comment) statement line after the do line.
        let Some(first_stmt_line) =
            first_code_line_after(src, &line_starts, &comments, do_line, do_span.end)
        else {
            continue; // inline `do stmt` — nothing on its own line; leave it
        };
        accepted.push(do_span);
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
                block_end: do_span.end,
                delta,
            });
        }
    }
    edits
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
        let new = add_signed_to_usize_saturating(cur, delta);
        out.push_str(&" ".repeat(new));
        out.push_str(&line[cur..]);
    }
    out
}

// ---- comment-line awareness ------------------------------------------------

/// Byte spans of every comment (line + block); sorted by start.
fn comment_spans(src: &str) -> Vec<(usize, usize)> {
    let source_tokens = SourceTokens::lex(src);
    let mut v: Vec<(usize, usize)> = source_tokens
        .trivia()
        .iter()
        .filter(|t| matches!(t.kind(), TriviaKind::LineComment | TriviaKind::BlockComment))
        .map(|t| (t.start(), t.end()))
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
fn indent_of_usize(src: &str, line_starts: &[usize], line: usize) -> usize {
    src[line_starts[line]..]
        .chars()
        .take_while(|&c| c == ' ')
        .count()
}
fn indent_of(src: &str, line_starts: &[usize], line: usize) -> i64 {
    usize_to_i64_saturating(indent_of_usize(src, line_starts, line))
}
fn usize_to_i64_saturating(value: usize) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}
fn add_signed_to_usize_saturating(value: usize, delta: i64) -> usize {
    if delta >= 0 {
        value.saturating_add(usize::try_from(delta).unwrap_or(usize::MAX))
    } else {
        value.saturating_sub(usize::try_from(delta.unsigned_abs()).unwrap_or(usize::MAX))
    }
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

fn next_code_line_starts_with_keyword(
    src: &str,
    line_starts: &[usize],
    comments: &[(usize, usize)],
    after_line: usize,
    keyword: &str,
) -> Option<usize> {
    let mut l = after_line + 1;
    while l < line_starts.len() {
        let ls = line_starts[l];
        let le = *line_starts.get(l + 1).unwrap_or(&src.len());
        let line = &src[ls..le];
        let cur = line.len() - line.trim_start_matches(' ').len();
        let trimmed = line[cur..].trim_end();
        if line.trim().is_empty() || is_comment_line(comments, ls + cur) {
            l += 1;
            continue;
        }
        return trimmed
            .strip_prefix(keyword)
            .is_some_and(|rest| {
                rest.is_empty()
                    || rest
                        .chars()
                        .next()
                        .is_some_and(|c| !c.is_ascii_alphanumeric() && c != '_')
            })
            .then_some(l);
    }
    None
}

// ---- AST walks -------------------------------------------------------------

fn collect_do_block_spans(module: &Module, do_block_spans: &mut Vec<Span>) {
    walk_module_expressions(module, &mut |expr| {
        if let Expr::Do { span, .. } = expr {
            do_block_spans.push(*span);
        }
    });
}

/// Visit every expression in the module, pre-order. The generic walker behind
/// construct-specific rules (do, if/then/else, ...).
fn walk_module_expressions(module: &Module, f: &mut impl FnMut(&Expr)) {
    for decl in &module.decls {
        match decl {
            Decl::Function(fun) => {
                for eq in &fun.equations {
                    walk_expression(&eq.body, f);
                    for (g, b) in &eq.guards {
                        walk_expression(g, f);
                        walk_expression(b, f);
                    }
                    for wb in &eq.where_bindings {
                        walk_expression(&wb.expr, f);
                    }
                }
            }
            Decl::Template(t) => {
                for b in &t.body {
                    match b {
                        TemplateBodyDecl::Choice(c) => {
                            if let Some(body) = &c.body {
                                walk_expression(body, f);
                            }
                        }
                        TemplateBodyDecl::Ensure { expr, .. }
                        | TemplateBodyDecl::Key { expr, .. }
                        | TemplateBodyDecl::Maintainer { expr, .. } => walk_expression(expr, f),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
}

fn walk_expression(expr: &Expr, f: &mut impl FnMut(&Expr)) {
    f(expr);
    match expr {
        Expr::App { func, args, .. } => {
            walk_expression(func, f);
            args.iter().for_each(|arg| walk_expression(arg, f));
        }
        Expr::BinOp { lhs, rhs, .. } => {
            walk_expression(lhs, f);
            walk_expression(rhs, f);
        }
        Expr::Neg { expr, .. } | Expr::Lambda { body: expr, .. } => walk_expression(expr, f),
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            walk_expression(cond, f);
            walk_expression(then_branch, f);
            walk_expression(else_branch, f);
        }
        Expr::Case {
            scrutinee, alts, ..
        } => {
            walk_expression(scrutinee, f);
            alts.iter().for_each(|alt| walk_expression(&alt.body, f));
        }
        Expr::Do { stmts, .. } => {
            for s in stmts {
                match s {
                    DoStmt::Bind { expr, .. } | DoStmt::Expr { expr, .. } => {
                        walk_expression(expr, f)
                    }
                    DoStmt::Let { bindings, .. } => {
                        bindings.iter().for_each(|b| walk_expression(&b.expr, f))
                    }
                    _ => {}
                }
            }
        }
        Expr::LetIn { bindings, body, .. } => {
            bindings.iter().for_each(|b| walk_expression(&b.expr, f));
            walk_expression(body, f);
        }
        Expr::Record { base, fields, .. } => {
            walk_expression(base, f);
            for fa in fields {
                if let Some(v) = &fa.value {
                    walk_expression(v, f);
                }
            }
        }
        Expr::Tuple { items, .. } | Expr::List { items, .. } => {
            items.iter().for_each(|item| walk_expression(item, f))
        }
        Expr::Try { body, handlers, .. } => {
            walk_expression(body, f);
            handlers
                .iter()
                .for_each(|handler| walk_expression(&handler.body, f));
        }
        Expr::Section {
            operand: Some(o), ..
        } => walk_expression(o, f),
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
        let before_ok = at == 0 || !is_ident_byte(bytes[at - 1]);
        let after = at + kw.len();
        let after_ok = after >= hay.len() || !is_ident_byte(bytes[after]);
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
    walk_module_expressions(module, &mut |e| {
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
        let if_col = usize_to_i64_saturating(src[line_starts[if_line]..if_byte].chars().count());
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
    walk_module_expressions(module, &mut |e| {
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
    walk_module_expressions(module, &mut |e| {
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
/// constructor `Con`) are touched here; record UPDATES (`expr with …`) are handled
/// by their own gated pass. Inline and tab-indented blocks stay verbatim. The
/// field block shifts by ONE uniform delta (so nested values ride along) and
/// `same_tokens` gates it. Mirrors the case rule.
fn con_with_edits(src: &str, module: &Module) -> Vec<Edit> {
    let line_starts = line_start_table(src);
    let comments = comment_spans(src);

    // (record_span, base_end, first_field_start, last_field_end), outermost first.
    let mut recs: Vec<(Span, usize, usize, usize)> = Vec::new();
    walk_module_expressions(module, &mut |e| {
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
        let target = rec_indent + INDENT;
        if next_code_line_starts_with_keyword(
            src,
            &line_starts,
            &comments,
            line_of(&line_starts, last_field_end.saturating_sub(1)),
            "where",
        )
        .is_some_and(|next_line| indent_of(src, &line_starts, next_line) <= target)
        {
            continue;
        }
        let delta = target - cur;
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

// ---- module / import continuations ------------------------------------------

fn module_edits(src: &str, module: &Module) -> Vec<Edit> {
    let line_starts = line_start_table(src);
    let comments = comment_spans(src);
    let mut edits = Vec::new();

    if !module.header.is_empty() {
        push_continuation_lines(
            &mut edits,
            src,
            &line_starts,
            &comments,
            module.header,
            INDENT,
        );
    }
    for imp in &module.imports {
        push_continuation_lines(&mut edits, src, &line_starts, &comments, imp.span, INDENT);
    }
    edits
}

fn reindent_modules_and_imports(src: &str, module: &Module) -> String {
    let edits = module_edits(src, module);
    if edits.is_empty() {
        return src.to_string();
    }
    apply_shifts(src, &edits)
}

fn push_continuation_lines(
    edits: &mut Vec<Edit>,
    src: &str,
    ls: &[usize],
    comments: &[(usize, usize)],
    span: Span,
    offset: i64,
) {
    let head_line = line_of(ls, span.start);
    let target = indent_of(src, ls, head_line) + offset;
    let mut line = head_line + 1;
    while line < ls.len() && ls[line] < span.end {
        push_code_line_edit(edits, src, ls, comments, line, target);
        line += 1;
    }
}

// ---- choice internals --------------------------------------------------------

fn collect_choices<'a>(module: &'a Module, choices: &mut Vec<&'a ChoiceDecl>) {
    for decl in &module.decls {
        match decl {
            Decl::Template(t) => {
                for body in &t.body {
                    if let TemplateBodyDecl::Choice(c) = body {
                        choices.push(c);
                    }
                }
            }
            Decl::Interface(i) => choices.extend(i.choices.iter()),
            _ => {}
        }
    }
    choices.sort_by(|a, b| {
        a.span
            .start
            .cmp(&b.span.start)
            .then(b.span.end.cmp(&a.span.end))
    });
}

fn choice_edits(src: &str, module: &Module) -> Vec<Edit> {
    let line_starts = line_start_table(src);
    let comments = comment_spans(src);
    let mut choices = Vec::new();
    collect_choices(module, &mut choices);

    let mut edits = Vec::new();
    for c in choices {
        let choice_line = line_of(&line_starts, c.span.start);
        if leading_has_tab(src, line_starts[choice_line]) {
            continue;
        }
        let choice_indent = indent_of(src, &line_starts, choice_line);
        let clause_target = choice_indent + INDENT;
        let nested_target = choice_indent + 2 * INDENT;

        if let Some(ty) = &c.return_ty {
            if let Some(colon) = find_symbol(src, c.span.start, ty.span().start, ":", &comments) {
                push_span_block_edit(
                    &mut edits,
                    &line_starts,
                    src,
                    colon,
                    ty.span().end,
                    clause_target,
                );
            }
        }

        if let (Some(first), Some(last)) = (c.params.first(), c.params.last()) {
            let kw_from = c.return_ty.as_ref().map_or(c.span.start, |t| t.span().end);
            if let Some(w) = find_keyword(src, kw_from, first.span.start, "with", &comments) {
                let with_line = line_of(&line_starts, w);
                let first_param_line = line_of(&line_starts, first.span.start);
                if first_param_line == with_line {
                    push_span_block_edit(
                        &mut edits,
                        &line_starts,
                        src,
                        w,
                        last.span.end,
                        clause_target,
                    );
                } else {
                    push_line_edit(&mut edits, &line_starts, src, with_line, clause_target);
                    push_block_edit(
                        &mut edits,
                        &line_starts,
                        src,
                        first.span.start,
                        last.span.end,
                        nested_target,
                        choice_line,
                    );
                }
            }
        }

        if let (Some(first), Some(last)) = (c.observers.first(), c.observers.last()) {
            if let Some(k) =
                find_keyword(src, c.span.start, first.span().start, "observer", &comments)
            {
                push_span_block_edit(
                    &mut edits,
                    &line_starts,
                    src,
                    k,
                    last.span().end,
                    clause_target,
                );
            }
        }
        if let (Some(first), Some(last)) = (c.controllers.first(), c.controllers.last()) {
            if let Some(k) = find_keyword(
                src,
                c.span.start,
                first.span().start,
                "controller",
                &comments,
            ) {
                push_span_block_edit(
                    &mut edits,
                    &line_starts,
                    src,
                    k,
                    last.span().end,
                    clause_target,
                );
            }
        }

        if let Some(body) = &c.body {
            push_span_block_edit(
                &mut edits,
                &line_starts,
                src,
                body.span().start,
                body.span().end,
                clause_target,
            );
        }
    }
    edits
}

fn reindent_choices(src: &str, module: &Module) -> String {
    let edits = choice_edits(src, module);
    if edits.is_empty() {
        return src.to_string();
    }
    apply_shifts(src, &edits)
}

// ---- top-level type/data/class/instance/exception declarations ---------------

fn type_def_edits(src: &str, module: &Module) -> Vec<Edit> {
    let line_starts = line_start_table(src);
    let comments = comment_spans(src);
    let mut edits = Vec::new();
    for decl in &module.decls {
        let Decl::TypeDef { span, .. } = decl else {
            continue;
        };
        let head_line = line_of(&line_starts, span.start);
        if leading_has_tab(src, line_starts[head_line]) {
            continue;
        }
        let head_indent = indent_of(src, &line_starts, head_line);
        let head_has_with = line_contains_word(src, &line_starts, head_line, "with");
        let mut in_with = head_has_with;
        let mut with_body_target = if head_has_with {
            first_body_anchor_indent_after(src, &line_starts, &comments, head_line, span.end)
                .unwrap_or(head_indent + INDENT)
        } else {
            head_indent + 2 * INDENT
        };
        let head_has_where = line_contains_word(src, &line_starts, head_line, "where");
        let mut in_where = head_has_where;
        let mut where_body_target = if head_has_where {
            first_body_anchor_indent_after(src, &line_starts, &comments, head_line, span.end)
                .unwrap_or(head_indent + INDENT)
        } else {
            head_indent + INDENT
        };
        let mut after_variant = line_contains_symbol(src, &line_starts, head_line, "=");
        let mut after_bar_variant = false;

        let mut line = head_line + 1;
        while line < line_starts.len() && line_starts[line] < span.end {
            let Some(trimmed) = code_line_trimmed(src, &line_starts, &comments, line) else {
                line += 1;
                continue;
            };
            let target = if starts_with_word(trimmed, "where") {
                in_with = false;
                in_where = true;
                where_body_target = head_indent + 2 * INDENT;
                after_variant = false;
                Some(head_indent + INDENT)
            } else if starts_with_word(trimmed, "with") {
                let target = if after_bar_variant {
                    head_indent + 2 * INDENT
                } else {
                    head_indent + INDENT
                };
                in_with = true;
                in_where = false;
                with_body_target = target + INDENT;
                after_variant = false;
                after_bar_variant = false;
                Some(target)
            } else if trimmed.starts_with('=') || trimmed.starts_with('|') {
                in_with = false;
                in_where = false;
                after_variant = true;
                after_bar_variant = trimmed.starts_with('|');
                Some(head_indent + INDENT)
            } else if starts_with_word(trimmed, "deriving") {
                in_with = false;
                in_where = false;
                after_variant = false;
                after_bar_variant = false;
                Some(head_indent + INDENT)
            } else if in_with {
                Some(with_body_target)
            } else if in_where {
                Some(where_body_target)
            } else if after_variant {
                Some(head_indent + INDENT)
            } else {
                None
            };
            if let Some(target) = target {
                push_code_line_edit(&mut edits, src, &line_starts, &comments, line, target);
            }
            line += 1;
        }
    }
    edits
}

fn first_body_anchor_indent_after(
    src: &str,
    ls: &[usize],
    comments: &[(usize, usize)],
    after_line: usize,
    block_end: usize,
) -> Option<i64> {
    let mut line = after_line + 1;
    while line < ls.len() && ls[line] < block_end {
        if leading_has_tab(src, ls[line]) {
            return None;
        }
        let end = *ls.get(line + 1).unwrap_or(&src.len());
        let text = &src[ls[line]..end];
        let cur = text.len() - text.trim_start_matches(' ').len();
        let trimmed = text[cur..].trim_end();
        if trimmed.is_empty() {
            line += 1;
            continue;
        }
        if is_comment_line(comments, ls[line] + cur) && !trimmed.starts_with("{-#") {
            line += 1;
            continue;
        }
        return Some(indent_of(src, ls, line));
    }
    None
}

fn reindent_type_defs(src: &str, module: &Module) -> String {
    let edits = type_def_edits(src, module);
    if edits.is_empty() {
        return src.to_string();
    }
    apply_shifts(src, &edits)
}

// ---- function guards and where-bindings --------------------------------------

fn guard_edits(src: &str, module: &Module) -> Vec<Edit> {
    let line_starts = line_start_table(src);
    let comments = comment_spans(src);
    let mut edits = Vec::new();
    for decl in &module.decls {
        let Decl::Function(fun) = decl else {
            continue;
        };
        for eq in &fun.equations {
            let eq_line = line_of(&line_starts, eq.span.start);
            if leading_has_tab(src, line_starts[eq_line]) {
                continue;
            }
            let guard_target = indent_of(src, &line_starts, eq_line) + INDENT;
            let mut cursor = eq.span.start;
            for (guard, body) in &eq.guards {
                if let Some(pipe) = find_symbol(src, cursor, guard.span().start, "|", &comments) {
                    push_span_block_edit(
                        &mut edits,
                        &line_starts,
                        src,
                        pipe,
                        body.span().end,
                        guard_target,
                    );
                }
                cursor = body.span().end;
            }
            if let (Some(first), Some(last)) = (eq.where_bindings.first(), eq.where_bindings.last())
            {
                if let Some(w) = find_keyword(src, cursor, first.span.start, "where", &comments) {
                    push_span_block_edit(
                        &mut edits,
                        &line_starts,
                        src,
                        w,
                        w + "where".len(),
                        guard_target,
                    );
                    push_block_edit(
                        &mut edits,
                        &line_starts,
                        src,
                        first.span.start,
                        last.span.end,
                        guard_target + INDENT,
                        eq_line,
                    );
                }
            }
        }
    }
    edits
}

fn reindent_guards(src: &str, module: &Module) -> String {
    let edits = guard_edits(src, module);
    if edits.is_empty() {
        return src.to_string();
    }
    apply_shifts(src, &edits)
}

// ---- record updates ----------------------------------------------------------

fn record_update_edits(src: &str, module: &Module) -> Vec<Edit> {
    let line_starts = line_start_table(src);
    let comments = comment_spans(src);
    let mut recs: Vec<(Span, usize, usize, usize)> = Vec::new();
    walk_module_expressions(module, &mut |e| {
        if let Expr::Record {
            span, base, fields, ..
        } = e
        {
            if matches!(base.as_ref(), Expr::Con { .. }) {
                return;
            }
            if let (Some(first), Some(last)) = (fields.first(), fields.last()) {
                recs.push((*span, base.span().end, first.span.start, last.span.end));
            }
        }
    });
    recs.sort_by(|a, b| a.0.start.cmp(&b.0.start).then(b.0.end.cmp(&a.0.end)));

    let mut edits = Vec::new();
    for (rec_span, base_end, first_field, last_field_end) in recs {
        let rec_line = line_of(&line_starts, rec_span.start);
        let field_line = line_of(&line_starts, first_field);
        if field_line <= rec_line || leading_has_tab(src, line_starts[rec_line]) {
            continue;
        }
        let Some(w) = find_keyword(src, base_end, first_field, "with", &comments) else {
            continue;
        };
        let with_line = line_of(&line_starts, w);
        let rec_indent = indent_of(src, &line_starts, rec_line);
        let field_target = if with_line > rec_line {
            push_line_edit(
                &mut edits,
                &line_starts,
                src,
                with_line,
                rec_indent + INDENT,
            );
            rec_indent + 2 * INDENT
        } else {
            rec_indent + INDENT
        };
        push_block_edit(
            &mut edits,
            &line_starts,
            src,
            first_field,
            last_field_end,
            field_target,
            rec_line,
        );
    }
    edits
}

fn reindent_record_updates(src: &str, module: &Module) -> String {
    let edits = record_update_edits(src, module);
    if edits.is_empty() {
        return src.to_string();
    }
    apply_shifts(src, &edits)
}

// ---- try/catch ---------------------------------------------------------------

fn try_edits(src: &str, module: &Module) -> Vec<Edit> {
    let line_starts = line_start_table(src);
    let comments = comment_spans(src);
    let mut tries: Vec<(Span, Span, Vec<Span>)> = Vec::new();
    walk_module_expressions(module, &mut |e| {
        if let Expr::Try {
            span,
            body,
            handlers,
            ..
        } = e
        {
            tries.push((
                *span,
                body.span(),
                handlers.iter().map(|h| h.span).collect(),
            ));
        }
    });
    tries.sort_by(|a, b| a.0.start.cmp(&b.0.start).then(b.0.end.cmp(&a.0.end)));

    let mut edits = Vec::new();
    for (try_span, body_span, handlers) in tries {
        let try_line = line_of(&line_starts, try_span.start);
        if leading_has_tab(src, line_starts[try_line]) {
            continue;
        }
        let try_col =
            usize_to_i64_saturating(src[line_starts[try_line]..try_span.start].chars().count());
        let nested_target = try_col + INDENT;
        push_block_edit(
            &mut edits,
            &line_starts,
            src,
            body_span.start,
            body_span.end,
            nested_target,
            try_line,
        );
        if let Some(first_handler) = handlers.first() {
            if let Some(catch) =
                find_keyword(src, body_span.end, first_handler.start, "catch", &comments)
            {
                push_span_block_edit(
                    &mut edits,
                    &line_starts,
                    src,
                    catch,
                    catch + "catch".len(),
                    try_col,
                );
            }
        }
        if let (Some(first), Some(last)) = (handlers.first(), handlers.last()) {
            push_block_edit(
                &mut edits,
                &line_starts,
                src,
                first.start,
                last.end,
                nested_target,
                try_line,
            );
        }
    }
    edits
}

fn reindent_tries(src: &str, module: &Module) -> String {
    let edits = try_edits(src, module);
    if edits.is_empty() {
        return src.to_string();
    }
    apply_shifts(src, &edits)
}

// ---- expression continuations ------------------------------------------------

fn continuation_edits(src: &str, module: &Module) -> Vec<Edit> {
    let line_starts = line_start_table(src);
    let comments = comment_spans(src);
    let mut edits = Vec::new();
    walk_module_expressions(module, &mut |e| match e {
        Expr::Tuple { span, items, .. } | Expr::List { span, items, .. } => {
            push_item_continuations(
                &mut edits,
                src,
                &line_starts,
                &comments,
                *span,
                items.iter().map(Expr::span),
            );
        }
        _ => {}
    });
    edits
}

fn push_item_continuations(
    edits: &mut Vec<Edit>,
    src: &str,
    ls: &[usize],
    comments: &[(usize, usize)],
    span: Span,
    items: impl Iterator<Item = Span>,
) {
    let head_line = line_of(ls, span.start);
    let target = indent_of(src, ls, head_line) + INDENT;
    for item in items {
        let item_line = line_of(ls, item.start);
        if item_line <= head_line {
            continue;
        }
        let mut first = item.start;
        let line_start = ls[item_line];
        if let Some(comma) = src[line_start..item.start].rfind(',') {
            first = line_start + comma;
        }
        if is_comment_line(comments, first) {
            continue;
        }
        push_span_block_edit(edits, ls, src, first, item.end, target);
    }
}

fn reindent_continuations(src: &str, module: &Module) -> String {
    let edits = continuation_edits(src, module);
    if edits.is_empty() {
        return src.to_string();
    }
    apply_shifts(src, &edits)
}

// ---- shared line-edit helpers ------------------------------------------------

fn push_span_block_edit(
    edits: &mut Vec<Edit>,
    ls: &[usize],
    src: &str,
    first_byte: usize,
    end_byte: usize,
    target: i64,
) {
    let first_line = line_of(ls, first_byte);
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

fn push_code_line_edit(
    edits: &mut Vec<Edit>,
    src: &str,
    ls: &[usize],
    comments: &[(usize, usize)],
    line: usize,
    target: i64,
) {
    if code_line_trimmed(src, ls, comments, line).is_none() || leading_has_tab(src, ls[line]) {
        return;
    }
    push_line_edit(edits, ls, src, line, target);
}

fn code_line_trimmed<'a>(
    src: &'a str,
    ls: &[usize],
    comments: &[(usize, usize)],
    line: usize,
) -> Option<&'a str> {
    let start = *ls.get(line)?;
    let end = *ls.get(line + 1).unwrap_or(&src.len());
    let text = &src[start..end];
    let cur = text.len() - text.trim_start_matches(' ').len();
    let trimmed = text[cur..].trim_end();
    if trimmed.is_empty() || is_comment_line(comments, start + cur) {
        None
    } else {
        Some(trimmed)
    }
}

fn starts_with_word(s: &str, word: &str) -> bool {
    s.strip_prefix(word).is_some_and(|rest| {
        rest.is_empty()
            || rest
                .chars()
                .next()
                .is_some_and(|c| !c.is_ascii_alphanumeric() && c != '_' && c != '\'')
    })
}

fn line_contains_word(src: &str, ls: &[usize], line: usize, word: &str) -> bool {
    let end = *ls.get(line + 1).unwrap_or(&src.len());
    let line_text = &src[ls[line]..end];
    let mut i = 0;
    while let Some(rel) = line_text[i..].find(word) {
        let at = i + rel;
        let before_ok = at == 0 || !is_ident_byte(line_text.as_bytes()[at - 1]);
        let after = at + word.len();
        let after_ok = after >= line_text.len() || !is_ident_byte(line_text.as_bytes()[after]);
        if before_ok && after_ok {
            return true;
        }
        i = at + 1;
    }
    false
}

const fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'\''
}

fn line_contains_symbol(src: &str, ls: &[usize], line: usize, symbol: &str) -> bool {
    let end = *ls.get(line + 1).unwrap_or(&src.len());
    src[ls[line]..end].contains(symbol)
}

fn find_symbol(
    src: &str,
    from: usize,
    to: usize,
    symbol: &str,
    comments: &[(usize, usize)],
) -> Option<usize> {
    let hay = &src[from..to.min(src.len())];
    let mut i = 0;
    while let Some(rel) = hay[i..].find(symbol) {
        let abs = from + i + rel;
        if !is_comment_line(comments, abs) {
            return Some(abs);
        }
        i += rel + symbol.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format_ast(src: &str) -> String {
        super::format_ast(src, crate::FormatOptions::default())
    }

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
    fn source_range_expectation_files_stay_byte_exact() {
        let src = "module M where\n-- @ WARN range=3:8-3:9; x\nfoo : Int\nfoo = 1\n";
        assert_eq!(format_ast(src), src);
    }

    #[test]
    fn source_location_query_files_stay_byte_exact() {
        let src = "-- @QUERY-LF .location.range | (.start_line == 8 and .start_col == 9)\n\n\nmodule Locations where\nfoo : Int\nfoo = 1\n";
        assert_eq!(format_ast(src), src);
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
    fn do_block_with_try_is_reindented() {
        // A do-block containing try/catch is now owned by the do and try passes.
        let src = "f = do\n      _ <- try foo catch _ -> bar\n      pure ()\n";
        let out = format_ast(src);
        assert_eq!(out, "f = do\n  _ <- try foo catch _ -> bar\n  pure ()\n");
        assert_eq!(format_ast(&out), out);
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
    fn single_line_if_is_expanded() {
        let src = "g x = if x then 1 else 2\n";
        assert_eq!(format_ast(src), "g x =\n  if x\n    then 1\n    else 2\n");
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
    fn inline_case_alts_are_expanded() {
        let src = "f x = case x of None -> 1; Some y -> y\n";
        assert_eq!(
            format_ast(src),
            "f x = case x of\n  None -> 1\n  Some y -> y\n"
        );
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
    fn inline_letin_is_expanded() {
        let src = "f = let x = 1 in x\n";
        assert_eq!(format_ast(src), "f =\n  let\n    x = 1\n  in x\n");
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
    fn record_update_fields_are_reindented() {
        // base is an expression (`this`), not a bare constructor: an update.
        let src = "f this p = this with\n      owner = p\n";
        let out = format_ast(src);
        assert_eq!(out, "f this p = this with\n  owner = p\n");
        assert_eq!(format_ast(&out), out);
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
    fn inline_con_with_fields_are_expanded() {
        let src = "f = Asset with issuer = a; owner = b\n";
        assert_eq!(
            format_ast(src),
            "f = Asset with\n  issuer = a\n  owner = b\n"
        );
    }

    #[test]
    fn con_with_before_where_keeps_fields_inside_expression() {
        let src = "module M where\nquery : T\nquery = lift $ QueryACS with\n    parties = p\n    tplId = t\n  where\n    convert = x\n";
        let out = format_ast(src);
        assert_eq!(
            out,
            "module M where\nquery: T\nquery = lift $ QueryACS with\n    parties = p\n    tplId = t\n  where\n    convert = x\n"
        );
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
    fn choice_internal_ladder_is_canonicalized() {
        let src = "template T\n  with\n    p: Party\n  where\n    choice C\n          : ()\n          with\n              arg: Text\n          observer p\n          controller p\n          do\n              pure ()\n";
        let out = format_ast(src);
        let want = "template T\n  with\n    p: Party\n  where\n    choice C\n      : ()\n      with\n        arg: Text\n      observer p\n      controller p\n      do\n        pure ()\n";
        assert_eq!(out, want);
        assert_eq!(format_ast(&out), out);
    }

    #[test]
    fn choice_keyword_scan_ignores_identifier_fragments() {
        let src = "template T\n  with\n    p: Party\n  where\n    choice C\n          : ()\n          with\n              observer_name: Party\n          observer p\n          controller p\n          do\n              pure ()\n";
        let out = format_ast(src);
        let want = "template T\n  with\n    p: Party\n  where\n    choice C\n      : ()\n      with\n        observer_name: Party\n      observer p\n      controller p\n      do\n        pure ()\n";
        assert_eq!(out, want);
        assert_eq!(format_ast(&out), out);
    }

    #[test]
    fn type_def_ladders_are_canonicalized() {
        let src = "data Color = Grey\n           | RGB\n                with r: Int\n           deriving (Eq, Show)\n\nexception E\n      with\n          msg: Text\n      where\n          message msg\n";
        let out = format_ast(src);
        let want = "data Color = Grey\n  | RGB\n    with r: Int\n  deriving (Eq, Show)\n\nexception E\n  with\n    msg: Text\n  where\n    message msg\n";
        assert_eq!(out, want);
        assert_eq!(format_ast(&out), out);
    }

    #[test]
    fn data_record_with_ladder_keeps_with_above_fields() {
        let src =
            "data ReceiverAmount = ReceiverAmount\n    with\n      receiver : Party\n      amount : Decimal\n";
        let out = format_ast(src);
        let want =
            "data ReceiverAmount = ReceiverAmount\n  with\n    receiver: Party\n    amount: Decimal\n";
        assert_eq!(out, want);
        assert_eq!(format_ast(&out), out);
    }

    #[test]
    fn inline_data_record_with_braces_keeps_body_column() {
        let src = "data Data = Data with\n  { dummy : ()\n  , srcLoc : SrcLoc\n  }\n";
        assert_eq!(format_ast(src), src);
    }

    #[test]
    fn class_where_body_with_comments_keeps_body_indent() {
        let src = "class ActionState s m | m -> s where\n  {-# MINIMAL get, (put | modify) #-}\n  -- | Fetch the current value.\n  get : m s\n\n  -- | Set the value.\n  put : s -> m ()\n  put = modify . const\n";
        let out = format_ast(src);
        let want = "class ActionState s m | m -> s where\n  {-# MINIMAL get, (put | modify) #-}\n  -- | Fetch the current value.\n  get: m s\n\n  -- | Set the value.\n  put: s -> m ()\n  put = modify . const\n";
        assert_eq!(out, want);
        assert_eq!(format_ast(&out), out);
    }

    #[test]
    fn class_where_body_with_indented_pragma_keeps_pragma_indent() {
        let src = "class Foo t where\n    {-# MINIMAL foo1 | foo2 #-}\n\n    foo1 : t -> Int\n    foo1 x = foo1 x + 1\n";
        let out = format_ast(src);
        let want = "class Foo t where\n    {-# MINIMAL foo1 | foo2 #-}\n\n    foo1: t -> Int\n    foo1 x = foo1 x + 1\n";
        assert_eq!(out, want);
        assert_eq!(format_ast(&out), out);
    }

    #[test]
    fn guards_and_where_bindings_are_canonicalized() {
        let src =
            "f x\n      | x > 0 = g\n               x\n      | otherwise = 0\n      where\n          g y = y\n";
        let out = format_ast(src);
        let want = "f x\n  | x > 0 = g\n           x\n  | otherwise = 0\n  where\n    g y = y\n";
        assert_eq!(out, want);
        assert_eq!(format_ast(&out), out);
    }

    #[test]
    fn multiline_try_catch_is_canonicalized() {
        let src = "f = try\n        foo\n      catch\n        _ -> bar\n";
        let out = format_ast(src);
        let want = "f = try\n      foo\n    catch\n      _ -> bar\n";
        assert_eq!(out, want);
        assert_eq!(format_ast(&out), out);
    }

    #[test]
    fn explicit_list_continuations_are_canonicalized() {
        let src = "x = [ 1\n      , 2\n      , 3 ]\n";
        let out = format_ast(src);
        let want = "x = [ 1\n  , 2\n  , 3 ]\n";
        assert_eq!(out, want);
        assert_eq!(format_ast(&out), out);
    }

    #[test]
    fn module_and_import_continuations_are_canonicalized() {
        let src = "module M\n      ( f\n      , g\n      ) where\n\nimport DA.Map\n      ( Map\n      )\n";
        let out = format_ast(src);
        let want = "module M\n  ( f\n  , g\n  ) where\n\nimport DA.Map\n  ( Map\n  )\n";
        assert_eq!(out, want);
        assert_eq!(format_ast(&out), out);
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
        assert!(out.ends_with("\r\n"), "got: {out:?}");
        assert!(!out.ends_with("\n\n"));
        assert_eq!(format_ast(&out), out); // idempotent
    }
}
