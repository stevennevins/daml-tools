//! Recursive-descent parser: laid-out token stream → typed AST (src/ast.rs).
//!
//! Error recovery is per-declaration: an unparseable declaration becomes
//! `Decl::Unknown` plus a diagnostic, and parsing continues at the next
//! virtual semicolon. The parser never panics and never aborts the file.

use crate::ast::{
    Alt, Binding, ChoiceDecl, Consuming, Decl, DoStmt, Equation, ExpectedToken, Expr, FieldAssign,
    FieldDecl, FixityAssoc, FixityDecl, FixityTarget, FunctionDecl, Identifier, ImportDecl,
    ImportStyle, InterfaceDecl, InterfaceInstanceDecl, LitKind, MalformedSyntaxKind, Module,
    ModuleName, Operator, ParseDiagnostic, ParseDiagnosticKind, Pat, SkippedDeclarationReason,
    Span, TemplateBodyDecl, TemplateDecl, Type, TypeAnnotation, TypeAnnotationContext,
    UnsupportedSyntaxKind,
};
use crate::layout::resolve_layout;
use crate::lexer::{lex, Pos, Token, TokenKind};
use std::collections::HashMap;

pub const MAX_RECURSION_DEPTH: u32 = 128;

/// Result of tolerant module parsing.
///
/// The parser always returns a [`Module`], even when it had to recover from
/// lexical errors, malformed syntax, unsupported syntax, or skipped
/// declarations. Inspect [`Self::diagnostics`] (or use [`Self::has_errors`]) to
/// decide whether the partial tree is acceptable. Use [`Self::into_result`] for
/// callers that require a diagnostic-free parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseModuleResult {
    /// Parsed module tree. This is always present and may be partial when
    /// diagnostics were recorded.
    pub module: Module,
    /// Recoverable parse and lex issues in source order.
    pub diagnostics: Vec<ParseDiagnostic>,
}

/// Recoverable parse failure for strict callers.
///
/// Produced when [`parse_module_strict`] or [`ParseModuleResult::into_result`]
/// reject a tolerant [`parse_module`] result because diagnostics were recorded.
/// The partial module tree is retained for inspection but is not considered a
/// successful parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseModuleError {
    diagnostics: Vec<ParseDiagnostic>,
    module: Box<Module>,
}

impl ParseModuleError {
    #[must_use]
    pub fn diagnostics(&self) -> &[ParseDiagnostic] {
        &self.diagnostics
    }

    #[must_use]
    pub fn module(&self) -> &Module {
        &self.module
    }

    #[must_use]
    pub fn into_parts(self) -> (Vec<ParseDiagnostic>, Module) {
        (self.diagnostics, *self.module)
    }
}

impl std::fmt::Display for ParseModuleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "parse failed with {} diagnostic(s)",
            self.diagnostics.len()
        )?;
        if let Some(first) = self.diagnostics.first() {
            write!(f, ": {}", first.message)?;
        }
        Ok(())
    }
}

impl std::error::Error for ParseModuleError {}

#[derive(Clone, Copy, Debug)]
enum DoExpressionMode {
    Allow,
    Disallow,
}

impl DoExpressionMode {
    const fn allows_do(&self) -> bool {
        matches!(self, Self::Allow)
    }
}

impl ParseModuleResult {
    /// True when tolerant parsing recorded any recoverable diagnostic.
    #[must_use]
    pub const fn has_errors(&self) -> bool {
        !self.diagnostics.is_empty()
    }

    /// Split the partial module tree from its source-ordered diagnostics.
    #[must_use]
    pub fn into_parts(self) -> (Module, Vec<ParseDiagnostic>) {
        (self.module, self.diagnostics)
    }

    /// Convert a tolerant parse result into a strict [`Result`].
    ///
    /// Returns [`Ok`] only when [`Self::diagnostics`] is empty. Any diagnostic —
    /// lexical, malformed, skipped declaration, unsupported syntax, or
    /// recursion-limit degradation — becomes [`Err`].
    ///
    /// # Errors
    ///
    /// Returns [`ParseModuleError`] when [`Self::diagnostics`] is non-empty.
    ///
    /// ```
    /// use daml_parser::parse::parse_module;
    ///
    /// let ok = parse_module("module M where\nfoo: Int\nfoo = 1\n").into_result();
    /// assert!(ok.is_ok());
    ///
    /// let err = parse_module("module M where\n@@@\n").into_result();
    /// assert!(err.is_err());
    /// assert!(!err.unwrap_err().diagnostics().is_empty());
    /// ```
    pub fn into_result(self) -> Result<Module, ParseModuleError> {
        if self.diagnostics.is_empty() {
            Ok(self.module)
        } else {
            Err(ParseModuleError {
                diagnostics: self.diagnostics,
                module: Box::new(self.module),
            })
        }
    }
}

/// Parse Daml `source` into a [`Module`] plus any [`ParseDiagnostic`]s, in
/// source order.
///
/// This is the crate's entry point. It **never panics and never aborts the
/// file**: recovery is per-declaration, so an unparseable declaration becomes a
/// [`Decl::Unknown`] (with a diagnostic) and parsing continues at the next
/// declaration. A `Module` is therefore always returned, even for badly broken
/// input — a non-empty diagnostics list signals problems, not a missing tree.
///
/// ```
/// let result = daml_parser::parse::parse_module("module M where\n");
/// assert_eq!(result.module.name, "M");
/// assert!(result.diagnostics.is_empty());
/// ```
#[must_use]
pub fn parse_module(source: &str) -> ParseModuleResult {
    let lexed = lex(source);
    let tokens = lexed.tokens;
    let lex_errors = lexed.errors;
    let tokens = resolve_layout(tokens);
    let mut p = Parser {
        toks: tokens,
        src_len: source.len(),
        i: 0,
        depth: 0,
        diags: lex_errors
            .into_iter()
            .map(|e| {
                let range = e.byte_range_in(source);
                ParseDiagnostic::new(
                    ParseDiagnosticKind::Lex(e.kind.clone()),
                    e.to_string(),
                    e.pos,
                    crate::ast::Span::from_usize(range.start, range.end),
                )
            })
            .collect(),
    };
    let mut module = p.module();
    module.span = crate::ast::Span::from_usize(0, source.len());
    ParseModuleResult {
        module,
        diagnostics: p.diags,
    }
}

/// Parse Daml `source` into a [`Module`], treating any diagnostic as failure.
///
/// This is a thin wrapper over [`parse_module`] followed by
/// [`ParseModuleResult::into_result`]. Use it for build/CI paths that must not
/// proceed on recoverable parse problems. For editors, formatters, and other
/// tools that need partial structure plus diagnostics, keep using
/// [`parse_module`].
///
/// ```
/// use daml_parser::parse::parse_module_strict;
///
/// # fn main() -> Result<(), Box<dyn std::error::Error>> {
/// let module = parse_module_strict("module M where\nfoo : Int\nfoo = 1\n")?;
/// assert_eq!(module.name, "M");
///
/// let strict = parse_module_strict("module M where\n%%% junk\n");
/// assert!(matches!(
///     strict.as_ref(),
///     Err(err) if !err.diagnostics().is_empty()
/// ));
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// Returns [`ParseModuleError`] when [`parse_module`] records any diagnostic.
pub fn parse_module_strict(source: &str) -> Result<Module, ParseModuleError> {
    parse_module(source).into_result()
}

struct Parser {
    toks: Vec<Token>,
    /// Source byte length — span fallback when a node consumes no real token.
    src_len: usize,
    i: usize,
    diags: Vec<ParseDiagnostic>,
    /// Expression/pattern recursion depth; bounded so hostile inputs
    /// (thousands of nested parens) cannot overflow the stack.
    depth: u32,
}

impl Parser {
    /// Byte span of every non-virtual token consumed since token index `from`
    /// (a function's entry cursor). This is the node's full extent: first real
    /// token's `start` to last real token's `end`. Virtual layout tokens carry
    /// no bytes and are skipped, so spans tile the source and never include the
    /// trailing whitespace a `VRBrace`/`VSemi` sits on.
    fn node_span(&self, from: usize) -> crate::ast::Span {
        let mut a = from;
        while a < self.i && self.toks[a].is_virtual() {
            a += 1;
        }
        let mut b = self.i;
        while b > a && self.toks[b - 1].is_virtual() {
            b -= 1;
        }
        if a >= b {
            // No real token consumed (e.g. an empty error node): zero-width
            // span at the next real byte position so it still nests inside its
            // parent. Use `a` (past any leading virtual tokens) — `from` itself
            // may be a virtual token whose byte offset is a meaningless 0.
            let p = self.byte_at(a);
            return crate::ast::Span::from_usize(p, p);
        }
        crate::ast::Span::from_usize(self.toks[a].start, self.toks[b - 1].end)
    }

    /// Byte offset where token `i` begins, or the source end past the last one.
    fn byte_at(&self, i: usize) -> usize {
        self.toks.get(i).map(|t| t.start).unwrap_or(self.src_len)
    }

    /// End byte of the last non-virtual token consumed so far — for nodes
    /// whose start comes from an already-parsed child rather than the entry
    /// cursor (e.g. `record with { .. }` built around its base expression).
    fn end_byte(&self) -> usize {
        let mut b = self.i;
        while b > 0 && self.toks[b - 1].is_virtual() {
            b -= 1;
        }
        if b == 0 {
            0
        } else {
            self.toks[b - 1].end
        }
    }
}

impl Parser {
    // ----- cursor primitives -------------------------------------------

    fn peek(&self) -> Option<&TokenKind> {
        self.toks.get(self.i).map(|t| &t.kind)
    }

    fn peek_at(&self, n: usize) -> Option<&TokenKind> {
        self.toks.get(self.i + n).map(|t| &t.kind)
    }

    fn pos(&self) -> Pos {
        self.toks
            .get(self.i)
            .or_else(|| self.toks.last())
            .map_or(Pos { line: 1, column: 1 }, |t| t.pos)
    }

    fn bump(&mut self) -> Option<Token> {
        let t = self.toks.get(self.i).cloned();
        if t.is_some() {
            self.i += 1;
        }
        t
    }

    fn at_keyword(&self, kw: &str) -> bool {
        self.peek().is_some_and(|t| t.is_keyword(kw))
    }

    fn eat_keyword(&mut self, kw: &str) -> bool {
        if self.at_keyword(kw) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn at_op(&self, op: &str) -> bool {
        self.peek().is_some_and(|t| t.is_op(op))
    }

    fn eat_op(&mut self, op: &str) -> bool {
        if self.at_op(op) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    fn at(&self, tok: &TokenKind) -> bool {
        self.peek() == Some(tok)
    }

    fn eat(&mut self, tok: &TokenKind) -> bool {
        if self.at(tok) {
            self.i += 1;
            true
        } else {
            false
        }
    }

    /// Emit an expected-token diagnostic at the current token.
    fn diag_expected(&mut self, expected: ExpectedToken, message: impl Into<String>) {
        self.diag_kind(ParseDiagnosticKind::ExpectedToken(expected), message);
    }

    /// Emit a malformed-syntax diagnostic at the current token.
    fn diag_malformed(&mut self, kind: MalformedSyntaxKind, message: impl Into<String>) {
        self.diag_kind(ParseDiagnosticKind::MalformedSyntax(kind), message);
    }

    /// Emit a diagnostic with an explicit typed recovery reason. The span is
    /// the current token's byte extent (the offending token), so consumers get
    /// an end position, not just a start.
    fn diag_kind(&mut self, kind: ParseDiagnosticKind, message: impl Into<String>) {
        let pos = self.pos();
        let span = self.cur_span();
        self.diags
            .push(ParseDiagnostic::new(kind, message, pos, span));
    }

    fn parse_type_annotation(
        &mut self,
        type_start: usize,
        type_end: usize,
        context: TypeAnnotationContext,
    ) -> TypeAnnotation {
        let type_start = type_start.min(self.toks.len());
        let type_end = type_end.min(self.toks.len());
        let tokens = &self.toks[type_start..type_end];
        let ty = parse_type_from_tokens(tokens).or_else(|| {
            let trimmed = Self::trim_type_tokens_for_parse(tokens);
            if trimmed < tokens.len() {
                parse_type_from_tokens(&tokens[..trimmed])
            } else {
                None
            }
        });
        match ty {
            Some(ty) => TypeAnnotation::Present(ty),
            None => {
                let span = self.span_of_token_range(type_start, type_end);
                self.diags.push(ParseDiagnostic::new(
                    ParseDiagnosticKind::MalformedTypeAnnotation(context),
                    format!("malformed {} type annotation", context.as_str()),
                    self.pos_of_token(type_start),
                    span,
                ));
                TypeAnnotation::Malformed { span }
            }
        }
    }

    /// Trim trailing tokens that belong to declaration tails (for example `= expr`
    /// in typed function signatures, explicit semicolons, or the next field
    /// declaration) so we can parse valid corpus forms that our tiny type
    /// parser doesn't model directly.
    fn trim_type_tokens_for_parse(tokens: &[Token]) -> usize {
        let mut depth = 0usize;
        let mut bracket_depth = 0usize;
        let mut i = 0usize;
        while i < tokens.len() {
            match &tokens[i].kind {
                TokenKind::LParen | TokenKind::LBracket => {
                    depth += 1;
                    i += 1;
                }
                TokenKind::RParen | TokenKind::RBracket => {
                    depth = depth.saturating_sub(1);
                    i += 1;
                }
                TokenKind::LBrace => {
                    bracket_depth += 1;
                    i += 1;
                }
                TokenKind::RBrace => {
                    bracket_depth = bracket_depth.saturating_sub(1);
                    i += 1;
                }
                TokenKind::Op(o) if o.as_str() == "=" && depth == 0 && bracket_depth == 0 => {
                    return i;
                }
                TokenKind::Semi | TokenKind::VSemi if depth == 0 && bracket_depth == 0 => {
                    return i;
                }
                TokenKind::Comma
                    if depth == 0
                        && bracket_depth == 0
                        && matches!(
                            tokens.get(i + 1),
                            Some(Token {
                                kind: TokenKind::LowerId {
                                    qualifier: None,
                                    ..
                                },
                                ..
                            })
                        )
                        && matches!(
                            tokens.get(i + 2),
                            Some(Token { kind: TokenKind::Op(o), .. }) if o.as_str() == ":"
                        ) =>
                {
                    return i;
                }
                _ => {
                    i += 1;
                }
            }
        }
        tokens.len()
    }

    fn pos_of_token(&self, idx: usize) -> Pos {
        self.toks.get(idx).map_or_else(|| self.pos(), |tok| tok.pos)
    }

    fn span_of_token_range(&self, start: usize, end: usize) -> Span {
        let start = start.min(self.toks.len());
        let end = end.min(self.toks.len());
        let span_start = self.byte_at(start);
        if end <= start {
            return Span::from_usize(span_start, span_start);
        }

        let mut cursor = end;
        while cursor > start {
            cursor -= 1;
            let token = &self.toks[cursor];
            if !token.is_virtual() {
                return Span::from_usize(span_start, token.end);
            }
        }

        Span::from_usize(span_start, span_start)
    }

    /// Byte span of the next real (non-virtual) token, or a zero-width span at
    /// end-of-input. Used to anchor a diagnostic to the offending token.
    fn cur_span(&self) -> crate::ast::Span {
        let mut j = self.i;
        while self.toks.get(j).is_some_and(|t| t.is_virtual()) {
            j += 1;
        }
        self.toks.get(j).map_or_else(
            || crate::ast::Span::from_usize(self.src_len, self.src_len),
            |t| crate::ast::Span::from_usize(t.start, t.end),
        )
    }

    /// Skip tokens until the end of the current block item: a `VSemi` or
    /// `VRBrace` at nesting depth zero (relative to here). Consumes neither.
    fn skip_to_item_end(&mut self) {
        let mut depth = 0usize;
        let mut brackets = 0usize;
        while let Some(t) = self.peek() {
            match t {
                TokenKind::VLBrace => depth += 1,
                TokenKind::VRBrace => {
                    if depth == 0 {
                        return;
                    }
                    depth -= 1;
                }
                TokenKind::VSemi if depth == 0 && brackets == 0 => return,
                TokenKind::LParen | TokenKind::LBracket | TokenKind::LBrace => brackets += 1,
                TokenKind::RParen | TokenKind::RBracket | TokenKind::RBrace => {
                    if brackets == 0 {
                        // Closing bracket of an enclosing construct: stop
                        // before it so the caller can match it.
                        return;
                    }
                    brackets -= 1;
                }
                _ => {}
            }
            self.i += 1;
        }
    }

    /// Raw text of tokens from `start` to the current position.
    fn slice_text(&self, start: usize) -> String {
        render_token_slice(&self.toks[start..self.i])
    }

    // ----- module ------------------------------------------------------

    fn module(&mut self) -> Module {
        let pos = self.pos();
        let header_start = self.i;
        let mut header = crate::ast::Span::from_usize(0, 0);
        let mut name = ModuleName::from("Unknown");

        if self.eat_keyword("module") {
            if let Some(TokenKind::UpperId { qualifier, name: n }) = self.peek().cloned() {
                self.bump();
                name = match qualifier {
                    Some(q) => format!("{q}.{n}").into(),
                    None => n.into(),
                };
            }
            // Optional export list.
            if self.at(&TokenKind::LParen) {
                self.skip_balanced_parens();
            }
            if !self.eat_keyword("where") {
                self.diag_expected(
                    ExpectedToken::WhereAfterModuleHeader,
                    "expected 'where' after module header",
                );
            }
            header = self.node_span(header_start);
        }

        let mut imports = Vec::new();
        let mut decls: Vec<Decl> = Vec::new();

        // Consume the opening brace of the module body if present. The result
        // is unused: the loop below terminates on the matching close brace or
        // end-of-input regardless of whether the block was braced.
        let _ = self.eat(&TokenKind::VLBrace) || self.eat(&TokenKind::LBrace);
        loop {
            while self.eat(&TokenKind::VSemi) || self.eat(&TokenKind::Semi) {}
            match self.peek() {
                None => break,
                Some(TokenKind::VRBrace | TokenKind::RBrace) => {
                    self.bump();
                    break;
                }
                // A stray closing bracket inside a block is garbage from a
                // failed item parse — record it as an Unknown declaration so
                // its bytes stay covered, then continue (skip_to_item_end
                // deliberately stops before unmatched closers).
                Some(TokenKind::RParen | TokenKind::RBracket) => {
                    let cpos = self.pos();
                    let cstart = self.i;
                    self.bump();
                    decls.push(Decl::Unknown {
                        raw: self.slice_text(cstart),
                        pos: cpos,
                        span: self.node_span(cstart),
                    });
                    continue;
                }
                _ => {}
            }
            let before = self.i;
            self.declaration(&mut imports, &mut decls);
            if self.i == before {
                // Defensive: guarantee progress even on a parser bug.
                self.bump();
            }
        }

        merge_functions(&mut decls);

        Module {
            name,
            pos,
            header,
            imports,
            decls,
            span: crate::ast::Span::from_usize(0, self.src_len),
        }
    }

    fn skip_balanced_parens(&mut self) {
        let mut depth = 0usize;
        while let Some(t) = self.peek() {
            match t {
                TokenKind::LParen => depth += 1,
                TokenKind::RParen => {
                    if depth == 0 {
                        return;
                    }
                    depth -= 1;
                    if depth == 0 {
                        self.i += 1;
                        return;
                    }
                }
                _ => {}
            }
            self.i += 1;
        }
    }

    /// If the cursor sits on an infix operator equation with a pattern
    /// left operand (`[] !! _ = ...`, `None <?> s = ...`), skip it and
    /// return true. Operators have no IR surface.
    fn try_infix_operator_decl(&mut self) -> bool {
        let snap = self.i;
        let saved_diags = self.diags.len();
        if self.pattern().is_some()
            && matches!(self.peek(), Some(TokenKind::Op(o)) if !is_reserved_op(o))
        {
            self.skip_to_item_end();
            return true;
        }
        self.i = snap;
        self.diags.truncate(saved_diags);
        false
    }

    fn declaration(&mut self, imports: &mut Vec<ImportDecl>, decls: &mut Vec<Decl>) {
        let pos = self.pos();
        let start = self.i;
        if matches!(
            self.peek(),
            Some(TokenKind::UpperId { .. } | TokenKind::LBracket | TokenKind::LParen)
        ) && self.try_infix_operator_decl()
        {
            decls.push(Decl::Unknown {
                raw: self.slice_text(start),
                pos,
                span: self.node_span(start),
            });
            return;
        }
        match self.peek() {
            Some(t) if t.is_keyword("import") => {
                let imp = self.import_decl();
                // The `(...)` import list / `hiding (...)` clause is consumed
                // here, after the decl is built; fold it into the span so the
                // import covers its whole source extent.
                self.skip_to_item_end();
                if let Some(mut imp) = imp {
                    imp.span = self.node_span(start);
                    imports.push(imp);
                }
            }
            Some(t) if t.is_keyword("template") => {
                // `template T = ...` (template-let synonym) is exotic; only
                // `template Name with/where` is a template declaration.
                match self.template_decl() {
                    Some(t) => decls.push(Decl::Template(t)),
                    None => {
                        self.skip_to_item_end();
                        decls.push(Decl::Unknown {
                            raw: self.slice_text(start),
                            span: self.node_span(start),
                            pos,
                        });
                    }
                }
            }
            Some(t) if t.is_keyword("interface") => match self.interface_decl() {
                Some(i) => decls.push(Decl::Interface(i)),
                None => {
                    self.skip_to_item_end();
                    decls.push(Decl::Unknown {
                        raw: self.slice_text(start),
                        span: self.node_span(start),
                        pos,
                    });
                }
            },
            Some(t) if matches!(t.keyword(), Some("infix" | "infixl" | "infixr")) => {
                if let Some(fixity) = self.fixity_decl() {
                    decls.push(Decl::Fixity(fixity));
                } else {
                    self.skip_to_item_end();
                    decls.push(Decl::Unknown {
                        raw: self.slice_text(start),
                        pos,
                        span: self.node_span(start),
                    });
                }
            }
            Some(t) if t.is_keyword("pattern") => {
                self.skip_to_item_end();
                decls.push(Decl::UnsupportedSyntax {
                    kind: UnsupportedSyntaxKind::PatternSynonym,
                    raw: self.slice_text(start),
                    pos,
                    span: self.node_span(start),
                });
            }
            Some(t) if t.is_keyword("default") => {
                self.skip_to_item_end();
                decls.push(Decl::Unknown {
                    raw: self.slice_text(start),
                    pos,
                    span: self.node_span(start),
                });
            }
            Some(t)
                if matches!(
                    t.keyword(),
                    Some(
                        "data"
                            | "type"
                            | "newtype"
                            | "class"
                            | "instance"
                            | "exception"
                            | "deriving"
                    )
                ) =>
            {
                let keyword = t
                    .keyword()
                    .expect("declaration-head keyword token")
                    .to_string();
                self.bump();
                let name = match self.peek() {
                    Some(TokenKind::UpperId { qualifier, name }) => {
                        let n = qualifier
                            .as_ref()
                            .map_or_else(|| name.to_string(), |q| format!("{q}.{name}"));
                        self.bump();
                        n
                    }
                    _ => String::new(),
                };
                self.skip_to_item_end();
                decls.push(Decl::TypeDef {
                    keyword,
                    name: name.into(),
                    pos,
                    span: self.node_span(start),
                });
            }
            Some(TokenKind::LowerId { .. }) => match self.function_item() {
                Some(d) => decls.push(d),
                None => {
                    self.skip_to_item_end();
                    decls.push(Decl::Unknown {
                        raw: self.slice_text(start),
                        span: self.node_span(start),
                        pos,
                    });
                }
            },
            // Operator definition or signature: `(<=) = curry Lte`, `(>=>) : ...`.
            Some(TokenKind::LParen)
                if matches!(self.peek_at(1), Some(TokenKind::Op(_)))
                    && self.peek_at(2) == Some(&TokenKind::RParen) =>
            {
                match self.operator_function_item() {
                    Some(d) => decls.push(d),
                    None => {
                        self.skip_to_item_end();
                        decls.push(Decl::Unknown {
                            raw: self.slice_text(start),
                            span: self.node_span(start),
                            pos,
                        });
                    }
                }
            }
            // Top-level pattern binding: `[a, b, c] = ...`, `(x, y) = ...`.
            Some(TokenKind::LParen | TokenKind::LBracket) => {
                if self.binding().is_none() {
                    self.diag_kind(
                        ParseDiagnosticKind::SkippedDeclaration(
                            SkippedDeclarationReason::TopLevelPatternBinding,
                        ),
                        "unparseable top-level pattern binding",
                    );
                }
                self.skip_to_item_end();
                decls.push(Decl::Unknown {
                    raw: self.slice_text(start),
                    span: self.node_span(start),
                    pos,
                });
            }
            _ => {
                self.diag_kind(
                    ParseDiagnosticKind::SkippedDeclaration(
                        SkippedDeclarationReason::UnrecognizedDeclaration,
                    ),
                    format!("unrecognized declaration: {:?}", self.peek()),
                );
                self.skip_to_item_end();
                decls.push(Decl::Unknown {
                    raw: self.slice_text(start),
                    span: self.node_span(start),
                    pos,
                });
            }
        }
    }

    // ----- imports -----------------------------------------------------

    fn import_decl(&mut self) -> Option<ImportDecl> {
        let pos = self.pos();
        let start_i = self.i;
        self.bump(); // import
        let mut style = if self.eat_keyword("qualified") {
            ImportStyle::Qualified
        } else {
            ImportStyle::Unqualified
        };
        // Package-qualified import: `import qualified "pkg-name" Main as V1`.
        if matches!(self.peek(), Some(TokenKind::StringLit(_))) {
            self.bump();
        }
        let module_name = match self.peek().cloned() {
            Some(TokenKind::UpperId { qualifier, name }) => {
                self.bump();
                match qualifier {
                    Some(q) => format!("{q}.{name}").into(),
                    None => name.into(),
                }
            }
            _ => {
                self.diag_expected(
                    ExpectedToken::ModuleNameAfterImport,
                    "expected module name after 'import'",
                );
                return None;
            }
        };
        // ImportQualifiedPost style: `import DA.Map qualified as Map`.
        if self.eat_keyword("qualified") {
            style = ImportStyle::Qualified;
        }
        let mut alias = None;
        if self.eat_keyword("as") {
            if let Some(TokenKind::UpperId { qualifier, name }) = self.peek().cloned() {
                self.bump();
                alias = Some(match qualifier {
                    Some(q) => format!("{q}.{name}").into(),
                    None => name.into(),
                });
            }
        }
        // `hiding (...)` / import list — consumed by skip_to_item_end.
        Some(ImportDecl {
            module_name,
            style,
            alias,
            pos,
            span: self.node_span(start_i),
        })
    }

    // ----- templates ---------------------------------------------------

    fn upper_name(&mut self) -> Option<ModuleName> {
        match self.peek().cloned() {
            Some(TokenKind::UpperId { qualifier, name }) => {
                self.bump();
                Some(match qualifier {
                    Some(q) => format!("{q}.{name}").into(),
                    None => name.into(),
                })
            }
            _ => None,
        }
    }

    fn template_decl(&mut self) -> Option<TemplateDecl> {
        let pos = self.pos();
        let start_i = self.i;
        self.bump(); // template
        if self.at_keyword("instance") {
            return None; // legacy `template instance` — not a template
        }
        let name = self.upper_name()?.to_string().into();

        let fields = self
            .eat_keyword("with")
            .then(|| self.field_block())
            .map(|parsed| parsed.fields)
            .unwrap_or_default();
        let body = if self.eat_keyword("where") {
            self.template_body()
        } else {
            Vec::new()
        };
        Some(TemplateDecl {
            name,
            fields,
            body,
            pos,
            span: self.node_span(start_i),
        })
    }

    /// `{ name : Type ; name2, name3 : Type ; ... }` (virtual or explicit).
    /// Returns the parsed fields and a "dangling" marker when the block was
    /// entered but abandoned early because its first item is not a field
    /// (an empty `with` whose layout block swallowed the next clause). The
    /// caller should discard the block's eventual closing `VRBrace` when this
    /// happens.
    fn field_block(&mut self) -> FieldBlock {
        let mut fields = Vec::new();
        if !(self.eat(&TokenKind::VLBrace) || self.eat(&TokenKind::LBrace)) {
            return FieldBlock {
                fields,
                dangling: false,
            };
        }
        loop {
            while self.eat(&TokenKind::VSemi) || self.eat(&TokenKind::Semi) {}
            match self.peek() {
                None => break,
                Some(TokenKind::VRBrace | TokenKind::RBrace) => {
                    self.bump();
                    break;
                }
                // A stray closing bracket inside a block is garbage from a
                // failed item parse — discard it or the loop cannot make
                // progress (skip_to_item_end deliberately stops before
                // unmatched closers).
                Some(TokenKind::RParen | TokenKind::RBracket) => {
                    self.bump();
                    continue;
                }
                _ => {}
            }
            // A field item must look like `name [, name] : Type`. If the
            // next item doesn't (an empty `with` swallowed the following
            // clause into its layout block — `with` + comment + `controller`),
            // stop without consuming so the caller can parse the clause.
            {
                let mut j = self.i;
                while let Some(TokenKind::LowerId {
                    qualifier: None, ..
                }) = self.toks.get(j).map(|t| &t.kind)
                {
                    j += 1;
                    match self.toks.get(j).map(|t| &t.kind) {
                        Some(TokenKind::Comma) => j += 1,
                        _ => break,
                    }
                }
                let is_field = j > self.i
                    && self
                        .toks
                        .get(j)
                        .map(|t| &t.kind)
                        .is_some_and(|t| t.is_op(":"));
                if !is_field {
                    return FieldBlock {
                        fields,
                        dangling: true,
                    };
                }
            }
            // One or more comma-separated names, then `:`, then the type.
            let mut names: Vec<(Identifier, Pos, Span)> = Vec::new();
            while let Some(TokenKind::LowerId {
                qualifier: None,
                name,
            }) = self.peek().cloned()
            {
                let p = self.pos();
                let nspan = Span::from_usize(self.toks[self.i].start, self.toks[self.i].end);
                self.bump();
                names.push((name, p, nspan));
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
            if names.is_empty() || !self.eat_op(":") {
                self.diag_expected(
                    ExpectedToken::FieldNameTypePair,
                    "expected 'name : Type' field",
                );
                self.skip_to_item_end();
                continue;
            }
            let ty_start = self.i;
            self.skip_to_item_end();
            let ty = self.parse_type_annotation(ty_start, self.i, TypeAnnotationContext::Field);
            // The type is shared by all names but sits after the last one, so
            // only the last field can span `name : Type` without overlapping a
            // sibling; earlier names of `x, y : T` stay name-only. daml-fmt
            // reads the type extent off the last field of a comma group.
            let type_end = self.end_byte();
            let last = names.len() - 1;
            for (idx, (name, p, nspan)) in names.into_iter().enumerate() {
                let span = if idx == last {
                    Span::from_usize(nspan.start_usize(), type_end.max(nspan.end_usize()))
                } else {
                    nspan
                };
                fields.push(FieldDecl {
                    name: name.to_string().into(),
                    ty: ty.clone(),
                    pos: p,
                    span,
                });
            }
        }
        FieldBlock {
            fields,
            dangling: false,
        }
    }

    // ----- template body ------------------------------------------------

    fn template_body(&mut self) -> Vec<TemplateBodyDecl> {
        let mut body = Vec::new();
        if !(self.eat(&TokenKind::VLBrace) || self.eat(&TokenKind::LBrace)) {
            return body;
        }
        loop {
            while self.eat(&TokenKind::VSemi) || self.eat(&TokenKind::Semi) {}
            match self.peek() {
                None => break,
                Some(TokenKind::VRBrace | TokenKind::RBrace) => {
                    self.bump();
                    break;
                }
                // A stray closing bracket inside a block is garbage from a
                // failed item parse — discard it or the loop cannot make
                // progress (skip_to_item_end deliberately stops before
                // unmatched closers).
                Some(TokenKind::RParen | TokenKind::RBracket) => {
                    self.bump();
                    continue;
                }
                _ => {}
            }
            let pos = self.pos();
            let start = self.i;
            let decl = self.template_body_item(pos, start);
            body.push(decl);
        }
        body
    }

    fn template_body_item(&mut self, pos: Pos, start: usize) -> TemplateBodyDecl {
        match self.peek().and_then(|t| t.keyword()) {
            Some("signatory") => {
                self.bump();
                let parties = self.expr_comma_list();
                self.skip_to_item_end();
                TemplateBodyDecl::Signatory {
                    parties,
                    pos,
                    span: self.node_span(start),
                }
            }
            Some("observer") => {
                self.bump();
                let parties = self.expr_comma_list();
                self.skip_to_item_end();
                TemplateBodyDecl::Observer {
                    parties,
                    pos,
                    span: self.node_span(start),
                }
            }
            Some("ensure") => {
                self.bump();
                let expr = self.expr();
                self.skip_to_item_end();
                TemplateBodyDecl::Ensure {
                    expr,
                    pos,
                    span: self.node_span(start),
                }
            }
            Some("key") => {
                self.bump();
                let expr_start = self.i;
                let expr = self.expr();
                let ty = if self.eat_op(":") {
                    let ty_start = self.i;
                    self.skip_to_item_end();
                    self.parse_type_annotation(ty_start, self.i, TypeAnnotationContext::Key)
                } else {
                    // The expression parser consumes `: Type` annotations;
                    // recover the key type from the last top-level colon.
                    let mut depth = 0i32;
                    let mut colon = None;
                    for j in expr_start..self.i {
                        match &self.toks[j].kind {
                            TokenKind::LParen | TokenKind::LBracket => depth += 1,
                            TokenKind::RParen | TokenKind::RBracket if depth > 0 => depth -= 1,
                            TokenKind::Op(o) if o.as_str() == ":" && depth == 0 => colon = Some(j),
                            _ => {}
                        }
                    }
                    let ty = colon.map_or(TypeAnnotation::Absent, |j| {
                        self.parse_type_annotation(j + 1, self.i, TypeAnnotationContext::Key)
                    });
                    self.skip_to_item_end();
                    ty
                };
                TemplateBodyDecl::Key {
                    expr,
                    ty,
                    pos,
                    span: self.node_span(start),
                }
            }
            Some("maintainer") => {
                self.bump();
                let expr = self.expr();
                self.skip_to_item_end();
                TemplateBodyDecl::Maintainer {
                    expr,
                    pos,
                    span: self.node_span(start),
                }
            }
            Some("choice" | "nonconsuming" | "preconsuming" | "postconsuming") => {
                self.choice_decl().map_or_else(
                    || {
                        self.skip_to_item_end();
                        TemplateBodyDecl::Other {
                            raw: self.slice_text(start),
                            span: self.node_span(start),
                            pos,
                        }
                    },
                    TemplateBodyDecl::Choice,
                )
            }
            Some("interface") => self.interface_instance_decl().map_or_else(
                || {
                    self.skip_to_item_end();
                    TemplateBodyDecl::Other {
                        raw: self.slice_text(start),
                        span: self.node_span(start),
                        pos,
                    }
                },
                TemplateBodyDecl::InterfaceInstance,
            ),
            Some("controller") => {
                // Legacy Daml 1.x `controller <party> can` choice blocks are
                // not analyzed — fail loud instead of silently dropping the
                // choices inside.
                self.diag_kind(
                    ParseDiagnosticKind::UnsupportedSyntax(
                        UnsupportedSyntaxKind::LegacyControllerCan,
                    ),
                    "legacy 'controller ... can' syntax is not supported; \
                     choices inside this block are not analyzed",
                );
                self.skip_to_item_end();
                TemplateBodyDecl::Other {
                    raw: self.slice_text(start),
                    span: self.node_span(start),
                    pos,
                }
            }
            _ => {
                self.skip_to_item_end();
                TemplateBodyDecl::Other {
                    raw: self.slice_text(start),
                    span: self.node_span(start),
                    pos,
                }
            }
        }
    }

    fn parse_choice_metadata_item(
        &mut self,
        observers: &mut Vec<Expr>,
        controllers: &mut Vec<Expr>,
        authority_exprs: &mut Vec<Expr>,
    ) -> bool {
        if self.eat_keyword("observer") {
            *observers = self.expr_comma_list_no_do();
            true
        } else if self.eat_keyword("controller") {
            *controllers = self.expr_comma_list_no_do();
            true
        } else if self.eat_keyword("authority") {
            *authority_exprs = self.expr_comma_list_no_do();
            true
        } else {
            false
        }
    }

    fn choice_metadata_block(
        &mut self,
        observers: &mut Vec<Expr>,
        controllers: &mut Vec<Expr>,
        authority_exprs: &mut Vec<Expr>,
    ) {
        loop {
            while self.eat(&TokenKind::VSemi) || self.eat(&TokenKind::Semi) {}
            match self.peek() {
                None => break,
                Some(TokenKind::VRBrace | TokenKind::RBrace) => {
                    self.bump();
                    break;
                }
                Some(TokenKind::RParen | TokenKind::RBracket) => {
                    self.bump();
                    continue;
                }
                _ => {}
            }
            if !self.parse_choice_metadata_item(observers, controllers, authority_exprs) {
                self.skip_to_item_end();
            }
        }
    }

    fn choice_decl(&mut self) -> Option<ChoiceDecl> {
        let pos = self.pos();
        let start_i = self.i;
        let consuming = match self.peek().and_then(|t| t.keyword()) {
            Some("nonconsuming") => {
                self.bump();
                Consuming::NonConsuming
            }
            Some("preconsuming") => {
                self.bump();
                Consuming::PreConsuming
            }
            Some("postconsuming") => {
                self.bump();
                Consuming::PostConsuming
            }
            _ => Consuming::Consuming,
        };
        if !self.eat_keyword("choice") {
            return None;
        }
        let name = self.upper_name()?.to_string().into();
        let return_ty = if self.eat_op(":") {
            let ty_start = self.i;
            self.skip_type_tokens();
            self.parse_type_annotation(ty_start, self.i, TypeAnnotationContext::Choice)
        } else {
            TypeAnnotation::Absent
        };
        let (params, dangling) = if self.eat_keyword("with") {
            let parsed = self.field_block();
            (parsed.fields, parsed.dangling)
        } else {
            (Vec::new(), false)
        };
        let mut observers = Vec::new();
        let mut controllers = Vec::new();
        let mut authority_exprs = Vec::new();
        if self.eat_keyword("where") {
            if self.eat(&TokenKind::VLBrace) || self.eat(&TokenKind::LBrace) {
                self.choice_metadata_block(&mut observers, &mut controllers, &mut authority_exprs);
            }
        } else {
            loop {
                // Inside a dangling (empty) with-block the controller/observer/
                // do clauses sit at the block's column, so layout separates
                // them with virtual semicolons — consume those.
                if dangling {
                    while self.eat(&TokenKind::VSemi) {}
                }
                if !self.parse_choice_metadata_item(
                    &mut observers,
                    &mut controllers,
                    &mut authority_exprs,
                ) {
                    break;
                }
            }
        }
        if dangling {
            while self.eat(&TokenKind::VSemi) {}
        }
        let body = if self.peek().is_some_and(|t| {
            !matches!(
                t,
                TokenKind::VSemi | TokenKind::VRBrace | TokenKind::Semi | TokenKind::RBrace
            )
        }) {
            Some(self.expr())
        } else {
            None
        };
        self.skip_to_item_end();
        if dangling {
            // Discard the abandoned with-block's closing brace so it does
            // not terminate the enclosing template/interface body.
            self.eat(&TokenKind::VRBrace);
            self.skip_to_item_end();
        }
        Some(ChoiceDecl {
            name,
            consuming,
            return_ty,
            params,
            controllers,
            observers,
            authority_exprs,
            body,
            pos,
            span: self.node_span(start_i),
        })
    }

    /// Consume type tokens up to (not including) a layout boundary or a
    /// `with`/`controller`/`observer`/`do`/`where` keyword at bracket depth 0.
    fn skip_type_tokens(&mut self) {
        let mut brackets = 0usize;
        while let Some(t) = self.peek() {
            match t {
                TokenKind::VSemi | TokenKind::VRBrace | TokenKind::VLBrace | TokenKind::Semi => {
                    return
                }
                TokenKind::LParen | TokenKind::LBracket => brackets += 1,
                TokenKind::RParen | TokenKind::RBracket => {
                    if brackets == 0 {
                        return;
                    }
                    brackets -= 1;
                }
                _ if brackets == 0
                    && matches!(
                        t.keyword(),
                        Some("with" | "controller" | "observer" | "authority" | "do" | "where")
                    ) =>
                {
                    return
                }
                _ => {}
            }
            self.i += 1;
        }
    }

    // ----- interfaces ----------------------------------------------------

    fn interface_decl(&mut self) -> Option<InterfaceDecl> {
        let pos = self.pos();
        let start_i = self.i;
        self.bump(); // interface
        if self.at_keyword("instance") {
            // Top-level retroactive interface instance: skip gracefully.
            return None;
        }
        let name = self.upper_name()?.to_string().into();
        let mut requires = Vec::new();
        if self.eat_keyword("requires") {
            while let Some(r) = self.upper_name() {
                requires.push(r);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }
        if !self.eat_keyword("where") {
            return None;
        }
        let mut viewtype = None;
        let mut methods = Vec::new();
        let mut choices = Vec::new();
        if !(self.eat(&TokenKind::VLBrace) || self.eat(&TokenKind::LBrace)) {
            return Some(InterfaceDecl {
                name,
                requires,
                viewtype,
                methods,
                choices,
                pos,
                span: self.node_span(start_i),
            });
        }
        loop {
            while self.eat(&TokenKind::VSemi) || self.eat(&TokenKind::Semi) {}
            match self.peek() {
                None => break,
                Some(TokenKind::VRBrace | TokenKind::RBrace) => {
                    self.bump();
                    break;
                }
                // A stray closing bracket inside a block is garbage from a
                // failed item parse — discard it or the loop cannot make
                // progress (skip_to_item_end deliberately stops before
                // unmatched closers).
                Some(TokenKind::RParen | TokenKind::RBracket) => {
                    self.bump();
                    continue;
                }
                _ => {}
            }
            match self.peek().and_then(|t| t.keyword()) {
                Some("viewtype") => {
                    self.bump();
                    viewtype = self.upper_name();
                    self.skip_to_item_end();
                }
                Some("choice" | "nonconsuming" | "preconsuming" | "postconsuming") => {
                    if let Some(c) = self.choice_decl() {
                        choices.push(c);
                    } else {
                        self.skip_to_item_end();
                    }
                }
                _ => {
                    // Method signature `name : Type`; anything else (default
                    // implementations, ensure, ...) is skipped.
                    let mpos = self.pos();
                    if let Some(TokenKind::LowerId {
                        qualifier: None,
                        name: mname,
                    }) = self.peek().cloned()
                    {
                        if self.peek_at(1).is_some_and(|t| t.is_op(":")) {
                            let mstart = self.toks[self.i].start;
                            self.bump();
                            self.bump();
                            let ty_start = self.i;
                            self.skip_to_item_end();
                            // Single name: span the whole `name : Type`.
                            methods.push(FieldDecl {
                                name: mname,
                                ty: self.parse_type_annotation(
                                    ty_start,
                                    self.i,
                                    TypeAnnotationContext::InterfaceMethod,
                                ),
                                pos: mpos,
                                span: Span::from_usize(mstart, self.end_byte().max(mstart)),
                            });
                            continue;
                        }
                    }
                    self.skip_to_item_end();
                }
            }
        }
        Some(InterfaceDecl {
            name,
            requires,
            viewtype,
            methods,
            choices,
            pos,
            span: self.node_span(start_i),
        })
    }

    /// `interface instance I for T where { method-bindings }`
    fn interface_instance_decl(&mut self) -> Option<InterfaceInstanceDecl> {
        let pos = self.pos();
        let start_i = self.i;
        self.bump(); // interface
        if !self.eat_keyword("instance") {
            return None;
        }
        let interface_name = self.upper_name()?;
        let for_template = if self.eat_keyword("for") {
            self.upper_name().map_or_else(
                || {
                    self.diag_expected(
                        ExpectedToken::TemplateNameAfterInterfaceInstanceFor,
                        "interface instance missing template name after 'for'",
                    );
                    None
                },
                Some,
            )
        } else {
            None
        };
        let mut methods = Vec::new();
        if self.eat_keyword("where")
            && (self.eat(&TokenKind::VLBrace) || self.eat(&TokenKind::LBrace))
        {
            loop {
                while self.eat(&TokenKind::VSemi) || self.eat(&TokenKind::Semi) {}
                match self.peek() {
                    None => break,
                    Some(TokenKind::VRBrace | TokenKind::RBrace) => {
                        self.bump();
                        break;
                    }
                    // Stray closer: discard so the loop always progresses.
                    Some(TokenKind::RParen | TokenKind::RBracket) => {
                        self.bump();
                        continue;
                    }
                    _ => {}
                }
                if let Some(b) = self.binding() {
                    methods.push(b);
                } else {
                    self.skip_to_item_end();
                }
            }
        }
        Some(InterfaceInstanceDecl {
            interface_name,
            for_template,
            methods,
            pos,
            span: self.node_span(start_i),
        })
    }

    // ----- functions -----------------------------------------------------

    fn fixity_decl(&mut self) -> Option<FixityDecl> {
        let pos = self.pos();
        let start_i = self.i;
        let assoc = match self.peek() {
            Some(t) if t.is_keyword("infixr") => FixityAssoc::InfixR,
            Some(t) if t.is_keyword("infixl") => FixityAssoc::InfixL,
            Some(t) if t.is_keyword("infix") => FixityAssoc::Infix,
            _ => return None,
        };
        self.bump();
        let precedence = match self.peek() {
            Some(TokenKind::IntLit(n)) => {
                let parsed: u8 = n.parse().ok()?;
                self.bump();
                parsed
            }
            _ => {
                self.diag_malformed(
                    MalformedSyntaxKind::FunctionEquation,
                    "expected precedence number in fixity declaration",
                );
                return None;
            }
        };
        let mut operators = Vec::new();
        loop {
            match self.peek().cloned() {
                Some(TokenKind::Backtick) => {
                    self.bump();
                    let name = match self.peek().cloned() {
                        Some(
                            TokenKind::LowerId {
                                qualifier: None,
                                name,
                            }
                            | TokenKind::UpperId {
                                qualifier: None,
                                name,
                            },
                        ) => {
                            self.bump();
                            name
                        }
                        _ => {
                            self.diag_malformed(
                                MalformedSyntaxKind::FunctionEquation,
                                "expected identifier in backtick fixity target",
                            );
                            return None;
                        }
                    };
                    if !self.eat(&TokenKind::Backtick) {
                        self.diag_malformed(
                            MalformedSyntaxKind::FunctionEquation,
                            "expected closing backtick in fixity declaration",
                        );
                        return None;
                    }
                    operators.push(FixityTarget::Backtick(name));
                }
                Some(TokenKind::Op(op)) if !is_reserved_op(&op) => {
                    self.bump();
                    operators.push(FixityTarget::Operator(op));
                }
                _ => break,
            }
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        if operators.is_empty() {
            self.diag_malformed(
                MalformedSyntaxKind::FunctionEquation,
                "fixity declaration must name at least one operator",
            );
            return None;
        }
        self.skip_to_item_end();
        Some(FixityDecl {
            assoc,
            precedence,
            operators,
            pos,
            span: self.node_span(start_i),
        })
    }

    /// Parenthesized operator signature or equation: `(===) : ...`, `(>=>) = ...`.
    fn operator_function_item(&mut self) -> Option<Decl> {
        let pos = self.pos();
        let start_i = self.i;
        if !self.eat(&TokenKind::LParen) {
            return None;
        }
        let Some(TokenKind::Op(op)) = self.peek().cloned() else {
            return None;
        };
        if is_reserved_op(&op) {
            return None;
        }
        self.bump();
        if !self.eat(&TokenKind::RParen) {
            return None;
        }
        let name = Identifier::from(op.as_str());

        if self.at_op(":") {
            self.eat_op(":");
            let ty_start = self.i;
            self.skip_to_item_end();
            let ty = self.parse_type_annotation(ty_start, self.i, TypeAnnotationContext::Function);
            return Some(Decl::Function(FunctionDecl {
                name,
                ty,
                equations: Vec::new(),
                pos,
                sig_span: Some(self.node_span(start_i)),
                span: self.node_span(start_i),
            }));
        }

        if self.at_op("=") || self.at_op("|") {
            let (body, guards) = self.equation_rhs()?;
            let where_bindings = if self.eat_keyword("where") {
                self.binding_block()
            } else {
                Vec::new()
            };
            self.skip_to_item_end();
            return Some(Decl::Function(FunctionDecl {
                name,
                ty: TypeAnnotation::Absent,
                equations: vec![Equation {
                    params: Vec::new(),
                    body,
                    guards,
                    where_bindings,
                    pos,
                    span: self.node_span(start_i),
                }],
                pos,
                sig_span: None,
                span: self.node_span(start_i),
            }));
        }

        None
    }

    fn try_infix_operator_function(&mut self) -> Option<Decl> {
        let pos = self.pos();
        let start_i = self.i;
        let Some(TokenKind::LowerId {
            qualifier: None,
            name: lhs,
        }) = self.peek().cloned()
        else {
            return None;
        };
        let lhs_start = self.i;
        self.bump();
        let Some(TokenKind::Op(op)) = self.peek().cloned() else {
            return None;
        };
        if is_reserved_op(&op) {
            return None;
        }
        self.bump();
        let rhs_start = self.i;
        let Some(TokenKind::LowerId {
            qualifier: None,
            name: rhs,
        }) = self.peek().cloned()
        else {
            return None;
        };
        self.bump();
        if !(self.at_op("=") || self.at_op("|")) {
            return None;
        }
        let name = Identifier::from(op.as_str());
        let lhs_pat = Pat::Var {
            name: lhs,
            pos: self.pos_of_token(lhs_start),
            span: self.node_span(lhs_start),
        };
        let rhs_pat = Pat::Var {
            name: rhs,
            pos: self.pos_of_token(rhs_start),
            span: self.node_span(rhs_start),
        };
        let (body, guards) = self.equation_rhs()?;
        let where_bindings = if self.eat_keyword("where") {
            self.binding_block()
        } else {
            Vec::new()
        };
        self.skip_to_item_end();
        Some(Decl::Function(FunctionDecl {
            name,
            ty: TypeAnnotation::Absent,
            equations: vec![Equation {
                params: vec![lhs_pat, rhs_pat],
                body,
                guards,
                where_bindings,
                pos,
                span: self.node_span(start_i),
            }],
            pos,
            sig_span: None,
            span: self.node_span(start_i),
        }))
    }

    /// A top-level item starting with a lowercase identifier: type
    /// signature or function equation. Operator definitions and other
    /// exotica return None.
    fn function_item(&mut self) -> Option<Decl> {
        let snap = self.i;
        let saved_diags = self.diags.len();
        if let Some(decl) = self.try_infix_operator_function() {
            return Some(decl);
        }
        self.i = snap;
        self.diags.truncate(saved_diags);

        let pos = self.pos();
        let start_i = self.i;
        let Some(TokenKind::LowerId {
            qualifier: None,
            name,
        }) = self.peek().cloned()
        else {
            return None;
        };

        // Type signature: `name [, name2] : Type`
        let mut j = self.i + 1;
        let mut is_sig = false;
        loop {
            match self.toks.get(j).map(|t| &t.kind) {
                Some(TokenKind::Comma) => {
                    j += 1;
                    if matches!(
                        self.toks.get(j).map(|t| &t.kind),
                        Some(TokenKind::LowerId {
                            qualifier: None,
                            ..
                        })
                    ) {
                        j += 1;
                        continue;
                    }
                    break;
                }
                Some(TokenKind::Op(o)) if o.as_str() == ":" => {
                    is_sig = true;
                    break;
                }
                _ => break,
            }
        }
        if is_sig {
            self.bump(); // name
            while self.eat(&TokenKind::Comma) {
                self.bump(); // more names
            }
            self.eat_op(":");
            let ty_start = self.i;
            self.skip_to_item_end();
            let ty = self.parse_type_annotation(ty_start, self.i, TypeAnnotationContext::Function);
            return Some(Decl::Function(FunctionDecl {
                name,
                ty,
                equations: Vec::new(),
                pos,
                sig_span: Some(self.node_span(start_i)),
                span: self.node_span(start_i),
            }));
        }

        // Function equation: name pats (= expr | guards), optional where.
        self.bump(); // name
        let mut params = Vec::new();
        while !self.at_op("=") && !self.at_op("|") {
            // Combined signature + body: `name (x : a) : RetType = expr` —
            // consume the return-type annotation up to the `=`.
            if self.at_op(":") {
                self.bump();
                let mut brackets = 0usize;
                while let Some(t) = self.peek() {
                    match t {
                        TokenKind::Op(o) if o.as_str() == "=" && brackets == 0 => break,
                        TokenKind::VSemi
                        | TokenKind::VRBrace
                        | TokenKind::Semi
                        | TokenKind::RBrace => break,
                        TokenKind::LParen | TokenKind::LBracket => brackets += 1,
                        TokenKind::RParen | TokenKind::RBracket => {
                            brackets = brackets.saturating_sub(1)
                        }
                        _ => {}
                    }
                    self.i += 1;
                }
                continue;
            }
            // Infix operator definition: `f $ x = f x`, `as <&> f = ...` —
            // operators have no IR surface; skip the item silently.
            if matches!(self.peek(), Some(TokenKind::Op(o)) if !is_reserved_op(o)) {
                self.skip_to_item_end();
                return None;
            }
            match self.peek() {
                None
                | Some(
                    TokenKind::VSemi | TokenKind::VRBrace | TokenKind::Semi | TokenKind::RBrace,
                ) => {
                    self.diag_malformed(
                        MalformedSyntaxKind::FunctionEquation,
                        format!("could not parse equation for '{name}'"),
                    );
                    return None;
                }
                _ => {}
            }
            match self.pattern_atom() {
                Some(p) => params.push(p),
                None => {
                    self.diag_malformed(
                        MalformedSyntaxKind::FunctionParameterPattern,
                        format!("bad parameter pattern in '{name}'"),
                    );
                    return None;
                }
            }
        }
        let (body, guards) = self.equation_rhs()?;
        let where_bindings = if self.eat_keyword("where") {
            self.binding_block()
        } else {
            Vec::new()
        };
        self.skip_to_item_end();
        Some(Decl::Function(FunctionDecl {
            name,
            ty: TypeAnnotation::Absent,
            equations: vec![Equation {
                params,
                body,
                guards,
                where_bindings,
                pos,
                span: self.node_span(start_i),
            }],
            pos,
            sig_span: None,
            span: self.node_span(start_i),
        }))
    }

    /// `= expr` or `| guard = expr | guard = expr ...`
    fn equation_rhs(&mut self) -> Option<(Expr, Vec<(Expr, Expr)>)> {
        if self.eat_op("=") {
            return Some((self.expr(), Vec::new()));
        }
        let mut guards = Vec::new();
        while self.eat_op("|") {
            // Comma-separated guard qualifiers, each a boolean expression
            // or a pattern guard `pat <- expr`.
            let g = loop {
                let g = self.expr();
                if self.eat_op("<-") {
                    let _ = self.expr(); // pattern guard: keep the pattern side
                }
                if !self.eat(&TokenKind::Comma) {
                    break g;
                }
            };
            if !self.eat_op("=") {
                self.diag_expected(ExpectedToken::EqualsAfterGuard, "expected '=' after guard");
                return None;
            }
            let e = self.expr();
            guards.push((g, e));
        }
        if guards.is_empty() {
            self.diag_expected(
                ExpectedToken::EqualsOrGuardedRightHandSide,
                "expected '=' or guarded right-hand side in equation",
            );
            None
        } else {
            let first = guards[0].1.clone();
            Some((first, guards))
        }
    }

    /// `{ binding ; binding ; ... }` for let/where blocks.
    fn binding_block(&mut self) -> Vec<Binding> {
        let mut bindings = Vec::new();
        if !(self.eat(&TokenKind::VLBrace) || self.eat(&TokenKind::LBrace)) {
            return bindings;
        }
        loop {
            while self.eat(&TokenKind::VSemi) || self.eat(&TokenKind::Semi) {}
            match self.peek() {
                None => break,
                Some(TokenKind::VRBrace | TokenKind::RBrace) => {
                    self.bump();
                    break;
                }
                // A stray closing bracket inside a block is garbage from a
                // failed item parse — discard it or the loop cannot make
                // progress (skip_to_item_end deliberately stops before
                // unmatched closers).
                Some(TokenKind::RParen | TokenKind::RBracket) => {
                    self.bump();
                    continue;
                }
                _ => {}
            }
            match self.binding() {
                Some(b) => bindings.push(b),
                None => self.skip_to_item_end(),
            }
        }
        bindings
    }

    /// One binding: `pat = expr`, `f x y = expr`, guarded variants, or a
    /// type signature (skipped, returns None).
    fn binding(&mut self) -> Option<Binding> {
        let pos = self.pos();
        let start_i = self.i;
        // Operator binding or signature: `(==) : Text -> Bool = ...` —
        // skip the whole item; operators aren't surfaced in the IR.
        if self.at(&TokenKind::LParen)
            && matches!(self.peek_at(1), Some(TokenKind::Op(_)))
            && self.peek_at(2) == Some(&TokenKind::RParen)
        {
            self.skip_to_item_end();
            return None;
        }
        let pat = self.pattern_atom()?;
        let mut params = Vec::new();
        loop {
            if self.at_op("=") {
                self.bump();
                let expr = self.expr();
                // Bindings can carry their own where blocks.
                if self.eat_keyword("where") {
                    let _ = self.binding_block();
                }
                return Some(Binding {
                    pat,
                    params,
                    expr,
                    pos,
                    span: self.node_span(start_i),
                });
            }
            if self.at_op("|") {
                let (body, _) = self.equation_rhs()?;
                if self.eat_keyword("where") {
                    let _ = self.binding_block();
                }
                return Some(Binding {
                    pat,
                    params,
                    expr: body,
                    pos,
                    span: self.node_span(start_i),
                });
            }
            if self.at_op(":") {
                if params.is_empty() {
                    // Type signature inside a let/where block — skip it.
                    self.skip_to_item_end();
                    return None;
                }
                // Combined signature + body: `f (x : a) : Ret = expr` —
                // consume the return-type annotation up to the `=`.
                self.bump();
                let mut brackets = 0usize;
                while let Some(t) = self.peek() {
                    match t {
                        TokenKind::Op(o) if o.as_str() == "=" && brackets == 0 => break,
                        TokenKind::VSemi
                        | TokenKind::VRBrace
                        | TokenKind::Semi
                        | TokenKind::RBrace => break,
                        TokenKind::LParen | TokenKind::LBracket => brackets += 1,
                        TokenKind::RParen | TokenKind::RBracket => {
                            brackets = brackets.saturating_sub(1)
                        }
                        _ => {}
                    }
                    self.i += 1;
                }
                continue;
            }
            // Infix operator binding with a pattern operand:
            // `None <?> s = ...` in a where/let block.
            if matches!(self.peek(), Some(TokenKind::Op(o)) if !is_reserved_op(o)) {
                self.skip_to_item_end();
                return None;
            }
            match self.peek() {
                None
                | Some(
                    TokenKind::VSemi | TokenKind::VRBrace | TokenKind::Semi | TokenKind::RBrace,
                ) => return None,
                _ => {}
            }
            params.push(self.pattern_atom()?);
        }
    }

    // ----- patterns ------------------------------------------------------

    fn pattern_atom(&mut self) -> Option<Pat> {
        if self.depth >= MAX_RECURSION_DEPTH {
            return None;
        }
        self.depth += 1;
        let result = self.pattern_atom_inner();
        self.depth -= 1;
        result
    }

    fn pattern_atom_inner(&mut self) -> Option<Pat> {
        let pos = self.pos();
        let start_i = self.i;
        // Lazy / strict pattern markers: `~(as, bs)`, `!x`.
        if self.at_op("~") || self.at_op("!") {
            self.bump();
            return self.pattern_atom();
        }
        match self.peek().cloned() {
            Some(TokenKind::LowerId {
                qualifier: None,
                name,
            }) => {
                self.bump();
                if name == "_" {
                    return Some(Pat::Wild {
                        pos,
                        span: self.node_span(start_i),
                    });
                }
                if self.at_op("@") {
                    self.bump();
                    let inner = self.pattern_atom()?;
                    return Some(Pat::As {
                        name,
                        pat: Box::new(inner),
                        pos,
                        span: self.node_span(start_i),
                    });
                }
                Some(Pat::Var {
                    name,
                    pos,
                    span: self.node_span(start_i),
                })
            }
            Some(TokenKind::Op(o)) if o.as_str() == "_" => {
                self.bump();
                Some(Pat::Wild {
                    pos,
                    span: self.node_span(start_i),
                })
            }
            Some(TokenKind::UpperId { qualifier, name }) => {
                self.bump();
                // Record pattern `Foo {..}` / `Foo {x = y}` /
                // `Foo with claim; tag`.
                if self.at(&TokenKind::LBrace) {
                    self.skip_balanced_braces();
                } else if self.eat_keyword("with") {
                    let _ = self.record_fields();
                }
                Some(Pat::Con {
                    qualifier,
                    name,
                    args: Vec::new(),
                    pos,
                    span: self.node_span(start_i),
                })
            }
            Some(TokenKind::IntLit(text)) => {
                self.bump();
                Some(Pat::Lit {
                    kind: LitKind::Int,
                    text,
                    pos,
                    span: self.node_span(start_i),
                })
            }
            Some(TokenKind::DecimalLit(text)) => {
                self.bump();
                Some(Pat::Lit {
                    kind: LitKind::Decimal,
                    text,
                    pos,
                    span: self.node_span(start_i),
                })
            }
            Some(TokenKind::StringLit(text)) => {
                self.bump();
                Some(Pat::Lit {
                    kind: LitKind::Text,
                    text,
                    pos,
                    span: self.node_span(start_i),
                })
            }
            Some(TokenKind::CharLit(text)) => {
                self.bump();
                Some(Pat::Lit {
                    kind: LitKind::Char,
                    text,
                    pos,
                    span: self.node_span(start_i),
                })
            }
            Some(TokenKind::LParen) => {
                self.bump();
                if self.eat(&TokenKind::RParen) {
                    return Some(Pat::Con {
                        qualifier: None,
                        name: "()".into(),
                        args: Vec::new(),
                        pos,
                        span: self.node_span(start_i),
                    });
                }
                // View pattern `(expr -> pat)`: scan for a top-level `->`
                // inside these parens; the expression side is discarded and
                // the pattern after the arrow is the binding. A top-level
                // `:` before the arrow means the arrow belongs to a type
                // annotation (`(f : Int -> Bool)`), not a view pattern.
                {
                    let mut depth = 0usize;
                    let mut j = self.i;
                    let mut arrow = None;
                    while let Some(t) = self.toks.get(j).map(|t| &t.kind) {
                        match t {
                            TokenKind::LParen | TokenKind::LBracket => depth += 1,
                            TokenKind::RParen | TokenKind::RBracket => {
                                if depth == 0 {
                                    break;
                                }
                                depth -= 1;
                            }
                            TokenKind::Op(o) if o.as_str() == ":" && depth == 0 => break,
                            TokenKind::Op(o) if o.as_str() == "->" && depth == 0 => {
                                arrow = Some(j);
                                break;
                            }
                            TokenKind::VSemi | TokenKind::VRBrace => break,
                            // A lambda's arrow belongs to the lambda.
                            TokenKind::Op(o) if o.as_str() == "\\" => break,
                            _ => {}
                        }
                        j += 1;
                    }
                    if let Some(j) = arrow {
                        self.i = j + 1; // skip the view expression and `->`
                        let inner = self.pattern()?;
                        self.eat(&TokenKind::RParen);
                        return Some(inner);
                    }
                }
                let first = self.pattern()?;
                // Type-annotated pattern `(e : AnyException)`: skip the type.
                if self.at_op(":") {
                    let mut depth = 0usize;
                    while let Some(t) = self.peek() {
                        match t {
                            TokenKind::LParen | TokenKind::LBracket => depth += 1,
                            TokenKind::RParen if depth == 0 => break,
                            TokenKind::RParen | TokenKind::RBracket => {
                                depth = depth.saturating_sub(1)
                            }
                            TokenKind::VSemi | TokenKind::VRBrace => break,
                            _ => {}
                        }
                        self.i += 1;
                    }
                }
                if self.at(&TokenKind::Comma) {
                    let mut items = vec![first];
                    while self.eat(&TokenKind::Comma) {
                        items.push(self.pattern()?);
                    }
                    self.eat(&TokenKind::RParen);
                    return Some(Pat::Tuple {
                        items,
                        pos,
                        span: self.node_span(start_i),
                    });
                }
                self.eat(&TokenKind::RParen);
                Some(first)
            }
            Some(TokenKind::LBracket) => {
                self.bump();
                let mut items = Vec::new();
                if !self.eat(&TokenKind::RBracket) {
                    loop {
                        items.push(self.pattern()?);
                        if !self.eat(&TokenKind::Comma) {
                            break;
                        }
                    }
                    self.eat(&TokenKind::RBracket);
                }
                Some(Pat::List {
                    items,
                    pos,
                    span: self.node_span(start_i),
                })
            }
            _ => None,
        }
    }

    /// Full pattern: constructor applications and infix cons `x :: xs`.
    fn pattern(&mut self) -> Option<Pat> {
        if self.depth >= MAX_RECURSION_DEPTH {
            return None;
        }
        self.depth += 1;
        let result = self.pattern_inner();
        self.depth -= 1;
        result
    }

    fn pattern_inner(&mut self) -> Option<Pat> {
        let pos = self.pos();
        let start_i = self.i;
        let first = match self.peek().cloned() {
            Some(TokenKind::UpperId { qualifier, name }) => {
                self.bump();
                if self.at(&TokenKind::LBrace) || self.at_keyword("with") {
                    if self.eat_keyword("with") {
                        let _ = self.record_fields();
                    } else {
                        self.skip_balanced_braces();
                    }
                    Pat::Con {
                        qualifier,
                        name,
                        args: Vec::new(),
                        pos,
                        span: self.node_span(start_i),
                    }
                } else {
                    let mut args = Vec::new();
                    while let Some(a) = self.try_pattern_atom() {
                        args.push(a);
                    }
                    Pat::Con {
                        qualifier,
                        name,
                        args,
                        pos,
                        span: self.node_span(start_i),
                    }
                }
            }
            _ => self.pattern_atom()?,
        };
        if self.at_op("::") {
            self.bump();
            let rest = self.pattern()?;
            return Some(Pat::Con {
                qualifier: None,
                name: "::".into(),
                args: vec![first, rest],
                pos,
                span: self.node_span(start_i),
            });
        }
        Some(first)
    }

    fn try_pattern_atom(&mut self) -> Option<Pat> {
        match self.peek() {
            Some(
                TokenKind::LowerId {
                    qualifier: None, ..
                }
                | TokenKind::UpperId { .. }
                | TokenKind::IntLit(_)
                | TokenKind::DecimalLit(_)
                | TokenKind::StringLit(_)
                | TokenKind::CharLit(_)
                | TokenKind::LParen
                | TokenKind::LBracket,
            ) => self.pattern_atom(),
            _ => None,
        }
    }

    fn skip_balanced_braces(&mut self) {
        let mut depth = 0usize;
        while let Some(t) = self.peek() {
            match t {
                TokenKind::LBrace => depth += 1,
                TokenKind::RBrace => {
                    if depth == 0 {
                        return;
                    }
                    depth -= 1;
                    if depth == 0 {
                        self.i += 1;
                        return;
                    }
                }
                _ => {}
            }
            self.i += 1;
        }
    }

    // ----- expressions ---------------------------------------------------

    fn expr(&mut self) -> Expr {
        self.expr_prec(0, DoExpressionMode::Allow)
    }

    fn expr_no_do(&mut self) -> Expr {
        self.expr_prec(0, DoExpressionMode::Disallow)
    }

    /// Comma-separated expressions (signatory/observer/controller lists).
    fn expr_comma_list(&mut self) -> Vec<Expr> {
        let mut out = vec![self.expr()];
        while self.eat(&TokenKind::Comma) {
            out.push(self.expr());
        }
        out
    }

    fn expr_comma_list_no_do(&mut self) -> Vec<Expr> {
        let mut out = vec![self.expr_no_do()];
        while self.eat(&TokenKind::Comma) {
            out.push(self.expr_no_do());
        }
        out
    }

    fn expr_prec(&mut self, min_prec: u8, do_mode: DoExpressionMode) -> Expr {
        let pos = self.pos();
        let start_i = self.i;
        if self.depth >= MAX_RECURSION_DEPTH {
            // Hostile nesting: degrade to raw text instead of recursing, and
            // report it so the degraded region is not silently mistaken for
            // unsupported syntax. `skip_to_item_end` below consumes the rest of
            // the item, so this trips about once per affected declaration.
            self.diag_kind(
                ParseDiagnosticKind::RecursionLimit {
                    limit: MAX_RECURSION_DEPTH,
                },
                "expression nesting too deep; truncated to raw text",
            );
            let start = self.i;
            self.skip_to_item_end();
            if self.i == start {
                self.bump();
            }
            return Expr::Error {
                raw: self.slice_text(start),
                span: self.node_span(start),
                pos,
            };
        }
        self.depth += 1;
        let result = self.expr_prec_inner(min_prec, do_mode, pos, start_i);
        self.depth -= 1;
        result
    }

    fn expr_prec_inner(
        &mut self,
        min_prec: u8,
        do_mode: DoExpressionMode,
        pos: Pos,
        start_i: usize,
    ) -> Expr {
        let Some(mut lhs) = self.unary(do_mode) else {
            // Unparseable here: degrade to raw text up to the item end.
            let start = self.i;
            self.skip_to_item_end();
            if self.i == start {
                self.bump();
            }
            return Expr::Error {
                raw: self.slice_text(start),
                span: self.node_span(start),
                pos,
            };
        };
        loop {
            let (op, prec, right_assoc) = match self.peek() {
                Some(TokenKind::Op(o)) => {
                    let o = o.clone();
                    if is_reserved_op(&o) {
                        // `e : Type` annotation: consume the type, keep e.
                        if o == ":" {
                            self.bump();
                            self.skip_type_tokens();
                            continue;
                        }
                        break;
                    }
                    let (p, r) = fixity(&o);
                    (o, p, r)
                }
                Some(TokenKind::Backtick) => {
                    // `e `div` e` — infix function application.
                    let name = match self.peek_at(1) {
                        Some(
                            TokenKind::LowerId { qualifier, name }
                            | TokenKind::UpperId { qualifier, name },
                        ) => qualifier
                            .as_ref()
                            .map_or_else(|| name.to_string(), |q| format!("{q}.{name}")),
                        _ => break,
                    };
                    if self.peek_at(2) != Some(&TokenKind::Backtick) {
                        break;
                    }
                    (format!("`{name}`").into(), 9, false)
                }
                _ => break,
            };
            if prec < min_prec {
                break;
            }
            self.bump();
            if op.starts_with('`') {
                self.bump();
                self.bump();
            }
            let next_min = if right_assoc { prec } else { prec + 1 };
            let rhs = self.expr_prec(next_min, do_mode);
            lhs = Expr::BinOp {
                op,
                lhs: Box::new(lhs),
                rhs: Box::new(rhs),
                pos,
                span: self.node_span(start_i),
            };
        }
        lhs
    }

    fn unary(&mut self, do_mode: DoExpressionMode) -> Option<Expr> {
        let pos = self.pos();
        let start_i = self.i;
        if self.at_op("-") {
            self.bump();
            let e = self.unary(do_mode)?;
            return Some(Expr::Neg {
                expr: Box::new(e),
                pos,
                span: self.node_span(start_i),
            });
        }
        self.application(do_mode)
    }

    fn application(&mut self, do_mode: DoExpressionMode) -> Option<Expr> {
        let pos = self.pos();
        let start_i = self.i;
        let head0 = self.atom(do_mode)?;
        let mut head = self.projection_tail(head0);
        let mut args = Vec::new();
        loop {
            // Record syntax binds tighter than application:
            // `create Foo with x = 1` applies create to (Foo with {x = 1}).
            if self.at_keyword("with") {
                let target = args.pop().unwrap_or_else(|| {
                    std::mem::replace(
                        &mut head,
                        Expr::Error {
                            raw: String::new(),
                            pos,
                            span: Span::default(),
                        },
                    )
                });
                self.bump(); // with
                let fields = self.record_fields();
                let tpos = target.pos();
                let sp = Span::from_usize(target.span().start_usize(), self.end_byte());
                let rec = Expr::Record {
                    base: Box::new(target),
                    fields,
                    pos: tpos,
                    span: sp,
                };
                if matches!(head, Expr::Error { ref raw, .. } if raw.is_empty()) {
                    head = rec;
                } else {
                    args.push(rec);
                }
                continue;
            }
            if !do_mode.allows_do() && self.at_keyword("do") {
                break;
            }
            // Type application `f @Type x` — consume and drop the type atom.
            if self.at_op("@") {
                self.bump();
                match self.peek() {
                    Some(TokenKind::UpperId { .. } | TokenKind::LowerId { .. }) => {
                        self.bump();
                    }
                    Some(TokenKind::LParen) => self.skip_balanced_parens(),
                    Some(TokenKind::LBracket) => {
                        let mut depth = 0usize;
                        while let Some(t) = self.peek() {
                            match t {
                                TokenKind::LBracket => depth += 1,
                                TokenKind::RBracket => {
                                    if depth == 0 {
                                        break;
                                    }
                                    depth -= 1;
                                    if depth == 0 {
                                        self.i += 1;
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            self.i += 1;
                        }
                    }
                    _ => {}
                }
                continue;
            }
            match self.try_atom(do_mode) {
                Some(a) => args.push(self.projection_tail(a)),
                None => break,
            }
        }
        if args.is_empty() {
            Some(head)
        } else {
            Some(Expr::App {
                func: Box::new(head),
                args,
                pos,
                span: self.node_span(start_i),
            })
        }
    }

    /// `{ f = e ; g ; .. }` after `with` (virtual block) or explicit braces.
    fn record_fields(&mut self) -> Vec<FieldAssign> {
        let mut fields = Vec::new();
        let explicit = self.at(&TokenKind::LBrace);
        if !(self.eat(&TokenKind::VLBrace) || self.eat(&TokenKind::LBrace)) {
            return fields;
        }
        loop {
            while self.eat(&TokenKind::VSemi)
                || self.eat(&TokenKind::Semi)
                || self.eat(&TokenKind::Comma)
            {}
            match self.peek() {
                None => break,
                Some(TokenKind::VRBrace) if !explicit => {
                    self.bump();
                    break;
                }
                Some(TokenKind::RBrace) => {
                    self.bump();
                    break;
                }
                // Stray closer: discard so the loop always progresses.
                Some(TokenKind::RParen | TokenKind::RBracket) => {
                    self.bump();
                    continue;
                }
                _ => {}
            }
            let pos = self.pos();
            let start_i = self.i;
            if self.at_op("..") {
                self.bump();
                fields.push(FieldAssign::Wildcard {
                    pos,
                    span: self.node_span(start_i),
                });
                continue;
            }
            let name = match self.peek().cloned() {
                Some(TokenKind::LowerId {
                    qualifier: None,
                    name,
                }) => {
                    self.bump();
                    name
                }
                _ => {
                    self.skip_to_item_end();
                    continue;
                }
            };
            if self.eat_op("=") {
                let value = self.expr_prec(1, DoExpressionMode::Allow);
                fields.push(FieldAssign::Assign {
                    name,
                    value,
                    pos,
                    span: self.node_span(start_i),
                });
            } else {
                // Pun: `Foo with owner`.
                fields.push(FieldAssign::Pun {
                    name,
                    pos,
                    span: self.node_span(start_i),
                });
            }
        }
        fields
    }

    fn try_atom(&mut self, do_mode: DoExpressionMode) -> Option<Expr> {
        match self.peek() {
            Some(TokenKind::LowerId { .. }) => {
                let kw = self.peek().and_then(|t| t.keyword());
                match kw {
                    // Block argument: `script do ...`, `submit p do ...`.
                    Some("do") if do_mode.allows_do() => self.atom(do_mode),
                    // Keywords that begin expressions are fine as atoms in
                    // head position but must not be slurped as arguments.
                    Some(
                        "if" | "case" | "do" | "let" | "try" | "where" | "then" | "else" | "of"
                        | "in" | "controller" | "with" | "catch",
                    ) => None,
                    _ => self.atom(do_mode),
                }
            }
            Some(
                TokenKind::UpperId { .. }
                | TokenKind::IntLit(_)
                | TokenKind::DecimalLit(_)
                | TokenKind::StringLit(_)
                | TokenKind::CharLit(_)
                | TokenKind::LParen
                | TokenKind::LBracket,
            ) => self.atom(do_mode),
            // Bare trailing lambda argument: `forA xs \x -> ...`.
            Some(TokenKind::Op(o)) if o.as_str() == "\\" => self.atom(do_mode),
            _ => None,
        }
    }

    /// Fold tight (whitespace-free) `.field` record projections onto `base`.
    /// Projection binds tighter than application, so `length this.note` is
    /// `length (this.note)` not `(length this).note`, and a chain `a.b.c`
    /// left-nests. A *spaced* dot (`f . g`, composition) is not tight and is
    /// left to the binary-operator layer untouched. Qualified names
    /// (`Map.lookup`) are already a single token and never reach here.
    fn projection_tail(&mut self, mut base: Expr) -> Expr {
        while self.at_tight_projection() {
            let start = base.span().start_usize();
            let pos = base.pos();
            self.bump(); // '.'
            let Some(field_tok) = self.bump() else {
                self.diag_expected(
                    ExpectedToken::ProjectionFieldAfterDot,
                    "expected projection field after '.'",
                );
                return base;
            };
            let TokenKind::LowerId { qualifier, name } = field_tok.kind else {
                self.diag_expected(
                    ExpectedToken::ProjectionFieldAfterDot,
                    "expected projection field after '.'",
                );
                return base;
            };
            let field = Expr::Var {
                qualifier,
                name,
                pos: field_tok.pos,
                span: Span::from_usize(field_tok.start, field_tok.end),
            };
            base = Expr::BinOp {
                op: ".".into(),
                lhs: Box::new(base),
                rhs: Box::new(field),
                pos,
                span: Span::from_usize(start, self.end_byte()),
            };
        }
        base
    }

    /// True when the cursor sits on a `.` that abuts a real token on its left
    /// and an unqualified lowercase field on its right with no whitespace on
    /// either side — i.e. a record projection, not function composition.
    fn at_tight_projection(&self) -> bool {
        if self.i == 0 {
            return false;
        }
        let Some(dot) = self.toks.get(self.i) else {
            return false;
        };
        if !matches!(&dot.kind, TokenKind::Op(o) if o.as_str() == ".") {
            return false;
        }
        // Tight on the left: the dot abuts the base's last byte. The previous
        // token must be real — a virtual layout token here means a newline or
        // dedent sat between base and dot, which can never be a tight dot.
        let prev = &self.toks[self.i - 1];
        if prev.is_virtual() || prev.end != dot.start {
            return false;
        }
        // Tight on the right: an unqualified lowercase field abuts the dot.
        self.toks.get(self.i + 1).is_some_and(|t| {
            matches!(
                &t.kind,
                TokenKind::LowerId {
                    qualifier: None,
                    ..
                }
            ) && t.start == dot.end
        })
    }

    fn atom(&mut self, do_mode: DoExpressionMode) -> Option<Expr> {
        let pos = self.pos();
        let start_i = self.i;
        match self.peek().cloned() {
            Some(TokenKind::LowerId { qualifier, name }) => {
                match name.as_str() {
                    "if" if qualifier.is_none() => return self.if_expr(),
                    "case" if qualifier.is_none() => return self.case_expr(),
                    "do" if qualifier.is_none() => {
                        if !do_mode.allows_do() {
                            return None;
                        }
                        return self.do_expr();
                    }
                    "let" if qualifier.is_none() => return self.let_expr(),
                    "try" if qualifier.is_none() => return self.try_expr(),
                    _ => {}
                }
                self.bump();
                Some(Expr::Var {
                    qualifier,
                    name,
                    pos,
                    span: self.node_span(start_i),
                })
            }
            Some(TokenKind::UpperId { qualifier, name }) => {
                self.bump();
                let base = Expr::Con {
                    qualifier,
                    name,
                    pos,
                    span: self.node_span(start_i),
                };
                // Explicit-brace record syntax: `Foo {x = 1}`.
                if self.at(&TokenKind::LBrace) {
                    let fields = self.record_fields();
                    return Some(Expr::Record {
                        base: Box::new(base),
                        fields,
                        pos,
                        span: self.node_span(start_i),
                    });
                }
                Some(base)
            }
            Some(TokenKind::IntLit(text)) => {
                self.bump();
                Some(Expr::Lit {
                    kind: LitKind::Int,
                    text,
                    pos,
                    span: self.node_span(start_i),
                })
            }
            Some(TokenKind::DecimalLit(text)) => {
                self.bump();
                Some(Expr::Lit {
                    kind: LitKind::Decimal,
                    text,
                    pos,
                    span: self.node_span(start_i),
                })
            }
            Some(TokenKind::StringLit(text)) => {
                self.bump();
                Some(Expr::Lit {
                    kind: LitKind::Text,
                    text,
                    pos,
                    span: self.node_span(start_i),
                })
            }
            Some(TokenKind::CharLit(text)) => {
                self.bump();
                Some(Expr::Lit {
                    kind: LitKind::Char,
                    text,
                    pos,
                    span: self.node_span(start_i),
                })
            }
            Some(TokenKind::Op(o)) if o.as_str() == "\\" => self.lambda_expr(),
            Some(TokenKind::LParen) => self.paren_expr(),
            Some(TokenKind::LBracket) => self.list_expr(),
            _ => None,
        }
    }

    fn if_expr(&mut self) -> Option<Expr> {
        let pos = self.pos();
        let start_i = self.i;
        self.bump(); // if
        let cond = self.expr();
        self.eat(&TokenKind::VSemi); // DoAndIfThenElse style
        if !self.eat_keyword("then") {
            self.diag_expected(ExpectedToken::ThenKeyword, "expected 'then'");
            return Some(Expr::Error {
                raw: format!("if {}", cond.render()),
                pos,
                span: self.node_span(start_i),
            });
        }
        let then_branch = self.expr();
        self.eat(&TokenKind::VSemi);
        if !self.eat_keyword("else") {
            self.diag_expected(ExpectedToken::ElseKeyword, "expected 'else'");
            return Some(Expr::Error {
                raw: format!("if {} then {}", cond.render(), then_branch.render()),
                pos,
                span: self.node_span(start_i),
            });
        }
        let else_branch = self.expr();
        Some(Expr::If {
            cond: Box::new(cond),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
            pos,
            span: self.node_span(start_i),
        })
    }

    fn case_expr(&mut self) -> Option<Expr> {
        let pos = self.pos();
        let start_i = self.i;
        self.bump(); // case
        let scrutinee = self.expr_no_do();
        if !self.eat_keyword("of") {
            self.diag_expected(
                ExpectedToken::OfKeywordInCaseExpression,
                "expected 'of' in case expression",
            );
            return Some(Expr::Error {
                raw: format!("case {}", scrutinee.render()),
                pos,
                span: self.node_span(start_i),
            });
        }
        let mut alts = Vec::new();
        if self.eat(&TokenKind::VLBrace) || self.eat(&TokenKind::LBrace) {
            loop {
                while self.eat(&TokenKind::VSemi) || self.eat(&TokenKind::Semi) {}
                match self.peek() {
                    None => break,
                    Some(TokenKind::VRBrace | TokenKind::RBrace) => {
                        self.bump();
                        break;
                    }
                    // Stray closer: discard so the loop always progresses.
                    Some(TokenKind::RParen | TokenKind::RBracket) => {
                        self.bump();
                        continue;
                    }
                    _ => {}
                }
                // An alternative can carry a `where` block for its body.
                if self.eat_keyword("where") {
                    let _ = self.binding_block();
                    continue;
                }
                match self.case_alt() {
                    Some(a) => alts.push(a),
                    None => self.skip_to_item_end(),
                }
            }
        }
        Some(Expr::Case {
            scrutinee: Box::new(scrutinee),
            alts,
            pos,
            span: self.node_span(start_i),
        })
    }

    fn case_alt(&mut self) -> Option<Alt> {
        let pos = self.pos();
        let start_i = self.i;
        let pat = self.pattern()?;
        if self.at_op("|") {
            // Guarded alternative(s): take the first body, consume all.
            // Each guard is comma-separated qualifiers, each a boolean
            // expression or a pattern guard `pat <- expr`.
            let mut first: Option<Expr> = None;
            while self.eat_op("|") {
                loop {
                    let _guard = self.expr();
                    if self.eat_op("<-") {
                        let _ = self.expr();
                    }
                    if !self.eat(&TokenKind::Comma) {
                        break;
                    }
                }
                if !self.eat_op("->") {
                    self.diag_expected(
                        ExpectedToken::ArrowInGuardedCaseAlternative,
                        "expected '->' in guarded case alternative",
                    );
                    return None;
                }
                let body = self.expr();
                if first.is_none() {
                    first = Some(body);
                }
            }
            return Some(Alt {
                pat,
                body: first?,
                pos,
                span: self.node_span(start_i),
            });
        }
        if !self.eat_op("->") {
            self.diag_expected(
                ExpectedToken::ArrowInCaseAlternative,
                "expected '->' in case alternative",
            );
            return None;
        }
        let body = self.expr();
        Some(Alt {
            pat,
            body,
            pos,
            span: self.node_span(start_i),
        })
    }

    fn do_expr(&mut self) -> Option<Expr> {
        let pos = self.pos();
        let start_i = self.i;
        self.bump(); // do
        let mut stmts = Vec::new();
        if self.eat(&TokenKind::VLBrace) || self.eat(&TokenKind::LBrace) {
            loop {
                while self.eat(&TokenKind::VSemi) || self.eat(&TokenKind::Semi) {}
                match self.peek() {
                    None => break,
                    Some(TokenKind::VRBrace | TokenKind::RBrace) => {
                        self.bump();
                        break;
                    }
                    // Stray closer: discard so the loop always progresses.
                    Some(TokenKind::RParen | TokenKind::RBracket) => {
                        self.bump();
                        continue;
                    }
                    _ => {}
                }
                stmts.push(self.do_stmt());
            }
        }
        Some(Expr::Do {
            stmts,
            pos,
            span: self.node_span(start_i),
        })
    }

    fn do_stmt(&mut self) -> DoStmt {
        let pos = self.pos();
        let start_i = self.i;
        if self.at_keyword("let") {
            self.bump();
            let bindings = self.binding_block();
            // `let ... in body` as a statement is an expression.
            if self.eat_keyword("in") {
                let body = self.expr();
                return DoStmt::Expr {
                    expr: Expr::LetIn {
                        bindings,
                        body: Box::new(body),
                        pos,
                        span: self.node_span(start_i),
                    },
                    pos,
                    span: self.node_span(start_i),
                };
            }
            return DoStmt::Let {
                bindings,
                pos,
                span: self.node_span(start_i),
            };
        }
        // Try `pat <- expr` with rollback.
        let snapshot = self.i;
        if let Some(pat) = self.try_bind_pattern() {
            if self.at_op("<-") {
                self.bump();
                let expr = self.expr();
                return DoStmt::Bind {
                    pat,
                    expr,
                    pos,
                    span: self.node_span(start_i),
                };
            }
        }
        self.i = snapshot;
        let expr = self.expr();
        DoStmt::Expr {
            expr,
            pos,
            span: self.node_span(start_i),
        }
    }

    /// Pattern attempt for `pat <- ...`; restores nothing itself (caller
    /// rolls back on failure).
    fn try_bind_pattern(&mut self) -> Option<Pat> {
        self.pattern()
    }

    fn let_expr(&mut self) -> Option<Expr> {
        let pos = self.pos();
        let start_i = self.i;
        self.bump(); // let
        let bindings = self.binding_block();
        if self.eat_keyword("in") {
            let body = self.expr();
            return Some(Expr::LetIn {
                bindings,
                body: Box::new(body),
                pos,
                span: self.node_span(start_i),
            });
        }
        // `let` without `in` outside a do block — degrade gracefully.
        Some(Expr::LetIn {
            bindings,
            body: Box::new(Expr::Error {
                raw: String::new(),
                pos,
                span: self.node_span(start_i),
            }),
            pos,
            span: self.node_span(start_i),
        })
    }

    fn try_expr(&mut self) -> Option<Expr> {
        let pos = self.pos();
        let start_i = self.i;
        self.bump(); // try
        let body = self.expr();
        let mut handlers = Vec::new();
        self.eat(&TokenKind::VSemi);
        if self.eat_keyword("catch") {
            if self.eat(&TokenKind::VLBrace) || self.eat(&TokenKind::LBrace) {
                loop {
                    while self.eat(&TokenKind::VSemi) || self.eat(&TokenKind::Semi) {}
                    match self.peek() {
                        None => break,
                        Some(TokenKind::VRBrace | TokenKind::RBrace) => {
                            self.bump();
                            break;
                        }
                        // Stray closer: discard so the loop always progresses.
                        Some(TokenKind::RParen | TokenKind::RBracket) => {
                            self.bump();
                            continue;
                        }
                        _ => {}
                    }
                    match self.case_alt() {
                        Some(a) => handlers.push(a),
                        None => self.skip_to_item_end(),
                    }
                }
            } else if let Some(a) = self.case_alt() {
                // Single-alternative catch on the same line.
                handlers.push(a);
            }
        }
        Some(Expr::Try {
            body: Box::new(body),
            handlers,
            pos,
            span: self.node_span(start_i),
        })
    }

    fn lambda_expr(&mut self) -> Option<Expr> {
        let pos = self.pos();
        let start_i = self.i;
        self.bump(); // backslash
                     // `\case` — lambda-case: one implicit argument matched by the alts.
        if self.eat_keyword("case") {
            let mut alts = Vec::new();
            if self.eat(&TokenKind::VLBrace) || self.eat(&TokenKind::LBrace) {
                loop {
                    while self.eat(&TokenKind::VSemi) || self.eat(&TokenKind::Semi) {}
                    match self.peek() {
                        None => break,
                        Some(TokenKind::VRBrace | TokenKind::RBrace) => {
                            self.bump();
                            break;
                        }
                        Some(TokenKind::RParen | TokenKind::RBracket) => {
                            self.bump();
                            continue;
                        }
                        _ => {}
                    }
                    match self.case_alt() {
                        Some(a) => alts.push(a),
                        None => self.skip_to_item_end(),
                    }
                }
            }
            return Some(Expr::Lambda {
                params: vec![Pat::Var {
                    name: "_".into(),
                    pos,
                    span: Span::from_usize(self.byte_at(start_i), self.byte_at(start_i)),
                }],
                body: Box::new(Expr::Case {
                    scrutinee: Box::new(Expr::Var {
                        qualifier: None,
                        name: "_".into(),
                        pos,
                        span: Span::from_usize(self.byte_at(start_i), self.byte_at(start_i)),
                    }),
                    alts,
                    pos,
                    span: self.node_span(start_i),
                }),
                pos,
                span: self.node_span(start_i),
            });
        }
        let mut params = Vec::new();
        while !self.at_op("->") {
            match self.pattern_atom() {
                Some(p) => params.push(p),
                None => {
                    self.diag_malformed(
                        MalformedSyntaxKind::LambdaParameter,
                        "bad lambda parameter",
                    );
                    let start = self.i;
                    self.skip_to_item_end();
                    return Some(Expr::Error {
                        raw: format!("\\{}", self.slice_text(start)),
                        pos,
                        span: self.node_span(start_i),
                    });
                }
            }
        }
        self.bump(); // ->
        let body = self.expr();
        Some(Expr::Lambda {
            params,
            body: Box::new(body),
            pos,
            span: self.node_span(start_i),
        })
    }

    fn paren_expr(&mut self) -> Option<Expr> {
        let pos = self.pos();
        let start_i = self.i;
        self.bump(); // (
        if self.eat(&TokenKind::RParen) {
            return Some(Expr::Con {
                qualifier: None,
                name: "()".into(),
                pos,
                span: self.node_span(start_i),
            });
        }
        // Operator section / operator reference: `(+)`, `(+ 1)`.
        if let Some(TokenKind::Op(o)) = self.peek().cloned() {
            if !is_reserved_op(&o) && o != "\\" && o != "-" {
                self.bump();
                if self.eat(&TokenKind::RParen) {
                    return Some(Expr::OperatorRef {
                        op: o,
                        pos,
                        span: self.node_span(start_i),
                    });
                }
                let operand = self.expr();
                self.eat(&TokenKind::RParen);
                return Some(Expr::RightSection {
                    op: o,
                    operand: Box::new(operand),
                    pos,
                    span: self.node_span(start_i),
                });
            }
        }
        let first = self.expr();
        if self.at(&TokenKind::Comma) {
            let mut items = vec![first];
            while self.eat(&TokenKind::Comma) {
                items.push(self.expr());
            }
            self.eat(&TokenKind::RParen);
            return Some(Expr::Tuple {
                items,
                pos,
                span: self.node_span(start_i),
            });
        }
        // Left section: `(x +)`.
        if let Some(TokenKind::Op(o)) = self.peek().cloned() {
            if !is_reserved_op(&o) && self.peek_at(1) == Some(&TokenKind::RParen) {
                self.bump();
                self.bump();
                return Some(Expr::LeftSection {
                    op: o,
                    operand: Box::new(first),
                    pos,
                    span: self.node_span(start_i),
                });
            }
        }
        self.eat(&TokenKind::RParen);
        Some(first)
    }

    fn list_expr(&mut self) -> Option<Expr> {
        let pos = self.pos();
        let start_i = self.i;
        self.bump(); // [
        let mut items = Vec::new();
        if self.eat(&TokenKind::RBracket) {
            return Some(Expr::List {
                items,
                pos,
                span: self.node_span(start_i),
            });
        }
        loop {
            let e = self.expr();
            // Range: `[a .. b]` / `[a ..]`.
            if self.at_op("..") {
                self.bump();
                let hi = if self.at(&TokenKind::RBracket) {
                    Expr::Error {
                        raw: String::new(),
                        pos,
                        span: self.node_span(start_i),
                    }
                } else {
                    self.expr()
                };
                self.eat(&TokenKind::RBracket);
                return Some(Expr::BinOp {
                    op: "..".into(),
                    lhs: Box::new(e),
                    rhs: Box::new(hi),
                    pos,
                    span: self.node_span(start_i),
                });
            }
            // List comprehension: degrade the qualifier part to raw text.
            if self.at_op("|") {
                let start = self.i;
                let mut brackets = 1usize;
                while let Some(t) = self.peek() {
                    match t {
                        TokenKind::LBracket => brackets += 1,
                        TokenKind::RBracket => {
                            brackets -= 1;
                            if brackets == 0 {
                                break;
                            }
                        }
                        TokenKind::VSemi | TokenKind::VRBrace => break,
                        _ => {}
                    }
                    self.i += 1;
                }
                let raw = self.slice_text(start);
                self.eat(&TokenKind::RBracket);
                return Some(Expr::App {
                    func: Box::new(e),
                    args: vec![Expr::Error {
                        raw,
                        pos,
                        span: self.node_span(start_i),
                    }],
                    pos,
                    span: self.node_span(start_i),
                });
            }
            items.push(e);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.eat(&TokenKind::RBracket);
        Some(Expr::List {
            items,
            pos,
            span: self.node_span(start_i),
        })
    }
}

/// Operators that structure declarations and can never be expression infix
/// operators.
fn is_reserved_op(op: &str) -> bool {
    matches!(op, "=" | "<-" | "->" | "|" | ":" | "=>" | "@" | "\\" | "..")
}

/// (precedence, right-assoc) — Haskell defaults; unknown operators get
/// infixl 9.
fn fixity(op: &str) -> (u8, bool) {
    match op {
        "$" | "$!" => (1, true),
        ">>=" | ">>" | "=<<" | "<&>" => (2, false),
        "||" => (3, true),
        "&&" => (4, true),
        "==" | "/=" | "<" | "<=" | ">" | ">=" => (5, false),
        "::" | "++" | "<>" => (6, true),
        "+" | "-" => (7, false),
        "*" | "/" => (8, false),
        "^" | "**" => (9, true),
        "." | "!!" => (10, true),
        _ => (9, false),
    }
}

/// Merge type signatures and successive equations of the same function into
/// one `Decl::Function`, preserving first-seen order.
/// Bounding span of a function's equations (their first start to last end).
/// `None` for a signature-only function (no equations yet).
fn equations_extent(eqs: &[Equation]) -> Option<Span> {
    let mut it = eqs.iter();
    let first = it.next()?;
    let mut s = first.span;
    for e in it {
        s.start = s.start.min(e.span.start);
        s.end = s.end.max(e.span.end);
    }
    Some(s)
}

fn merge_functions(decls: &mut Vec<Decl>) {
    let mut out: Vec<Decl> = Vec::with_capacity(decls.len());
    let mut function_index_by_name: HashMap<Identifier, usize> = HashMap::new();
    for decl in decls.drain(..) {
        match decl {
            Decl::Function(f) => {
                if let Some(existing_index) = function_index_by_name.get(&f.name).copied() {
                    let Decl::Function(g) = &mut out[existing_index] else {
                        out.push(Decl::Function(f));
                        continue;
                    };
                    if g.ty.is_absent() {
                        g.ty = f.ty.clone();
                    }
                    if g.sig_span.is_none() {
                        g.sig_span = f.sig_span;
                    }
                    // The function's reported position is its first equation,
                    // not its type signature.
                    if g.equations.is_empty() && !f.equations.is_empty() {
                        g.pos = f.pos;
                    }
                    g.equations.extend(f.equations);
                    // A function's span is the extent of its equations, which
                    // are contiguous in well-formed source. A type signature
                    // can sit apart (with unrelated decls in between), so it is
                    // tracked in `sig_span` and never folded into `span` —
                    // doing so could straddle a sibling decl and break the
                    // nesting invariant.
                    g.span = equations_extent(&g.equations)
                        .or(g.sig_span)
                        .unwrap_or(g.span);
                } else {
                    function_index_by_name.insert(f.name.clone(), out.len());
                    out.push(Decl::Function(f));
                }
            }
            other => out.push(other),
        }
    }
    *decls = out;
}

/// Parse a type from a slice of the type's tokens (e.g. the tokens between a
/// field's `:` and the item end). PURE: it never touches the main parser cursor
/// and never affects any span, so it is invisible to daml-fmt. Returns `None`
/// when the whole slice does not parse cleanly as a type.
///
/// Grammar (precedence low → high): constraint `C => T`, function `a -> b`
/// (right-assoc), application `head arg...` (left-assoc), then atoms (`Con`,
/// `Var`, `[T]`, `()`, `(T)`, `(a, b)`).
pub(crate) fn parse_type_from_tokens(tokens: &[Token]) -> Option<Type> {
    // Virtual layout tokens carry no type meaning; drop them so a stray VSemi
    // in the slice can't sink an otherwise-clean parse.
    let real_tokens: Vec<&Token> = tokens.iter().filter(|t| !t.is_virtual()).collect();
    if real_tokens.is_empty() {
        return None;
    }
    let mut parser = TypeTokenParser {
        tokens: &real_tokens,
        cursor: 0,
    };
    let ty = parser.parse_type()?;
    // Require the whole slice to be consumed: a partial parse means the type
    // had a shape we don't model, so report unknown rather than a half-truth.
    if parser.cursor == real_tokens.len() {
        Some(ty)
    } else {
        None
    }
}

struct TypeTokenParser<'a> {
    tokens: &'a [&'a Token],
    cursor: usize,
}

#[derive(Debug)]
struct FieldBlock {
    fields: Vec<FieldDecl>,
    dangling: bool,
}

/// Result of parsing one atom: a real type, or a dropped type-level nat literal
/// (`Numeric 10` — not a type, so it never enters the App arg list).
enum TypeAtom {
    ParsedType(Type),
    DroppedLiteral(Span),
}

impl<'a> TypeTokenParser<'a> {
    fn peek(&self) -> Option<&'a Token> {
        self.tokens.get(self.cursor).copied()
    }

    fn eat_op(&mut self, op: &str) -> bool {
        if self.peek().is_some_and(|t| t.kind.is_op(op)) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    /// Full type: a constraint context `=> body`, or a function `a -> b`, or a
    /// bare application.
    fn parse_type(&mut self) -> Option<Type> {
        let lhs = if self.eat_keyword("forall") {
            self.parse_forall_type()?
        } else {
            self.parse_application_type()?
        };
        if self.eat_op("=>") {
            // `lhs` was the constraint context; drop it, keep the body.
            let body = self.parse_type()?;
            let span = Span::from_usize(lhs.span().start_usize(), body.span().end_usize());
            return Some(Type::Constrained(Box::new(body), span));
        }
        if self.eat_op("->") {
            let rhs = self.parse_type()?;
            let span = Span::from_usize(lhs.span().start_usize(), rhs.span().end_usize());
            return Some(Type::Fun(Box::new(lhs), Box::new(rhs), span));
        }
        Some(lhs)
    }

    /// Application spine: one head atom applied to zero or more argument atoms.
    fn parse_application_type(&mut self) -> Option<Type> {
        let head = match self.parse_atom()? {
            TypeAtom::ParsedType(t) => t,
            // A bare nat literal is not a type.
            TypeAtom::DroppedLiteral(_) => return None,
        };
        let mut args = Vec::new();
        let start = head.span().start_usize();
        let mut end = head.span().end_usize();
        loop {
            // Only continue the spine if the next token can START an atom; an
            // operator (`->`, `=>`) or closer ends it.
            if !self.is_at_type_atom_start() {
                break;
            }
            match self.parse_atom()? {
                TypeAtom::ParsedType(t) => {
                    end = t.span().end_usize();
                    args.push(t);
                }
                TypeAtom::DroppedLiteral(span) => {
                    // `Numeric 10` — drop the `10` as structure but keep it in
                    // the enclosing type span.
                    end = span.end_usize();
                }
            }
        }
        let span = Span::from_usize(start, end);
        if args.is_empty() {
            Some(head.with_span(span))
        } else {
            Some(Type::App(Box::new(head), args, span))
        }
    }

    /// True if the current token can begin an atom (so the application spine
    /// should keep going).
    fn is_at_type_atom_start(&self) -> bool {
        matches!(
            self.peek().map(|t| &t.kind),
            Some(
                TokenKind::UpperId { .. }
                    | TokenKind::LowerId { .. }
                    | TokenKind::IntLit(_)
                    | TokenKind::DecimalLit(_)
                    | TokenKind::StringLit(_)
                    | TokenKind::CharLit(_)
                    | TokenKind::LBracket
                    | TokenKind::LParen
            )
        )
    }

    fn parse_atom(&mut self) -> Option<TypeAtom> {
        let tok = self.peek()?;
        match &tok.kind {
            TokenKind::UpperId { qualifier, name } => {
                let con = Type::Con {
                    qualifier: qualifier.clone(),
                    name: name.clone(),
                    span: Span::from_usize(tok.start, tok.end),
                };
                self.cursor += 1;
                Some(TypeAtom::ParsedType(con))
            }
            TokenKind::LowerId { name, .. } => {
                // Type variable (`a`, `n`). Qualified lowercase never appears in
                // a real type position; treat the name as the variable.
                let var = Type::Var(name.clone(), Span::from_usize(tok.start, tok.end));
                self.cursor += 1;
                Some(TypeAtom::ParsedType(var))
            }
            TokenKind::IntLit(_) | TokenKind::DecimalLit(_) => {
                // Type-level nat literal (`Numeric 10`): consumed, but dropped.
                self.cursor += 1;
                Some(TypeAtom::DroppedLiteral(Span::from_usize(
                    tok.start, tok.end,
                )))
            }
            TokenKind::StringLit(text) => {
                self.cursor += 1;
                Some(TypeAtom::ParsedType(Type::Lit {
                    kind: LitKind::Text,
                    text: text.clone(),
                    span: Span::from_usize(tok.start, tok.end),
                }))
            }
            TokenKind::CharLit(text) => {
                self.cursor += 1;
                Some(TypeAtom::ParsedType(Type::Lit {
                    kind: LitKind::Char,
                    text: text.clone(),
                    span: Span::from_usize(tok.start, tok.end),
                }))
            }
            TokenKind::LBracket => {
                let start = tok.start;
                self.cursor += 1;
                let inner = self.parse_type()?;
                self.eat_token(&TokenKind::RBracket).map(|end| {
                    TypeAtom::ParsedType(Type::List(
                        Box::new(inner),
                        Span::from_usize(start, end.end),
                    ))
                })
            }
            TokenKind::LParen => {
                let start = tok.start;
                self.cursor += 1;
                if let Some(op) = self.eat_token_if_operator() {
                    let mut name = op.as_str().to_string();
                    while matches!(
                        self.tokens.get(self.cursor).map(|t| &t.kind),
                        Some(TokenKind::Op(_))
                    ) {
                        self.cursor += 1;
                        if let TokenKind::Op(o) = &self.tokens[self.cursor - 1].kind {
                            name.push_str(o.as_str());
                        }
                    }
                    if self.eat_token(&TokenKind::RParen).is_some()
                        && self
                            .tokens
                            .get(self.cursor)
                            .is_some_and(|t| Self::is_type_atom_start(&t.kind))
                    {
                        let end = self.tokens[self.cursor - 1];
                        return Some(TypeAtom::ParsedType(Type::Con {
                            qualifier: None,
                            name: name.into(),
                            span: Span::from_usize(start, end.end),
                        }));
                    }
                    return None;
                }
                if matches!(
                    self.tokens.get(self.cursor).map(|t| &t.kind),
                    Some(TokenKind::Comma)
                ) {
                    let mut name = String::from(",");
                    self.cursor += 1;
                    while matches!(
                        self.tokens.get(self.cursor).map(|t| &t.kind),
                        Some(TokenKind::Comma)
                    ) {
                        self.cursor += 1;
                        name.push(',');
                    }
                    if self.eat_token(&TokenKind::RParen).is_some()
                        && self
                            .tokens
                            .get(self.cursor)
                            .is_some_and(|t| Self::is_type_atom_start(&t.kind))
                    {
                        let end = self.tokens[self.cursor - 1];
                        return Some(TypeAtom::ParsedType(Type::Con {
                            qualifier: None,
                            name: name.into(),
                            span: Span::from_usize(start, end.end),
                        }));
                    }
                    return None;
                }
                if let Some(end) = self.eat_token(&TokenKind::RParen) {
                    // ()
                    return Some(TypeAtom::ParsedType(Type::Unit(Span::from_usize(
                        start, end.end,
                    ))));
                }
                let first = self.parse_type()?;
                if self.peek().map(|t| &t.kind) == Some(&TokenKind::Comma) {
                    let mut items = vec![first];
                    while self.eat_token(&TokenKind::Comma).is_some() {
                        items.push(self.parse_type()?);
                    }
                    self.eat_token(&TokenKind::RParen).map(|end| {
                        TypeAtom::ParsedType(Type::Tuple(items, Span::from_usize(start, end.end)))
                    })
                } else {
                    self.eat_token(&TokenKind::RParen).map(|end| {
                        // Grouping parens.
                        TypeAtom::ParsedType(first.with_span(Span::from_usize(start, end.end)))
                    })
                }
            }
            _ => None,
        }
    }

    fn eat_keyword(&mut self, kw: &str) -> bool {
        if self.peek().is_some_and(|t| t.kind.is_keyword(kw)) {
            self.cursor += 1;
            true
        } else {
            false
        }
    }

    fn parse_forall_type(&mut self) -> Option<Type> {
        let start = self
            .tokens
            .get(self.cursor.wrapping_sub(1))
            .map(|t| t.start)
            .unwrap_or_default();
        while self.cursor < self.tokens.len() {
            if self.peek().is_some_and(|t| t.kind.is_op(".")) {
                self.cursor += 1;
                let body = self.parse_type()?;
                let body_span = body.span();
                return Some(body.with_span(Span::from_usize(start, body_span.end_usize())));
            }
            self.cursor += 1;
        }
        None
    }

    fn eat_token(&mut self, tok: &TokenKind) -> Option<&'a Token> {
        if self.peek().is_some_and(|t| t.kind == *tok) {
            let t = self.peek();
            self.cursor += 1;
            t
        } else {
            None
        }
    }

    fn eat_token_if_operator(&mut self) -> Option<&'a Operator> {
        match self.peek() {
            Some(Token {
                kind: TokenKind::Op(op),
                ..
            }) => {
                self.cursor += 1;
                Some(op)
            }
            _ => None,
        }
    }

    const fn is_type_atom_start(kind: &TokenKind) -> bool {
        matches!(
            kind,
            TokenKind::UpperId { .. }
                | TokenKind::LowerId { .. }
                | TokenKind::IntLit(_)
                | TokenKind::DecimalLit(_)
                | TokenKind::StringLit(_)
                | TokenKind::CharLit(_)
                | TokenKind::LParen
                | TokenKind::LBracket
        )
    }
}

fn render_token_slice(tokens: &[Token]) -> String {
    let mut s = String::new();
    let mut prev_no_space_after = true;
    for t in tokens {
        let (text, no_space_before, no_space_after): (String, bool, bool) = match &t.kind {
            TokenKind::LowerId { qualifier, name } | TokenKind::UpperId { qualifier, name } => (
                qualifier
                    .as_ref()
                    .map_or_else(|| name.to_string(), |q| format!("{q}.{name}")),
                false,
                false,
            ),
            TokenKind::Op(o) => (o.to_string(), false, false),
            TokenKind::IntLit(n) | TokenKind::DecimalLit(n) => (n.clone(), false, false),
            TokenKind::StringLit(v) => (format!("{v:?}"), false, false),
            TokenKind::CharLit(v) => (format!("'{v}'"), false, false),
            TokenKind::LParen => ("(".to_string(), false, true),
            TokenKind::RParen => (")".to_string(), true, false),
            TokenKind::LBracket => ("[".to_string(), false, true),
            TokenKind::RBracket => ("]".to_string(), true, false),
            TokenKind::LBrace => ("{".to_string(), false, true),
            TokenKind::RBrace => ("}".to_string(), true, false),
            TokenKind::Comma => (",".to_string(), true, false),
            TokenKind::Semi | TokenKind::VSemi => (";".to_string(), true, false),
            TokenKind::Backtick => ("`".to_string(), false, false),
            TokenKind::VLBrace | TokenKind::VRBrace => continue,
        };
        if !s.is_empty() && !no_space_before && !prev_no_space_after {
            s.push(' ');
        }
        s.push_str(&text);
        prev_no_space_after = no_space_after;
    }
    s
}

// Unit tests for `parse_type_from_tokens` (private type-grammar phase) stay here;
// full-module type wiring and other observable parse behavior live in integration tests.
#[cfg(test)]
mod type_tests {
    use super::*;
    use crate::lexer::lex;

    /// Parse a bare type string straight through the lexer. A single-line type
    /// has no layout-significant newlines, so no virtual tokens appear — this
    /// exercises the private type-grammar phase in isolation.
    fn ty(s: &str) -> Option<Type> {
        let (toks, errs) = lex(s).into_parts();
        assert!(errs.is_empty(), "lex errors for {s:?}: {errs:?}");
        parse_type_from_tokens(&toks)
    }

    fn con(name: &str) -> Type {
        Type::Con {
            qualifier: None,
            name: name.into(),
            span: Span::default(),
        }
    }

    fn qualified_con(qualifier: &str, name: &str) -> Type {
        Type::Con {
            qualifier: Some(qualifier.into()),
            name: name.into(),
            span: Span::default(),
        }
    }

    fn app(head: Type, args: Vec<Type>) -> Type {
        Type::App(Box::new(head), args, Span::default())
    }

    fn list(inner: Type) -> Type {
        Type::List(Box::new(inner), Span::default())
    }

    fn tuple(items: Vec<Type>) -> Type {
        Type::Tuple(items, Span::default())
    }

    fn fun(param: Type, result: Type) -> Type {
        Type::Fun(Box::new(param), Box::new(result), Span::default())
    }

    fn var(name: &str) -> Type {
        Type::Var(name.into(), Span::default())
    }

    fn unit() -> Type {
        Type::Unit(Span::default())
    }

    fn constrained(body: Type) -> Type {
        Type::Constrained(Box::new(body), Span::default())
    }

    fn text_lit(value: &str) -> Type {
        Type::Lit {
            kind: LitKind::Text,
            text: value.to_string(),
            span: Span::default(),
        }
    }

    fn char_lit(value: &str) -> Type {
        Type::Lit {
            kind: LitKind::Char,
            text: value.to_string(),
            span: Span::default(),
        }
    }

    #[test]
    fn atoms() {
        assert_eq!(ty("Party"), Some(con("Party")));
        assert_eq!(ty("Decimal"), Some(con("Decimal")));
        assert_eq!(ty("a"), Some(var("a")));
        assert_eq!(ty("()"), Some(unit()));
    }

    #[test]
    fn application_vs_constructor() {
        // The whole point of the new model: `ContractId Foo` is an APPLICATION,
        // not one opaque name.
        assert_eq!(
            ty("ContractId Foo"),
            Some(app(con("ContractId"), vec![con("Foo")]))
        );
        assert_eq!(
            ty("Optional (ContractId Foo)"),
            Some(app(
                con("Optional"),
                vec![app(con("ContractId"), vec![con("Foo")])]
            ))
        );
        assert_eq!(
            ty("Map Text Int"),
            Some(app(con("Map"), vec![con("Text"), con("Int")]))
        );
    }

    #[test]
    fn qualified_constructor_keeps_qualifier() {
        assert_eq!(
            ty("DA.Map.Map Text Int"),
            Some(app(
                qualified_con("DA.Map", "Map"),
                vec![con("Text"), con("Int")]
            ))
        );
    }

    #[test]
    fn list_and_tuple() {
        assert_eq!(ty("[Text]"), Some(list(con("Text"))));
        assert_eq!(
            ty("(Int, Text)"),
            Some(tuple(vec![con("Int"), con("Text")]))
        );
        // A tuple is NOT a grouping paren — must stay a Tuple, never collapse.
        assert_eq!(
            ty("(a, b, c)"),
            Some(tuple(vec![var("a"), var("b"), var("c")]))
        );
        // Single grouping paren unwraps.
        assert_eq!(ty("(Text)"), Some(con("Text")));
    }

    #[test]
    fn function_types_are_arrows_not_names() {
        // These are exactly the corpus strings the old matcher swallowed into
        // one opaque `Named`.
        assert_eq!(ty("Int -> Int"), Some(fun(con("Int"), con("Int"))));
        // Right associativity: `a -> b -> c` == `a -> (b -> c)`.
        assert_eq!(
            ty("Int -> Text -> Bool"),
            Some(fun(con("Int"), fun(con("Text"), con("Bool"))))
        );
        assert_eq!(
            ty("Party -> Script ()"),
            Some(fun(con("Party"), app(con("Script"), vec![unit()])))
        );
    }

    #[test]
    fn script_application() {
        // `Script ()` ×147 in the corpus — an application flattened to `Named`
        // before. Now a real App.
        assert_eq!(ty("Script ()"), Some(app(con("Script"), vec![unit()])));
    }

    #[test]
    fn numeric_nat_literal_is_dropped() {
        // `Numeric 10`: the `10` is a type-level nat, not a type, so it drops
        // and the head Con stands alone. `Numeric n` keeps the type variable.
        assert_eq!(ty("Numeric 10"), Some(con("Numeric")));
        assert_eq!(ty("Numeric n"), Some(app(con("Numeric"), vec![var("n")])));
    }

    #[test]
    fn string_and_char_type_literals_are_structured() {
        // `HasField "observers"` — the field name is a type-level string
        // literal, not opaque syntax that collapses to malformed diagnostics.
        assert_eq!(
            ty(r#"HasField "observers""#),
            Some(app(con("HasField"), vec![text_lit("observers")]))
        );
        assert_eq!(
            ty(r#"HasField "observers" t PartiesMap"#),
            Some(app(
                con("HasField"),
                vec![text_lit("observers"), var("t"), con("PartiesMap")]
            ))
        );
        assert_eq!(
            ty(r"HasField 'x'"),
            Some(app(con("HasField"), vec![char_lit("x")]))
        );
    }

    #[test]
    fn constraint_context_is_dropped_body_kept() {
        // `NumericScale n => Numeric 37 -> Numeric n` — a constrained function.
        // Context dropped; body (the arrow) kept.
        assert_eq!(
            ty("NumericScale n => Numeric 37 -> Numeric n"),
            Some(constrained(fun(
                con("Numeric"),
                app(con("Numeric"), vec![var("n")])
            )))
        );
        // Tuple context `(Eq a, Show a) => a` also drops cleanly.
        assert_eq!(ty("(Eq a, Show a) => a"), Some(constrained(var("a"))));
    }

    #[test]
    fn unparseable_is_none() {
        // A trailing arrow with no body is not a clean type → unknown (None),
        // never a half-parse.
        assert_eq!(ty("Int ->"), None);
        assert_eq!(ty("-> Int"), None);
    }
}
