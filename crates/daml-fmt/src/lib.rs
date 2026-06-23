//! daml-fmt: a code formatter for Daml, built on the daml-parser pipeline.
//!
//! Strategy: reconstruct source from parser spans and the lossless
//! token+trivia stream, never inside string or comment spans (CLAUDE.md:
//! comments are sacred). Pure reindent and whitespace candidates pass through a
//! token-equivalence gate: re-lex and require the laid-out token stream,
//! including Daml's virtual layout tokens (`VLBrace`/`VSemi`/`VRBrace`), to
//! match their immediate input. Layout-organizing rewrites intentionally change
//! layout shape, so the corpus desugar oracle and idempotence checks are the
//! safety bar for those rules.
//!
//! The shipping backend is `layout_ast` (AST-driven, own-design canonical
//! layout, and NOT aimed at matching an external formatter baseline).
//! `normalize_gaps` below is the proven, token-gated whitespace + colon-spacing
//! pass it composes on top of the structural reindent:
//! - trailing-whitespace: strip spaces/tabs before a newline; one final newline.
//! - colon-spacing: `name : Type` -> `name: Type` (drop same-line spaces before a
//!   lone `:` type-annotation colon; never `::`, never a line-leading colon).
//!
//! # Example
//!
//! ```
//! let src = "module M where\nfoo : Int\nfoo = 1\n";
//! let formatted = daml_fmt::format_source(src);
//!
//! assert_eq!(formatted, "module M where\nfoo: Int\nfoo = 1\n");
//! ```

// AST-driven layout (own-design canonical layout). This is the shipping
// backend. See src/layout_ast.rs.
mod layout_ast;

use daml_parser::lexer::{TokenKind, TriviaKind};
use daml_syntax::{SourceFile, SourceTokens};

/// Lexer diagnostics for `src`.
///
/// Returns one `line:col: message` string per error (e.g. unterminated string /
/// block comment). Empty when the source lexes clean. The formatter still passes
/// malformed input through verbatim (`format_source` is byte-safe); the CLI uses
/// this to surface a non-zero exit + diagnostic so a formatter "success" is not
/// mistaken for parse success. All 924 corpus files lex clean, so this never
/// flags them.
pub fn lex_diagnostics(src: &str) -> Vec<String> {
    SourceTokens::lex(src)
        .lex_errors()
        .iter()
        .map(|error| format!("{}:{}: {}", error.pos.line, error.pos.column, error))
        .collect()
}

/// Source diagnostics for `src`, including lexical and parser diagnostics.
///
/// CPP-conditional source is treated specially: Daml SDK sources can contain
/// both active and inactive `#if`/`#else` module branches. The parser does not
/// preprocess those branches, so parser recovery diagnostics there are not a
/// reliable signal that formatter input is malformed. Lexical diagnostics are
/// still reported.
///
/// Returns one `line:col: [category] message` string per error.
pub fn source_diagnostics(src: &str) -> Vec<String> {
    if has_cpp_conditionals(src) {
        return lex_diagnostics(src);
    }

    SourceFile::parse(src)
        .diagnostics()
        .iter()
        .map(|diagnostic| {
            format!(
                "{}:{}: [{}] {}",
                diagnostic.line,
                diagnostic.column,
                diagnostic.category.as_str(),
                diagnostic.message
            )
        })
        .collect()
}

fn has_cpp_conditionals(src: &str) -> bool {
    src.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with("#if") || line.starts_with("#else") || line.starts_with("#endif")
    })
}

/// Import ordering strategy for formatter output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImportOrder {
    /// Sort import declarations into formatter-defined groups.
    Organize,
    /// Preserve declaration order exactly as written by the source.
    Preserve,
}

/// Formatter behavior switches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatOptions {
    /// How formatter handles import declarations:
    ///
    /// * `Organize` groups/sorts imports into canonical formatter order.
    /// * `Preserve` keeps original declaration order.
    ///
    /// Reordering imports can change package identity even when the source-level
    /// declarations denote the same imports; use `--preserve-import-order` in the
    /// CLI when package identity stability matters more than import
    /// organization.
    pub import_order: ImportOrder,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            import_order: ImportOrder::Organize,
        }
    }
}

impl FormatOptions {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            import_order: ImportOrder::Organize,
        }
    }

    #[must_use]
    pub const fn with_import_order(mut self, import_order: ImportOrder) -> Self {
        self.import_order = import_order;
        self
    }
}

/// Format Daml source with default formatter options.
///
/// Delegates to the AST-driven backend (`layout_ast::format_ast`): an
/// own-design canonical layout that reindents modeled AST constructs, applies
/// layout-organizing rules, token-gated whitespace/blank-line/colon-spacing
/// normalization, and passes unmodeled constructs through verbatim.
pub fn format_source(src: &str) -> String {
    format_source_with_options(src, FormatOptions::default())
}

/// Format Daml source with explicit formatter options.
pub fn format_source_with_options(src: &str, options: FormatOptions) -> String {
    layout_ast::format_ast(src, options)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatCoverage {
    pub formatted: usize,
    pub total: usize,
}

/// Count AST formatter structural edit candidates over modeled constructs.
pub fn coverage(src: &str) -> FormatCoverage {
    layout_ast::coverage(src)
}

/// Reconstruct `src`, normalizing gap whitespace. With `colon`, also drop
/// same-line spaces before a lone `:` token. Shared with the AST backend
/// (`layout_ast`) so both paths apply the same proven, token-gated spacing.
pub(crate) fn normalize_gaps(src: &str, colon: bool) -> String {
    rewrite(src, colon)
}

fn rewrite(src: &str, colon: bool) -> String {
    let source_tokens = SourceTokens::lex(src);

    // Items that carry bytes, in source order. For each: brace-depth delta
    // (+1/-1 for `{`/`}`/parens), is-lone-colon, is-rparen, is-token (vs trivia).
    let mut items: Vec<(usize, usize, i32, bool, bool, bool)> = source_tokens
        .tokens()
        .iter()
        .filter(|t| {
            !matches!(
                t.kind(),
                TokenKind::VLBrace | TokenKind::VRBrace | TokenKind::VSemi
            )
        })
        .map(|t| {
            (
                t.start(),
                t.end(),
                brace_delta(t.kind()),
                is_lone_colon(t.kind()),
                matches!(t.kind(), TokenKind::RParen),
                true,
            )
        })
        .chain(
            source_tokens
                .trivia()
                .iter()
                .filter(|t| !matches!(t.kind(), TriviaKind::BlankLines(_)))
                .map(|t| (t.start(), t.end(), 0, false, false, false)),
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
            out.push_str(&collapse_blank_lines(&strip_trailing_ws(gap)));
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
    out.push_str(&collapse_blank_lines(&strip_trailing_ws(tail)));

    normalize_final_newline(&mut out);
    out
}

fn is_lone_colon(t: &TokenKind) -> bool {
    matches!(t, TokenKind::Op(op) if op.as_str() == ":")
}

const fn brace_delta(t: &TokenKind) -> i32 {
    match t {
        TokenKind::LBrace | TokenKind::LParen => 1,
        TokenKind::RBrace | TokenKind::RParen => -1,
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

/// Collapse interior blank-line runs to at most one blank line (two line
/// endings), preserving LF vs CRLF inside the whitespace-only gap.
fn collapse_blank_lines(gap: &str) -> String {
    let mut out = String::with_capacity(gap.len());
    let mut i = 0;
    let mut newline_run = 0usize;
    while i < gap.len() {
        let rest = &gap[i..];
        let (line_ending, width) = if rest.starts_with("\r\n") {
            ("\r\n", 2)
        } else if rest.starts_with('\n') {
            ("\n", 1)
        } else {
            let ch = rest.chars().next().expect("non-empty rest");
            out.push(ch);
            newline_run = 0;
            i += ch.len_utf8();
            continue;
        };
        newline_run += 1;
        if newline_run <= 2 {
            out.push_str(line_ending);
        }
        i += width;
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
    fn format_options_can_be_created_and_built() {
        let options = FormatOptions::new().with_import_order(ImportOrder::Preserve);

        assert_eq!(options.import_order, ImportOrder::Preserve);
    }

    #[test]
    fn clean_source_has_no_lex_diagnostics() {
        assert!(lex_diagnostics("module M where\nfoo : Int\nfoo = 1\n").is_empty());
    }

    #[test]
    fn parser_diagnostics_are_reported() {
        let src = "module M where\nfoo = if x then 1\n";
        assert!(!source_diagnostics(src).is_empty());
    }

    #[test]
    fn cpp_conditionals_do_not_surface_parser_recovery_diagnostics() {
        let src =
            "module A where\n#if defined(foo)\nmodule B where\n#else\nmodule C where\n#endif\n";
        assert!(source_diagnostics(src).is_empty());
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
        use daml_syntax::SourceFile;
        use std::path::{Path, PathBuf};

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
            let source_file = SourceFile::parse(&src);
            if let Err(e) = render_from_ast(&src, source_file.module(), source_file.trivia()) {
                failures.push(format!("render_from_ast {}: {}", f.display(), e));
            }
            // Lex errors drop bytes by design; losslessness is only promised for
            // files that lex clean (all 924 do).
            if source_file
                .diagnostics()
                .iter()
                .all(|diagnostic| diagnostic.category != daml_parser::ast::DiagnosticCategory::Lex)
            {
                if let Err(e) = daml_parser::lexer::render_lossless(
                    &src,
                    source_file.tokens(),
                    source_file.trivia(),
                ) {
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

    #[test]
    fn interior_blank_runs_collapse_to_one_blank_line() {
        let src = "module M where\n\n\n\nx = 1\n";
        assert_eq!(format_source(src), "module M where\n\nx = 1\n");
    }

    #[test]
    fn gap_cases_format_to_expected_output() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("corpus/gap-cases");
        let bad_dir = root.join("bad");
        let good_dir = root.join("good");
        if !bad_dir.exists() || !good_dir.exists() {
            eprintln!("gap cases corpus missing (published crate test fixture), skipping");
            return;
        }
        let mut checked = 0usize;
        for entry in std::fs::read_dir(&bad_dir).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_none_or(|ext| ext != "daml") {
                continue;
            }
            let name = path.file_name().unwrap();
            let bad = std::fs::read_to_string(&path).unwrap();
            let good = std::fs::read_to_string(good_dir.join(name)).unwrap();
            let formatted = format_source(&bad);
            assert_eq!(formatted, good, "gap fixture mismatch: {}", path.display());
            assert_eq!(
                format_source(&good),
                good,
                "gap fixture not idempotent: {}",
                path.display()
            );
            checked += 1;
        }
        assert_eq!(checked, 9, "unexpected gap fixture count");
    }
}
