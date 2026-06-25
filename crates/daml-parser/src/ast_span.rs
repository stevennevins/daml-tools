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

use crate::ast::{
    Alt, Binding, ChoiceDecl, Decl, DoStmt, Equation, Expr, FieldAssign, FixityDecl, Module, Pat,
    Span, TemplateBodyDecl, Type, TypeAnnotation,
};
use crate::lexer::{Trivia, TriviaKind};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AstSpanError {
    /// An AST span had `start > end`; both fields are byte offsets.
    InvalidSpan {
        /// Inclusive start byte offset.
        start: usize,
        /// Exclusive end byte offset.
        end: usize,
    },
    /// Two AST spans overlapped without one containing the other.
    OverlappingSpans {
        /// Inclusive start byte offset of the offending span.
        start: usize,
        /// Exclusive end byte offset of the offending span.
        end: usize,
        /// Inclusive start byte offset of the active parent/sibling span.
        parent_start: usize,
        /// Exclusive end byte offset of the active parent/sibling span.
        parent_end: usize,
    },
    /// A reconstruction tile overlapped bytes already emitted.
    OverlappingTile {
        /// Inclusive start byte offset of the overlapping tile.
        start: usize,
        /// Exclusive end byte offset of the overlapping tile.
        end: usize,
        /// Exclusive end byte offset of the previous tile.
        previous_end: usize,
    },
    /// A non-empty source byte interval was not covered by AST or trivia.
    UncoveredBytes {
        /// Inclusive start byte offset of the uncovered interval.
        start: usize,
        /// Exclusive end byte offset of the uncovered interval.
        end: usize,
        /// Source text from the uncovered interval.
        text: String,
    },
    /// Source bytes after the final AST/trivia interval were not covered.
    UncoveredTail {
        /// Inclusive start byte offset of the uncovered tail.
        start: usize,
        /// Source text from the uncovered tail.
        text: String,
    },
    /// Reconstructed source length differed from the original source length.
    ReconstructionMismatch {
        /// Reconstructed byte length.
        reconstructed_len: usize,
        /// Original source byte length.
        source_len: usize,
    },
    /// A token/trivia interval had `start > end`; both fields are byte offsets.
    IntervalStartAfterEnd {
        /// Inclusive start byte offset.
        start: usize,
        /// Exclusive end byte offset.
        end: usize,
    },
    /// A token/trivia interval extended past `source_len`.
    IntervalExceedsSource {
        /// Inclusive start byte offset.
        start: usize,
        /// Exclusive end byte offset.
        end: usize,
        /// Original source byte length.
        source_len: usize,
    },
    /// A token/trivia interval boundary was not a UTF-8 boundary in `source`.
    IntervalNotUtf8Boundary {
        /// Inclusive start byte offset.
        start: usize,
        /// Exclusive end byte offset.
        end: usize,
    },
}

impl std::fmt::Display for AstSpanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidSpan { start, end } => write!(f, "invalid span [{start}, {end})"),
            Self::OverlappingSpans {
                start,
                end,
                parent_start,
                parent_end,
            } => write!(
                f,
                "span [{start}, {end}) overlaps [{parent_start}, {parent_end}) without nesting"
            ),
            Self::OverlappingTile {
                start,
                end,
                previous_end,
            } => write!(
                f,
                "span/trivia interval [{start}, {end}) overlaps previous tile ending at {previous_end}"
            ),
            Self::UncoveredBytes { start, end, text } => write!(
                f,
                "bytes {start}..{end} not covered by any node or trivia span: {text:?}"
            ),
            Self::UncoveredTail { start, text } => {
                write!(f, "bytes {start}.. lost at EOF: {text:?}")
            }
            Self::ReconstructionMismatch {
                reconstructed_len,
                source_len,
            } => write!(
                f,
                "reconstruction differs from source ({reconstructed_len} vs {source_len} bytes)"
            ),
            Self::IntervalStartAfterEnd { start, end } => {
                write!(f, "span/trivia interval [{start}, {end}) has start after end")
            }
            Self::IntervalExceedsSource {
                start,
                end,
                source_len,
            } => write!(
                f,
                "span/trivia interval [{start}, {end}) exceeds source length {source_len}"
            ),
            Self::IntervalNotUtf8Boundary { start, end } => write!(
                f,
                "span/trivia interval [{start}, {end}) does not align with UTF-8 boundaries"
            ),
        }
    }
}

impl std::error::Error for AstSpanError {}

/// Reconstruct `source` from the AST's byte spans plus the lexer's `trivia`.
///
/// The AST-level mirror of `lexer::render_lossless`: it checks the spans nest
/// (V2), then tiles the file from every *content* node span merged with the
/// non-blank trivia spans (V1/V3). `Ok(reconstruction)` is byte-identical to
/// `source`; `Err` names the first nesting violation or the first run of bytes
/// no span covers (content the AST dropped).
///
/// Obtain `trivia` from [`crate::lexer::lex_with_trivia`].
///
/// # Errors
///
/// Returns [`AstSpanError`] when spans fail nesting checks or when bytes in
/// `source` are not covered by any AST node or trivia span.
#[must_use = "check and handle render failures"]
pub fn render_from_ast(
    source: &str,
    module: &Module,
    trivia: &[Trivia],
) -> Result<String, AstSpanError> {
    check_nesting(module)?;
    tile(source, module, trivia)
}

/// V2 — every child span is contained in its parent, and sibling spans are
/// ordered and disjoint. Validates the whole node set (module container
/// included) as a laminar family via a containment stack.
fn check_nesting(module: &Module) -> Result<(), AstSpanError> {
    let mut spans: Vec<Span> = Vec::new();
    collect_module(module, &mut spans);
    for span in &spans {
        if !span.is_valid() {
            return Err(AstSpanError::InvalidSpan {
                start: span.start_usize(),
                end: span.end_usize(),
            });
        }
    }
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
                return Err(AstSpanError::OverlappingSpans {
                    start: span.start_usize(),
                    end: span.end_usize(),
                    parent_start: parent.start_usize(),
                    parent_end: parent.end_usize(),
                });
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
fn tile(source: &str, module: &Module, trivia: &[Trivia]) -> Result<String, AstSpanError> {
    let mut content: Vec<Span> = Vec::new();
    collect_module(module, &mut content);
    let container = module.span;
    content.retain(|s| !(s.is_empty() || (s.start == container.start && s.end == container.end)));

    let mut items: Vec<(usize, usize)> = content
        .iter()
        .map(|s| (s.start_usize(), s.end_usize()))
        .collect();
    // Blank-line trivia carry no bytes; comment/CPP trivia fill the gaps that
    // are legitimately not AST nodes.
    items.extend(
        trivia
            .iter()
            .filter(|t| !matches!(t.kind, TriviaKind::BlankLines(_)))
            .map(|t| (t.start, t.end)),
    );
    // Outer-first, like `check_nesting`: when intervals share a start, emit the
    // broader parent before contained children so child spans can be skipped as
    // already-covered tiles.
    items.sort_unstable_by(|a, b| a.0.cmp(&b.0).then(b.1.cmp(&a.1)));

    let mut out = String::with_capacity(source.len());
    let mut prev = 0usize;
    for (start, end) in items {
        validate_interval(source, start, end)?;
        if start < prev {
            // Nested AST child spans and contained trivia are already covered
            // by their parent tile. A partial overlap that extends past `prev`
            // cannot be tiled losslessly and means the interval set is invalid.
            if end <= prev {
                continue;
            }
            return Err(AstSpanError::OverlappingTile {
                start,
                end,
                previous_end: prev,
            });
        }
        let gap = &source[prev..start];
        if !gap.chars().all(char::is_whitespace) {
            return Err(AstSpanError::UncoveredBytes {
                start: prev,
                end: start,
                text: gap.to_string(),
            });
        }
        out.push_str(gap);
        out.push_str(&source[start..end]);
        prev = end;
    }
    let tail = &source[prev..];
    if !tail.chars().all(char::is_whitespace) {
        return Err(AstSpanError::UncoveredTail {
            start: prev,
            text: tail.to_string(),
        });
    }
    out.push_str(tail);

    if out != source {
        return Err(AstSpanError::ReconstructionMismatch {
            reconstructed_len: out.len(),
            source_len: source.len(),
        });
    }
    Ok(out)
}

const fn validate_interval(source: &str, start: usize, end: usize) -> Result<(), AstSpanError> {
    if start > end {
        return Err(AstSpanError::IntervalStartAfterEnd { start, end });
    }
    if end > source.len() {
        return Err(AstSpanError::IntervalExceedsSource {
            start,
            end,
            source_len: source.len(),
        });
    }
    if !source.is_char_boundary(start) || !source.is_char_boundary(end) {
        return Err(AstSpanError::IntervalNotUtf8Boundary { start, end });
    }
    Ok(())
}

// ----- span collection ---------------------------------------------------

fn collect_module(module: &Module, spans: &mut Vec<Span>) {
    spans.push(module.span);
    if !module.header.is_empty() {
        spans.push(module.header);
    }
    for import in &module.imports {
        spans.push(import.span);
    }
    for decl in &module.decls {
        collect_decl(decl, spans);
    }
}

fn collect_decl(decl: &Decl, spans: &mut Vec<Span>) {
    match decl {
        Decl::Template(template) => {
            spans.push(template.span);
            for field in &template.fields {
                spans.push(field.span);
                if let TypeAnnotation::Present(ty) = &field.ty {
                    collect_type(ty, spans);
                }
            }
            for body_decl in &template.body {
                collect_tbody(body_decl, spans);
            }
        }
        Decl::Interface(interface) => {
            spans.push(interface.span);
            for method in &interface.methods {
                spans.push(method.span);
                if let TypeAnnotation::Present(ty) = &method.ty {
                    collect_type(ty, spans);
                }
            }
            for choice in &interface.choices {
                collect_choice(choice, spans);
            }
        }
        Decl::Function(function) => {
            // `function.span` is the equations' extent (contiguous); the signature,
            // which may sit apart, is a separate sibling span.
            spans.push(function.span);
            for equation in &function.equations {
                collect_eq(equation, spans);
            }
            if let Some(sig) = function.sig_span {
                spans.push(sig);
            }
            if let TypeAnnotation::Present(ty) = &function.ty {
                collect_type(ty, spans);
            }
        }
        Decl::TypeDef { span, .. }
        | Decl::Unknown { span, .. }
        | Decl::Fixity(FixityDecl { span, .. })
        | Decl::UnsupportedSyntax { span, .. } => spans.push(*span),
    }
}

fn collect_tbody(template_body_decl: &TemplateBodyDecl, spans: &mut Vec<Span>) {
    match template_body_decl {
        TemplateBodyDecl::Signatory { parties, span, .. }
        | TemplateBodyDecl::Observer { parties, span, .. } => {
            spans.push(*span);
            for party in parties {
                collect_expr(party, spans);
            }
        }
        TemplateBodyDecl::Ensure { expr, span, .. }
        | TemplateBodyDecl::Maintainer { expr, span, .. } => {
            spans.push(*span);
            collect_expr(expr, spans);
        }
        TemplateBodyDecl::Key { expr, span, ty, .. } => {
            spans.push(*span);
            collect_expr(expr, spans);
            if let TypeAnnotation::Present(ty) = ty {
                collect_type(ty, spans);
            }
        }
        TemplateBodyDecl::Choice(choice) => collect_choice(choice, spans),
        TemplateBodyDecl::InterfaceInstance(interface_instance) => {
            spans.push(interface_instance.span);
            for method in &interface_instance.methods {
                collect_binding(method, spans);
            }
        }
        TemplateBodyDecl::Other { span, .. } => spans.push(*span),
    }
}

fn collect_choice(choice: &ChoiceDecl, spans: &mut Vec<Span>) {
    spans.push(choice.span);
    for param in &choice.params {
        spans.push(param.span);
        if let TypeAnnotation::Present(ty) = &param.ty {
            collect_type(ty, spans);
        }
    }
    for controller in &choice.controllers {
        collect_expr(controller, spans);
    }
    for observer in &choice.observers {
        collect_expr(observer, spans);
    }
    if let Some(body) = &choice.body {
        collect_expr(body, spans);
    }
    if let TypeAnnotation::Present(return_ty) = &choice.return_ty {
        collect_type(return_ty, spans);
    }
}

fn collect_type(ty: &Type, spans: &mut Vec<Span>) {
    spans.push(ty.span());
    match ty {
        Type::Con { .. } | Type::Var(_, _) | Type::Unit(_) | Type::Lit { .. } => {}
        Type::App(head, args, _) => {
            collect_type(head, spans);
            for arg in args {
                collect_type(arg, spans);
            }
        }
        Type::List(inner, _) | Type::Constrained(inner, _) => collect_type(inner, spans),
        Type::Tuple(elems, _) => {
            for elem in elems {
                collect_type(elem, spans);
            }
        }
        Type::Fun(lhs, rhs, _) => {
            collect_type(lhs, spans);
            collect_type(rhs, spans);
        }
    }
}

fn collect_eq(equation: &Equation, spans: &mut Vec<Span>) {
    spans.push(equation.span);
    for param in &equation.params {
        collect_pat(param, spans);
    }
    collect_expr(&equation.body, spans);
    for (guard, body) in &equation.guards {
        collect_expr(guard, spans);
        collect_expr(body, spans);
    }
    for where_binding in &equation.where_bindings {
        collect_binding(where_binding, spans);
    }
}

fn collect_binding(binding: &Binding, spans: &mut Vec<Span>) {
    spans.push(binding.span);
    collect_pat(&binding.pat, spans);
    for param in &binding.params {
        collect_pat(param, spans);
    }
    collect_expr(&binding.expr, spans);
}

fn collect_pat(pattern: &Pat, spans: &mut Vec<Span>) {
    spans.push(pattern.span());
    match pattern {
        Pat::Con { args, .. } => {
            for arg in args {
                collect_pat(arg, spans);
            }
        }
        Pat::Tuple { items, .. } | Pat::List { items, .. } => {
            for item in items {
                collect_pat(item, spans);
            }
        }
        Pat::As { pat, .. } => collect_pat(pat, spans),
        Pat::Var { .. } | Pat::Wild { .. } | Pat::Lit { .. } | Pat::Other { .. } => {}
    }
}

fn collect_expr(expr: &Expr, spans: &mut Vec<Span>) {
    spans.push(expr.span());
    match expr {
        Expr::App { func, args, .. } => {
            collect_expr(func, spans);
            for arg in args {
                collect_expr(arg, spans);
            }
        }
        Expr::BinOp { lhs, rhs, .. } => {
            collect_expr(lhs, spans);
            collect_expr(rhs, spans);
        }
        Expr::Neg { expr, .. } => collect_expr(expr, spans),
        Expr::Lambda { params, body, .. } => {
            for param in params {
                collect_pat(param, spans);
            }
            collect_expr(body, spans);
        }
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => {
            collect_expr(cond, spans);
            collect_expr(then_branch, spans);
            collect_expr(else_branch, spans);
        }
        Expr::Case {
            scrutinee, alts, ..
        } => {
            collect_expr(scrutinee, spans);
            for alt in alts {
                collect_alt(alt, spans);
            }
        }
        Expr::Do { stmts, .. } => {
            for stmt in stmts {
                collect_dostmt(stmt, spans);
            }
        }
        Expr::LetIn { bindings, body, .. } => {
            for binding in bindings {
                collect_binding(binding, spans);
            }
            collect_expr(body, spans);
        }
        Expr::Record { base, fields, .. } => {
            collect_expr(base, spans);
            for field in fields {
                collect_field_assign(field, spans);
            }
        }
        Expr::Tuple { items, .. } | Expr::List { items, .. } => {
            for item in items {
                collect_expr(item, spans);
            }
        }
        Expr::Try { body, handlers, .. } => {
            collect_expr(body, spans);
            for handler in handlers {
                collect_alt(handler, spans);
            }
        }
        Expr::LeftSection { operand, .. } | Expr::RightSection { operand, .. } => {
            collect_expr(operand, spans);
        }
        Expr::Var { .. }
        | Expr::Con { .. }
        | Expr::Lit { .. }
        | Expr::OperatorRef { .. }
        | Expr::Error { .. } => {}
    }
}

fn collect_alt(alt: &Alt, spans: &mut Vec<Span>) {
    spans.push(alt.span);
    collect_pat(&alt.pat, spans);
    collect_expr(&alt.body, spans);
}

fn collect_field_assign(field_assign: &FieldAssign, spans: &mut Vec<Span>) {
    spans.push(field_assign.span());
    if let FieldAssign::Assign { value, .. } = field_assign {
        collect_expr(value, spans);
    }
}

fn collect_dostmt(do_stmt: &DoStmt, spans: &mut Vec<Span>) {
    match do_stmt {
        DoStmt::Bind {
            pat, expr, span, ..
        } => {
            spans.push(*span);
            collect_pat(pat, spans);
            collect_expr(expr, spans);
        }
        DoStmt::Let { bindings, span, .. } => {
            spans.push(*span);
            for binding in bindings {
                collect_binding(binding, spans);
            }
        }
        DoStmt::Expr { expr, span, .. } => {
            spans.push(*span);
            collect_expr(expr, spans);
        }
    }
}

// Tile-oracle interval validation and overlap detection for the ast_span phase.
#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::lexer::{Pos, Trivia};

    #[test]
    fn tile_reports_overlapping_intervals() {
        let source = "module M where\n-- comment\n";
        let module = Module {
            name: "M".into(),
            pos: Pos { line: 1, column: 1 },
            span: Span::from_usize(0, source.len()),
            header: Span::from_usize(0, "module M where".len()),
            imports: Vec::new(),
            decls: Vec::new(),
        };
        let trivia = vec![Trivia {
            kind: TriviaKind::LineComment,
            text: "-- comment".to_string(),
            pos: Pos { line: 2, column: 1 },
            start: "module M wher".len(),
            end: "module M where\n-- comment".len(),
        }];

        let err = tile(source, &module, &trivia).unwrap_err();
        assert!(
            err.to_string().contains("overlaps previous tile"),
            "overlap should fail loudly, got: {err}"
        );
    }

    #[test]
    fn validate_interval_reports_malformed_bounds() {
        assert_eq!(
            validate_interval("abc", 2, 1),
            Err(AstSpanError::IntervalStartAfterEnd { start: 2, end: 1 })
        );
        assert_eq!(
            validate_interval("abc", 0, 4),
            Err(AstSpanError::IntervalExceedsSource {
                start: 0,
                end: 4,
                source_len: 3
            })
        );
        assert_eq!(
            validate_interval("é", 1, 2),
            Err(AstSpanError::IntervalNotUtf8Boundary { start: 1, end: 2 })
        );
    }

    #[test]
    fn tile_reports_uncovered_non_whitespace_bytes() {
        let source = "module M where\nmissing\n";
        let module = Module {
            name: "M".into(),
            pos: Pos { line: 1, column: 1 },
            span: Span::from_usize(0, source.len()),
            header: Span::from_usize(0, "module M where".len()),
            imports: Vec::new(),
            decls: Vec::new(),
        };

        assert_eq!(
            tile(source, &module, &[]),
            Err(AstSpanError::UncoveredTail {
                start: "module M where".len(),
                text: "\nmissing\n".to_string(),
            })
        );
    }

    #[test]
    fn render_from_ast_roundtrips_header_only_module() {
        let source = "module M where\n";
        let module = Module {
            name: "M".into(),
            pos: Pos { line: 1, column: 1 },
            span: Span::from_usize(0, source.len()),
            header: Span::from_usize(0, "module M where".len()),
            imports: Vec::new(),
            decls: Vec::new(),
        };

        assert_eq!(render_from_ast(source, &module, &[]).as_deref(), Ok(source));
    }
}
