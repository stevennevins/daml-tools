//! Layout resolution: insert virtual braces/semicolons per the Haskell
//! offside rule (adapted for DAML, where `with` also opens a layout block).
//!
//! Input is the raw token stream from the lexer; output is the same stream
//! with `VLBrace`/`VRBrace`/`VSemi` inserted, so the parser never has to
//! look at columns.

use crate::lexer::{Pos, Token, TokenKind};

/// Keywords that open a layout block. DAML adds `with` (template fields,
/// choice parameters, record construction) and `catch` (exception handler
/// alternatives) to Haskell's set.
fn layout_keyword(tok: &TokenKind) -> Option<LayoutKeyword> {
    match tok.keyword() {
        Some("where") => Some(LayoutKeyword::Where),
        Some("do") => Some(LayoutKeyword::Do),
        Some("of") => Some(LayoutKeyword::Of),
        Some("let") => Some(LayoutKeyword::Let),
        Some("with") => Some(LayoutKeyword::With),
        Some("catch") => Some(LayoutKeyword::Catch),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LayoutKeyword {
    Module,
    Where,
    Do,
    Of,
    Let,
    With,
    Catch,
}

#[derive(Debug)]
struct Context {
    /// Column of the block; 0 for an explicit `{ }` context (no offside).
    col: usize,
    /// Line the block was opened on (same-line `where` closure rule).
    line: usize,
    /// Keyword that opened it (`let` matters for the `in` rule).
    opened_by: LayoutKeyword,
    /// Bracket nesting depth at open time, so `)` can close blocks that
    /// were opened inside the parentheses: `(do stmts)`.
    bracket_depth: usize,
}

/// Resolve indentation-sensitive layout: insert virtual `VLBrace`/`VRBrace`/
/// `VSemi` tokens per the offside rule so the parser never inspects columns.
///
/// Input is the raw lexer token stream; the output preserves every input token
/// in order and interleaves the virtual layout tokens. This is a total function
/// over any token slice — it never panics, even on unbalanced or truncated
/// input (stray brackets and unclosed blocks are tolerated, not rejected).
///
/// ```
/// use daml_parser::lexer::lex;
/// use daml_parser::layout::resolve_layout;
///
/// let lexed = lex("module M where\nfoo = 1\n");
/// let laid_out = resolve_layout(lexed.tokens);
/// assert!(laid_out.len() >= 1);
/// ```
#[must_use]
pub fn resolve_layout(tokens: Vec<Token>) -> Vec<Token> {
    let eof_pos = tokens
        .last()
        .map(|t| t.pos)
        .unwrap_or(Pos { line: 1, column: 1 });
    let mut out: Vec<Token> = Vec::with_capacity(tokens.len() + tokens.len() / 4);
    let mut stack: Vec<Context> = Vec::new();
    let mut bracket_depth = 0usize;
    // Set after a layout keyword: the next token starts a block.
    let mut expecting_open: Option<LayoutKeyword> = None;
    let mut last_line = 0usize;

    // A file that doesn't start with `module` opens an implicit top context
    // at its first token's column (Haskell rule for missing module headers).
    if let Some(first) = tokens.first() {
        if !first.kind.is_keyword("module") {
            stack.push(Context {
                col: first.pos.column,
                line: first.pos.line,
                opened_by: LayoutKeyword::Module,
                bracket_depth: 0,
            });
            out.push(virtual_tok(TokenKind::VLBrace, first.pos));
            last_line = first.pos.line;
        }
    }

    let close = |out: &mut Vec<Token>, pos: Pos| {
        out.push(virtual_tok(TokenKind::VRBrace, pos));
    };

    for token in tokens {
        let pos = token.pos;
        let col = pos.column;

        if let Some(kw) = expecting_open.take() {
            if matches!(token.kind, TokenKind::LBrace) {
                // Explicit block: push a no-offside context.
                stack.push(Context {
                    col: 0,
                    line: pos.line,
                    opened_by: kw,
                    bracket_depth,
                });
                out.push(token);
                last_line = pos.line;
                continue;
            }
            let enclosing = stack.iter().rev().find(|c| c.col > 0).map_or(0, |c| c.col);
            if col > enclosing {
                stack.push(Context {
                    col,
                    line: pos.line,
                    opened_by: kw,
                    bracket_depth,
                });
                out.push(virtual_tok(TokenKind::VLBrace, pos));
                last_line = pos.line;
                // fall through to emit the token itself
            } else {
                // Token not indented past the enclosing block: empty block,
                // then let the normal offside logic below handle the token.
                out.push(virtual_tok(TokenKind::VLBrace, pos));
                out.push(virtual_tok(TokenKind::VRBrace, pos));
            }
        }

        // Offside check at the first token of each new line.
        if pos.line != last_line {
            while let Some(top) = stack.last() {
                if top.col > 0 && col < top.col {
                    if token.kind.is_keyword("in") && top.opened_by == LayoutKeyword::Let {
                        close(&mut out, pos);
                        stack.pop();
                        break;
                    }
                    close(&mut out, pos);
                    stack.pop();
                } else {
                    break;
                }
            }
            // `where` never starts a new block item — the rule below closes
            // the block instead, so a VSemi here would be orphaned.
            if let Some(top) = stack.last() {
                if top.col > 0 && col == top.col && !token.kind.is_keyword("where") {
                    out.push(virtual_tok(TokenKind::VSemi, pos));
                }
            }
            last_line = pos.line;
        }

        // `where` closes any block at or right of it (`do ... where` at the
        // same indentation must end the do-block; GHC handles this via the
        // parse-error rule).
        if token.kind.is_keyword("where") {
            while let Some(top) = stack.last() {
                // Close blocks at/right of the `where`, and with-blocks
                // opened on the same line (`template S with p : Party
                // where ...` — the inline with-block ends at the where).
                if top.col > 0
                    && (top.col >= col
                        || (top.line == pos.line && top.opened_by == LayoutKeyword::With))
                    && top.opened_by != LayoutKeyword::Module
                {
                    close(&mut out, pos);
                    stack.pop();
                } else {
                    break;
                }
            }
        }

        // `in` closes the matching `let` block when still open. Offside may
        // already have closed an inner `let` on this line; in that case the
        // current top context still decides whether this `in` belongs to an
        // enclosing `let`.
        if token.kind.is_keyword("in") {
            if let Some(top) = stack.last() {
                if top.col > 0
                    && top.opened_by == LayoutKeyword::Let
                    && (top.line == pos.line || col <= top.col)
                {
                    close(&mut out, pos);
                    stack.pop();
                }
            }
        }

        match token.kind {
            TokenKind::LParen | TokenKind::LBracket => bracket_depth += 1,
            TokenKind::RParen | TokenKind::RBracket | TokenKind::Comma => {
                // Close implicit blocks opened inside this bracket pair
                // before the bracket closes over them: `(do stmts)`,
                // `[f x, do y]`.
                let target = if matches!(token.kind, TokenKind::Comma) {
                    bracket_depth
                } else {
                    bracket_depth.saturating_sub(1)
                };
                while let Some(top) = stack.last() {
                    if top.col > 0 && top.bracket_depth > target {
                        close(&mut out, pos);
                        stack.pop();
                    } else {
                        break;
                    }
                }
                if !matches!(token.kind, TokenKind::Comma) {
                    bracket_depth = bracket_depth.saturating_sub(1);
                }
            }
            TokenKind::RBrace
                // Close an explicit context if one is on top.
                if stack.last().is_some_and(|c| c.col == 0) => {
                    stack.pop();
                }
            _ => {}
        }

        let was_backslash = out
            .last()
            .is_some_and(|t| matches!(&t.kind, TokenKind::Op(o) if *o == "\\"));
        let opens_layout = layout_keyword(&token.kind);
        let case_after_backslash = token.kind.is_keyword("case") && was_backslash;
        out.push(token);

        if let Some(keyword) = opens_layout {
            expecting_open = Some(keyword);
        } else if case_after_backslash {
            // `\case` alternatives form a layout block like `of`.
            expecting_open = Some(LayoutKeyword::Of);
        }
    }

    // EOF closes everything implicit.
    if expecting_open.is_some() {
        out.push(virtual_tok(TokenKind::VLBrace, eof_pos));
        out.push(virtual_tok(TokenKind::VRBrace, eof_pos));
    }
    for ctx in stack.iter().rev() {
        if ctx.col > 0 {
            out.push(virtual_tok(TokenKind::VRBrace, eof_pos));
        }
    }

    out
}

const fn virtual_tok(tok: TokenKind, pos: Pos) -> Token {
    // Layout tokens have no source bytes; zero-width span keeps the
    // lossless render (which skips them anyway) honest.
    Token {
        kind: tok,
        pos,
        start: 0,
        end: 0,
    }
}

// Virtual-brace layout invariants for the layout resolver phase.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::lex;

    /// Render the laid-out stream compactly: `{` `}` `;` for virtual tokens,
    /// token text otherwise.
    fn layout_str(src: &str) -> String {
        let (tokens, errors) = lex(src).into_parts();
        assert!(errors.is_empty(), "lex errors: {errors:?}");
        resolve_layout(tokens)
            .iter()
            .map(|t| match &t.kind {
                TokenKind::VLBrace => "{".to_string(),
                TokenKind::VRBrace => "}".to_string(),
                TokenKind::VSemi => ";".to_string(),
                TokenKind::LowerId { qualifier, name } | TokenKind::UpperId { qualifier, name } => {
                    qualifier
                        .as_ref()
                        .map_or_else(|| name.to_string(), |q| format!("{q}.{name}"))
                }
                TokenKind::Op(o) => o.to_string(),
                TokenKind::IntLit(n) | TokenKind::DecimalLit(n) => n.clone(),
                TokenKind::StringLit(s) => format!("{s:?}"),
                TokenKind::CharLit(c) => format!("'{c}'"),
                TokenKind::LParen => "(".into(),
                TokenKind::RParen => ")".into(),
                TokenKind::LBracket => "[".into(),
                TokenKind::RBracket => "]".into(),
                TokenKind::LBrace => "{{".into(),
                TokenKind::RBrace => "}}".into(),
                TokenKind::Comma => ",".into(),
                TokenKind::Semi => ";;".into(),
                TokenKind::Backtick => "`".into(),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn module_where_opens_top_block() {
        assert_eq!(
            layout_str("module M where\n\nf = 1\ng = 2\n"),
            "module M where { f = 1 ; g = 2 }"
        );
    }

    #[test]
    fn template_with_where_blocks() {
        let src = "module M where\n\ntemplate Foo\n  with\n    x : Int\n    y : Party\n  where\n    signatory y\n";
        assert_eq!(
            layout_str(src),
            "module M where { template Foo with { x : Int ; y : Party } where { signatory y } }"
        );
    }

    #[test]
    fn do_block_and_dedent() {
        let src = "module M where\nf = do\n  a\n  b\ng = 1\n";
        assert_eq!(
            layout_str(src),
            "module M where { f = do { a ; b } ; g = 1 }"
        );
    }

    #[test]
    fn same_line_record_with() {
        let src = "module M where\nf = do\n  cid <- create this with owner = p\n  pure cid\n";
        assert_eq!(
            layout_str(src),
            "module M where { f = do { cid <- create this with { owner = p } ; pure cid } }"
        );
    }

    #[test]
    fn let_in_same_line() {
        assert_eq!(
            layout_str("module M where\nf = let x = 1 in x\n"),
            "module M where { f = let { x = 1 } in x }"
        );
    }

    #[test]
    fn let_in_multiline() {
        let src = "module M where\nf =\n  let x = 1\n      y = 2\n  in x\n";
        assert_eq!(
            layout_str(src),
            "module M where { f = let { x = 1 ; y = 2 } in x }"
        );
    }

    #[test]
    fn paren_closes_do_block() {
        assert_eq!(
            layout_str("module M where\nf = g (do\n  a) b\n"),
            "module M where { f = g ( do { a } ) b }"
        );
    }

    #[test]
    fn where_at_do_indent_closes_do() {
        let src = "module M where\nf = do\n  a\n  where\n    g = 1\n";
        assert_eq!(
            layout_str(src),
            "module M where { f = do { a } where { g = 1 } }"
        );
    }

    #[test]
    fn in_closes_enclosing_let_after_inner_let_closes_offside() {
        let src = "module M where\nf = let\n  let x = 1\n      y = 2\nin x\n";
        assert_eq!(
            layout_str(src),
            "module M where { f = let { let { x = 1 ; y = 2 } } in x }"
        );
    }

    #[test]
    fn file_without_module_header() {
        assert_eq!(layout_str("f = 1\ng = 2\n"), "{ f = 1 ; g = 2 }");
    }

    /// Phase gate: lexer + layout must survive the whole daml-finance
    /// corpus (no panic, no hang, balanced virtual braces). The corpus is
    /// vendored once at the workspace root, shared with daml-lint.
    #[test]
    fn corpus_lex_and_layout_survives() {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../corpus/daml-finance/daml");
        if !root.exists() {
            // Corpus absent (e.g. a published crate built outside the
            // workspace): skip rather than panic. In CI, fail loud so a
            // missing vendored corpus cannot pass green.
            assert!(
                std::env::var_os("CI").is_none(),
                "vendored corpus missing under CI (was it committed?): {}",
                root.display()
            );
            eprintln!("corpus absent (published crate?), skipping");
            return;
        }
        let mut files = Vec::new();
        collect_daml(&root, &mut files).expect("collect corpus files");
        assert!(
            files.len() > 600,
            "corpus incomplete: {} files",
            files.len()
        );
        let mut lex_errors = 0usize;
        for f in &files {
            let src = std::fs::read_to_string(f)
                .unwrap_or_else(|e| panic!("failed to read corpus file {}: {e}", f.display()));
            let (tokens, errors) = lex(&src).into_parts();
            lex_errors += errors.len();
            let laid = resolve_layout(tokens);
            let opens = laid.iter().filter(|t| t.kind == TokenKind::VLBrace).count();
            let closes = laid.iter().filter(|t| t.kind == TokenKind::VRBrace).count();
            assert_eq!(opens, closes, "unbalanced virtual braces in {f:?}");
        }
        assert_eq!(lex_errors, 0, "lex errors across corpus");
    }

    fn collect_daml(
        dir: &std::path::Path,
        out: &mut Vec<std::path::PathBuf>,
    ) -> std::io::Result<()> {
        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let p = entry.path();
            if p.is_dir() {
                collect_daml(&p, out)?;
            } else if p.extension().is_some_and(|e| e == "daml") {
                out.push(p);
            }
        }
        Ok(())
    }

    #[test]
    fn case_of_alternatives() {
        let src = "module M where\nf x = case x of\n  1 -> a\n  _ -> b\n";
        assert_eq!(
            layout_str(src),
            "module M where { f x = case x of { 1 -> a ; _ -> b } }"
        );
    }
}
