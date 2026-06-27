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
//!
//! # Formatter options
//!
//! ```
//! use daml_fmt::{FormatOptions, ImportOrder, format_source_with_options};
//!
//! let src = "module M where\nimport DA.Optional\nimport DA.List\n\nx = []\n";
//!
//! // Default: organize imports.
//! let organized = format_source_with_options(src, FormatOptions::default());
//!
//! // Preserve declaration order when package identity must stay stable.
//! let preserved = format_source_with_options(
//!     src,
//!     FormatOptions::new().with_import_order(ImportOrder::Preserve),
//! );
//!
//! assert_eq!(organized, "module M where\nimport DA.List\nimport DA.Optional\n\nx = []\n");
//! assert_eq!(preserved, src);
//! ```
//!
//! ## API posture
//!
//! This crate is pre-1.0. [`ImportOrder`] is `#[non_exhaustive]` so downstream
//! `match` arms stay forward-compatible when new import strategies appear; use
//! [`Default`] for the standard strategy and [`std::fmt::Display`] for stable
//! user-facing labels.
//! [`FormatOptions`] uses private fields and `with_*` helpers so new switches can
//! ship with defaults without breaking downstream struct literals.

// AST-driven layout (own-design canonical layout). This is the shipping
// backend. See src/layout_ast.rs.
pub mod config;
mod format_rules;
mod layout_ast;

use daml_parser::ast::DiagnosticCategory;
use daml_parser::lexer::{TokenKind, TriviaKind};
use daml_syntax::{CharColumn, LineNumber, SourceFile, SourceTokens};
pub use format_rules::{FormatRule, FormatRuleParseError, FormatRuleSet};
use std::error::Error;
use std::fmt;

/// A formatter input diagnostic with typed location and category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatDiagnostic {
    line: LineNumber,
    column: CharColumn,
    category: DiagnosticCategory,
    message: String,
}

impl FormatDiagnostic {
    /// 1-based line number of the diagnostic start.
    #[must_use]
    pub const fn line(&self) -> LineNumber {
        self.line
    }

    /// 1-based character column of the diagnostic start.
    #[must_use]
    pub const fn column(&self) -> CharColumn {
        self.column
    }

    /// Parser diagnostic category from `daml-parser`.
    #[must_use]
    pub const fn category(&self) -> DiagnosticCategory {
        self.category
    }

    /// Human-readable diagnostic message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for FormatDiagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.category == DiagnosticCategory::Lex {
            write!(f, "{}:{}: {}", self.line, self.column, self.message)
        } else {
            write!(
                f,
                "{}:{}: [{}] {}",
                self.line, self.column, self.category, self.message
            )
        }
    }
}

/// Formatting failed because the source has lexical or parser diagnostics.
///
/// Use [`FormatError::diagnostics`] for readable call sites or
/// [`AsRef`]<[`FormatDiagnostic`]> when passing the diagnostic slice to generic
/// APIs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatError {
    diagnostics: Vec<FormatDiagnostic>,
}

impl FormatError {
    /// Typed diagnostics explaining why formatting was rejected.
    #[must_use]
    pub fn diagnostics(&self) -> &[FormatDiagnostic] {
        &self.diagnostics
    }
}

impl AsRef<[FormatDiagnostic]> for FormatError {
    fn as_ref(&self) -> &[FormatDiagnostic] {
        self.diagnostics()
    }
}

impl fmt::Display for FormatError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index > 0 {
                f.write_str("\n")?;
            }
            diagnostic.fmt(f)?;
        }
        Ok(())
    }
}

impl Error for FormatError {}

/// Lexer diagnostics for `src`.
///
/// Empty when the source lexes clean. The formatter still passes malformed input
/// through verbatim ([`format_source`] is byte-safe); callers that need a typed
/// failure should use [`try_format_source_with_options`].
#[must_use]
pub fn lex_diagnostics(src: &str) -> Vec<FormatDiagnostic> {
    collect_lex_diagnostics(src)
}

/// Source diagnostics for `src`, including lexical and parser diagnostics.
///
/// CPP-conditional source is treated specially: Daml SDK sources can contain
/// both active and inactive `#if`/`#else` module branches. The parser does not
/// preprocess those branches, so skipped-declaration recovery there is not a
/// reliable signal that formatter input is malformed. Active malformed parser
/// diagnostics and lexical diagnostics are still reported.
#[must_use]
pub fn source_diagnostics(src: &str) -> Vec<FormatDiagnostic> {
    let cpp_module_header_ranges = cpp_conditional_module_header_ranges(src);
    let suppress_cpp_recovery = has_cpp_conditionals(src);
    let source_line_count = src.lines().count();

    SourceFile::parse(src)
        .diagnostics()
        .iter()
        .filter(|diagnostic| {
            !suppress_cpp_recovery
                || !is_cpp_suppressed_parser_diagnostic(
                    diagnostic,
                    src,
                    &cpp_module_header_ranges,
                    source_line_count,
                )
        })
        .map(|diagnostic| FormatDiagnostic {
            line: diagnostic.line(),
            column: diagnostic.column(),
            category: diagnostic.category(),
            message: diagnostic.message().to_string(),
        })
        .collect()
}

fn collect_lex_diagnostics(src: &str) -> Vec<FormatDiagnostic> {
    SourceTokens::lex(src)
        .lex_errors()
        .iter()
        .map(|error| FormatDiagnostic {
            line: LineNumber::new(error.pos.line),
            column: CharColumn::new(error.pos.column),
            category: DiagnosticCategory::Lex,
            message: error.to_string(),
        })
        .collect()
}

fn has_cpp_conditionals(src: &str) -> bool {
    src.lines().any(|line| {
        let line = line.trim_start();
        line.starts_with("#if")
            || line.starts_with("#elif")
            || line.starts_with("#else")
            || line.starts_with("#endif")
    })
}

fn cpp_conditional_module_header_ranges(src: &str) -> Vec<std::ops::RangeInclusive<usize>> {
    let mut ranges = Vec::new();
    let mut cpp_depth = 0usize;
    let mut module_header_start = None;

    for (index, line) in src.lines().enumerate() {
        let line_no = index + 1;
        let trimmed = line.trim_start();

        if let Some(start) = module_header_start {
            if is_cpp_branch_boundary(trimmed) {
                ranges.push(start..=line_no.saturating_sub(1));
                module_header_start = None;
            } else if module_header_ends(trimmed) {
                ranges.push(start..=line_no);
                module_header_start = None;
                continue;
            } else {
                continue;
            }
        }

        if trimmed.starts_with("#if") {
            cpp_depth += 1;
            continue;
        }
        if trimmed.starts_with("#endif") {
            cpp_depth = cpp_depth.saturating_sub(1);
            continue;
        }

        if cpp_depth > 0 && trimmed.starts_with("module ") {
            if module_header_ends(trimmed) {
                ranges.push(line_no..=line_no);
            } else {
                module_header_start = Some(line_no);
            }
        }
    }

    if let Some(start) = module_header_start {
        let end = src.lines().count().max(start);
        ranges.push(start..=end);
    }

    ranges
}

fn is_cpp_branch_boundary(line: &str) -> bool {
    line.starts_with("#elif") || line.starts_with("#else") || line.starts_with("#endif")
}

fn module_header_ends(line: &str) -> bool {
    line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '\''))
        .any(|token| token == "where")
}

fn is_cpp_suppressed_parser_diagnostic(
    diagnostic: &daml_syntax::Diagnostic,
    src: &str,
    cpp_module_header_ranges: &[std::ops::RangeInclusive<usize>],
    source_line_count: usize,
) -> bool {
    if diagnostic.category() == DiagnosticCategory::SkippedDecl {
        let line = diagnostic.line().get();
        return cpp_module_header_ranges
            .iter()
            .any(|range| range.contains(&line));
    }
    if diagnostic.category() == DiagnosticCategory::Malformed {
        let line = diagnostic.line().get();
        let in_cpp_module_header = cpp_module_header_ranges
            .iter()
            .any(|range| range.contains(&line));
        let eof_recovery_artifact = line > source_line_count;
        if in_cpp_module_header || eof_recovery_artifact {
            return matches!(
                diagnostic.message(),
                "bad parameter pattern in 'module'" | "bad parameter pattern in 'where'"
            );
        }
        let Some(source_line) = src.lines().nth(line.saturating_sub(1)) else {
            return matches!(
                diagnostic.message(),
                "bad parameter pattern in 'module'" | "bad parameter pattern in 'where'"
            );
        };
        match diagnostic.message() {
            "bad parameter pattern in 'module'" => !looks_like_module_declaration(source_line),
            "bad parameter pattern in 'where'" => !contains_where_keyword(source_line),
            _ => false,
        }
    } else {
        false
    }
}

fn looks_like_module_declaration(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("module ")
}

fn contains_where_keyword(line: &str) -> bool {
    line.split(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '\''))
        .any(|token| token == "where")
}

/// Import ordering strategy for formatter output.
///
/// The default strategy is [`ImportOrder::Organize`]. Its [`fmt::Display`]
/// implementation returns stable lowercase labels (`organize`, `preserve`) for
/// logs, configuration summaries, and CLI messages.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImportOrder {
    /// Sort import declarations into formatter-defined groups.
    #[default]
    Organize,
    /// Preserve declaration order exactly as written by the source.
    Preserve,
}

impl fmt::Display for ImportOrder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Organize => f.write_str("organize"),
            Self::Preserve => f.write_str("preserve"),
        }
    }
}

/// Formatter behavior switches.
///
/// Prefer [`Default`], [`FormatOptions::new`], or the `with_*` helpers when
/// constructing options so new fields can ship with defaults without breaking
/// call sites.
///
/// # Examples
///
/// ```
/// use daml_fmt::{FormatOptions, ImportOrder};
///
/// let from_default = FormatOptions::default();
/// let preserved = FormatOptions::new().with_import_order(ImportOrder::Preserve);
///
/// assert_eq!(preserved.import_order(), ImportOrder::Preserve);
/// assert_ne!(from_default.import_order(), preserved.import_order());
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FormatOptions {
    import_order: ImportOrder,
    rules: FormatRuleSet,
}

impl FormatOptions {
    /// Create formatter options with the default organizing configuration.
    ///
    /// This is equivalent to [`Default::default`] and currently uses
    /// [`ImportOrder::Organize`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            import_order: ImportOrder::Organize,
            rules: FormatRuleSet::all(),
        }
    }

    /// How the formatter handles import declarations.
    ///
    /// * [`ImportOrder::Organize`] groups/sorts imports into canonical formatter order.
    /// * [`ImportOrder::Preserve`] keeps original declaration order.
    ///
    /// Reordering imports can change package identity even when the source-level
    /// declarations denote the same imports; use `--preserve-import-order` in the
    /// CLI when package identity stability matters more than import organization.
    #[must_use]
    pub const fn import_order(self) -> ImportOrder {
        self.import_order
    }

    /// Formatter rules enabled for this formatting run.
    #[must_use]
    pub const fn rules(self) -> FormatRuleSet {
        self.rules
    }

    /// Set the formatter rules enabled for this formatting run.
    #[must_use]
    pub const fn with_rules(mut self, rules: FormatRuleSet) -> Self {
        self.rules = rules;
        self
    }

    /// Set the import ordering strategy.
    ///
    /// See [`FormatOptions::import_order`] for the package-identity warning
    /// when imports are reordered.
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
#[must_use]
pub fn format_source(src: &str) -> String {
    format_source_with_options(src, FormatOptions::default())
}

/// Format Daml source with explicit formatter options.
///
/// Malformed input is formatted as a byte-faithful passthrough. Use
/// [`try_format_source_with_options`] when callers need a typed error instead.
#[must_use]
pub fn format_source_with_options(src: &str, options: FormatOptions) -> String {
    layout_ast::format_ast(src, options)
}

/// Format Daml source with explicit formatter options, rejecting malformed input.
///
/// Returns [`FormatError`] with typed [`FormatDiagnostic`] entries when
/// [`source_diagnostics`] reports lexical or parser diagnostics. CPP-conditional
/// skipped-declaration recovery diagnostics and alternate `module` declarations
/// inside inactive CPP branches are ignored by [`source_diagnostics`], while
/// active malformed parser diagnostics and lexical diagnostics are still
/// rejected.
///
/// # Errors
///
/// Returns [`FormatError`] when `src` produces diagnostics reported by
/// [`source_diagnostics`].
pub fn try_format_source_with_options(
    src: &str,
    options: FormatOptions,
) -> Result<String, FormatError> {
    reject_source_diagnostics(src)?;
    Ok(layout_ast::format_ast(src, options))
}

/// Format Daml source with default formatter options, rejecting malformed input.
///
/// # Errors
///
/// Returns [`FormatError`] when `src` produces diagnostics reported by
/// [`source_diagnostics`].
///
/// ```
/// use daml_fmt::try_format_source;
///
/// let ok = try_format_source("module M where\nfoo: Int\nfoo = 1\n");
/// assert!(ok.is_ok());
///
/// let err = try_format_source("module M where\n@@@\n");
/// assert!(err.is_err());
/// assert!(!err.unwrap_err().diagnostics().is_empty());
/// ```
pub fn try_format_source(src: &str) -> Result<String, FormatError> {
    try_format_source_with_options(src, FormatOptions::default())
}

/// Structural formatter coverage over modeled constructs.
///
/// This is not a normalized ratio: one construct can produce multiple edit
/// candidates, and on an already-canonical corpus most modeled constructs are
/// no-ops.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormatCoverage {
    edit_candidates: usize,
    modeled_constructs: usize,
}

impl FormatCoverage {
    /// Count of structural edit candidates the formatter would apply.
    #[must_use]
    pub const fn edit_candidates(self) -> usize {
        self.edit_candidates
    }

    /// Count of modeled constructs walked by the coverage metric.
    #[must_use]
    pub const fn modeled_constructs(self) -> usize {
        self.modeled_constructs
    }
}

/// Count AST formatter structural edit candidates over modeled constructs.
///
/// # Errors
///
/// Returns [`FormatError`] when `src` produces diagnostics reported by
/// [`source_diagnostics`].
pub fn coverage(src: &str) -> Result<FormatCoverage, FormatError> {
    reject_source_diagnostics(src)?;
    Ok(layout_ast::coverage(src))
}

fn reject_source_diagnostics(src: &str) -> Result<(), FormatError> {
    let diagnostics = source_diagnostics(src);
    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(FormatError { diagnostics })
    }
}

/// Reconstruct `src`, normalizing gap whitespace. With `colon`, also drop
/// same-line spaces before a lone `:` token. Shared with the AST backend
/// (`layout_ast`) so both paths apply the same proven, token-gated spacing.
pub(crate) fn normalize_gaps(src: &str, mode: ColonSpacingMode) -> String {
    rewrite(src, mode)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColonSpacingMode {
    Canonical,
    Preserve,
}

impl ColonSpacingMode {
    const fn do_canonicalize_colons(&self) -> bool {
        matches!(self, Self::Canonical)
    }
}

#[derive(Debug, Clone, Copy)]
struct GapTokenSpan {
    start: usize,
    end: usize,
    brace_depth_delta: i32,
    is_lone_colon: bool,
    is_rparen: bool,
    is_token: bool,
}

fn rewrite(src: &str, mode: ColonSpacingMode) -> String {
    let source_tokens = SourceTokens::lex(src);

    // Items that carry bytes, in source order. For each: brace-depth delta
    // (+1/-1 for `{`/`}`/parens), is-lone-colon, is-rparen, is-token (vs trivia).
    let mut items: Vec<GapTokenSpan> = source_tokens
        .tokens()
        .iter()
        .filter(|t| {
            !matches!(
                t.kind(),
                TokenKind::VLBrace | TokenKind::VRBrace | TokenKind::VSemi
            )
        })
        .map(|t| GapTokenSpan {
            start: t.start().get(),
            end: t.end().get(),
            brace_depth_delta: brace_delta(t.kind()),
            is_lone_colon: is_lone_colon(t.kind()),
            is_rparen: matches!(t.kind(), TokenKind::RParen),
            is_token: true,
        })
        .chain(
            source_tokens
                .trivia()
                .iter()
                .filter(|t| !matches!(t.kind(), TriviaKind::BlankLines(_)))
                .map(|t| GapTokenSpan {
                    start: t.start().get(),
                    end: t.end().get(),
                    brace_depth_delta: 0,
                    is_lone_colon: false,
                    is_rparen: false,
                    is_token: false,
                }),
        )
        .collect();
    items.sort_by_key(|item| item.start);

    let mut out = String::with_capacity(src.len());
    let mut prev = 0usize;
    let mut brace_depth: i32 = 0;
    let mut prev_was_rparen = false;
    // True when the previously-emitted token was a type-annotation colon we
    // canonicalized (same gate as the before-colon collapse). Lets us collapse
    // a duplicate space *after* that colon (`x:  T` -> `x: T`) symmetrically.
    let mut prev_was_canon_colon = false;
    for item in items {
        let GapTokenSpan {
            start,
            end,
            brace_depth_delta: delta,
            is_lone_colon: is_colon,
            is_rparen,
            is_token,
        } = item;
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
        let this_is_canon_colon =
            mode.do_canonicalize_colons() && is_colon && brace_depth == 0 && !prev_was_rparen;
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
    let mut out = String::with_capacity(gap.len());
    let bytes = gap.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b' ' || b == b'\t' {
            let mut j = i + 1;
            while j < bytes.len() && (bytes[j] == b' ' || bytes[j] == b'\t') {
                j += 1;
            }
            if !matches!(bytes.get(j), Some(b'\n' | b'\r')) {
                out.push_str(&gap[i..j]);
            }
            i = j;
        } else {
            let ch = gap[i..].chars().next().expect("non-empty gap substring");
            let width = ch.len_utf8();
            if width == 1 {
                out.push(ch);
                i += 1;
            } else {
                out.push_str(&gap[i..i + width]);
                i += width;
            }
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
