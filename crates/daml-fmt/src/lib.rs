//! daml-fmt: a code formatter for Daml, built on the daml-parser pipeline.
//!
//! Strategy: reconstruct the file from the lossless token+trivia stream and
//! normalize whitespace in the gaps between spans — never inside string or
//! comment spans (CLAUDE.md: comments are sacred). Every candidate output is
//! passed through a TOKEN-EQUIVALENCE GATE before it is returned: re-lex it and
//! require the full token stream — including the virtual layout tokens
//! (`VLBrace`/`VSemi`/`VRBrace`) that encode Daml's offside rule — to be
//! identical to the input's. Identical post-layout tokens ⇒ identical parse ⇒
//! identical desugar, so the gate makes a rewrite desugar-safe by construction
//! (the corpus desugar oracle still verifies it). If a rule would change the
//! token stream (e.g. collapsing `+ :` into the operator `+:`), the gate falls
//! back to a safer output and ultimately to the input unchanged.
//!
//! The shipping backend is `layout_ast` (AST-driven, our own pattern — NOT a
//! LimeChain derivative, and NOT aimed at matching the `expected/` baseline).
//! `normalize_gaps` below is the proven, token-gated whitespace + colon-spacing
//! pass it composes on top of the structural reindent:
//! - trailing-whitespace: strip spaces/tabs before a newline; one final newline.
//! - colon-spacing: `name : Type` -> `name: Type` (drop same-line spaces before a
//!   lone `:` type-annotation colon; never `::`, never a line-leading colon).

// AST-driven layout (our own pattern, NO LimeChain derivative). This is the
// shipping backend. See src/layout_ast.rs.
pub mod layout_ast;

use daml_parser::lexer::{lex_with_trivia, Tok, TriviaKind};

/// Lexer diagnostics for `src`, one `line:col: message` string per error
/// (e.g. unterminated string / block comment). Empty when the source lexes
/// clean. The formatter still passes malformed input through verbatim
/// (`format_source` is byte-safe); the CLI uses this to surface a non-zero
/// exit + diagnostic so a formatter "success" is not mistaken for parse
/// success. All 924 corpus files lex clean, so this never flags them.
pub fn lex_diagnostics(src: &str) -> Vec<String> {
    let (_tokens, _trivia, errors) = lex_with_trivia(src);
    errors
        .iter()
        .map(|e| format!("{}:{}: {}", e.pos.line, e.pos.column, e.message))
        .collect()
}

/// Format Daml source.
///
/// Delegates to the AST-driven backend (`layout_ast::format_ast`): an
/// own-design canonical layout that reindents `do`-blocks, applies the
/// token-gated whitespace + colon-spacing normalization, and passes every
/// unmodeled construct through verbatim. Every change is gated on the offside
/// token stream, so it is desugar-safe by construction.
pub fn format_source(src: &str) -> String {
    layout_ast::format_ast(src)
}

/// Reconstruct `src`, normalizing gap whitespace. With `colon`, also drop
/// same-line spaces before a lone `:` token. Shared with the AST backend
/// (`layout_ast`) so both paths apply the same proven, token-gated spacing.
pub(crate) fn normalize_gaps(src: &str, colon: bool) -> String {
    rewrite(src, colon)
}

fn rewrite(src: &str, colon: bool) -> String {
    let (tokens, trivia, _lex_errors) = lex_with_trivia(src);

    // Items that carry bytes, in source order. For each: brace-depth delta
    // (+1/-1 for `{`/`}`/parens), is-lone-colon, is-rparen, is-token (vs trivia).
    let mut items: Vec<(usize, usize, i32, bool, bool, bool)> = tokens
        .iter()
        .filter(|t| !matches!(t.tok, Tok::VLBrace | Tok::VRBrace | Tok::VSemi))
        .map(|t| {
            (
                t.start,
                t.end,
                brace_delta(&t.tok),
                is_lone_colon(&t.tok),
                matches!(t.tok, Tok::RParen),
                true,
            )
        })
        .chain(
            trivia
                .iter()
                .filter(|t| !matches!(t.kind, TriviaKind::BlankLines(_)))
                .map(|t| (t.start, t.end, 0, false, false, false)),
        )
        .collect();
    items.sort_by_key(|&(start, ..)| start);

    let mut out = String::with_capacity(src.len());
    let mut prev = 0usize;
    let mut brace_depth: i32 = 0;
    let mut prev_was_rparen = false;
    // True when the previously-emitted token was a type-annotation colon we
    // canonicalized (same gate as the before-colon collapse). Lets us collapse
    // a duplicate space *after* that colon (`x:  T` -> `x: T`) symmetrically.
    let mut prev_was_canon_colon = false;
    for (start, end, delta, is_colon, is_rparen, is_token) in items {
        if start < prev {
            return src.to_string(); // overlap — bail
        }
        let gap = &src[prev..start];
        if !gap.chars().all(char::is_whitespace) {
            return src.to_string(); // non-whitespace between spans — bail
        }
        // Canonicalize the space around a lone colon only OUTSIDE braces/parens
        // and not after `)`: `with`-block / field colons canonicalize to `x: T`,
        // but `{ field : Type }`, `(n : Nat)` and function-return `(args) : Ret`
        // keep the space (expected/ convention).
        let this_is_canon_colon = colon && is_colon && brace_depth == 0 && !prev_was_rparen;
        if this_is_canon_colon && !gap.is_empty() && !gap.contains('\n') {
            // drop same-line space(s) before the colon
        } else if prev_was_canon_colon && !gap.is_empty() && !gap.contains('\n') {
            // collapse same-line space(s) after a canonicalized colon to one
            out.push(' ');
        } else {
            out.push_str(&strip_trailing_ws(gap));
        }
        out.push_str(&src[start..end]);
        prev = end;
        brace_depth += delta;
        prev_was_canon_colon = this_is_canon_colon;
        if is_token {
            prev_was_rparen = is_rparen; // trivia leave the previous token intact
        }
    }
    let tail = &src[prev..];
    if !tail.chars().all(char::is_whitespace) {
        return src.to_string();
    }
    out.push_str(&strip_trailing_ws(tail));

    normalize_final_newline(&mut out);
    out
}

fn is_lone_colon(t: &Tok) -> bool {
    matches!(t, Tok::Op(op) if op == ":")
}

fn brace_delta(t: &Tok) -> i32 {
    match t {
        Tok::LBrace | Tok::LParen => 1,
        Tok::RBrace | Tok::RParen => -1,
        _ => 0,
    }
}

/// In a whitespace-only gap, drop runs of spaces/tabs that immediately precede a
/// newline. Leading indentation and inter-token spacing are preserved.
fn strip_trailing_ws(gap: &str) -> String {
    let chars: Vec<char> = gap.chars().collect();
    let mut out = String::with_capacity(gap.len());
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if c == ' ' || c == '\t' {
            let mut j = i;
            while j < chars.len() && (chars[j] == ' ' || chars[j] == '\t') {
                j += 1;
            }
            if j < chars.len() && chars[j] == '\n' {
                // trailing whitespace before a newline: drop it
            } else {
                out.extend(&chars[i..j]);
            }
            i = j;
        } else {
            out.push(c);
            i += 1;
        }
    }
    out
}

/// End with exactly one newline (no trailing blank lines), unless the file is
/// empty/whitespace-only. Preserve the file's newline style: a CRLF file keeps
/// its final line ending CRLF so we never produce a mixed-ending file.
fn normalize_final_newline(out: &mut String) {
    let trimmed_len = out.trim_end_matches(['\n', '\r', ' ', '\t']).len();
    if trimmed_len == 0 {
        return;
    }
    let crlf = out.contains("\r\n");
    out.truncate(trimmed_len);
    out.push_str(if crlf { "\r\n" } else { "\n" });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_source_has_no_lex_diagnostics() {
        assert!(lex_diagnostics("module M where\nfoo : Int\nfoo = 1\n").is_empty());
    }

    #[test]
    fn unterminated_string_is_diagnosed() {
        // Malformed input must be flagged so a format "success" is not mistaken
        // for parse success; output stays a verbatim passthrough.
        let src = "module M where\nx = \"oops\n";
        let diags = lex_diagnostics(src);
        assert!(!diags.is_empty(), "expected a diagnostic, got none");
        assert!(diags.iter().any(|d| d.contains("unterminated string")));
        assert_eq!(format_source(src), src); // byte-faithful passthrough
    }

    /// `render_from_ast` byte-span losslessness oracle over daml-fmt's own
    /// 924-file corpus (the SDK corpus the formatter is differential-tested
    /// against). This is the AST-span invariant the formatter relies on for
    /// verbatim span-slicing — a distinct check from the `format_ast` output
    /// differential (`test/diff.js`). It lives here, in the crate that owns
    /// `original/` and consumes the oracle, so daml-parser stays decoupled.
    /// Runs in CI; skips gracefully when `original/` is absent (a published
    /// crate off the workspace), but fails loud under CI so a missing/forgotten
    /// corpus can't pass green.
    #[test]
    fn render_from_ast_lossless_over_corpus() {
        use daml_parser::ast_span::render_from_ast;
        use daml_parser::parse::parse_module;
        use std::path::{Path, PathBuf};

        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("original");
        if !root.exists() {
            assert!(
                std::env::var_os("CI").is_none(),
                "corpus missing under CI (was crates/daml-fmt/original committed?): {}",
                root.display()
            );
            eprintln!("corpus absent (published crate?), skipping");
            return;
        }

        fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    collect(&p, out);
                } else if p.extension().is_some_and(|x| x == "daml") {
                    out.push(p);
                }
            }
        }

        let mut files = Vec::new();
        collect(&root, &mut files);
        assert!(
            files.len() > 800,
            "corpus incomplete: {} files",
            files.len()
        );

        // Both byte-span oracles over the 924-file SDK corpus: render_from_ast
        // (AST span reconstruction) and render_lossless (token+trivia
        // reconstruction). The token-level oracle is otherwise only exercised
        // over the smaller daml-finance corpus.
        let mut failures = Vec::new();
        for f in &files {
            let Ok(src) = std::fs::read_to_string(f) else {
                continue;
            };
            let (tokens, trivia, errors) = lex_with_trivia(&src);
            if let Err(e) = render_from_ast(&src, &parse_module(&src).0, &trivia) {
                failures.push(format!("render_from_ast {}: {}", f.display(), e));
            }
            // Lex errors drop bytes by design; losslessness is only promised for
            // files that lex clean (all 924 do).
            if errors.is_empty() {
                if let Err(e) = daml_parser::lexer::render_lossless(&src, &tokens, &trivia) {
                    failures.push(format!("render_lossless {}: {}", f.display(), e));
                }
            }
        }
        assert!(
            failures.is_empty(),
            "{} / {} files failed a span oracle:\n{}",
            failures.len(),
            files.len(),
            failures
                .iter()
                .take(20)
                .cloned()
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}
