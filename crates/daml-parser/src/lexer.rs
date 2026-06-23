//! DAML lexer: source text → tokens with spans.
//!
//! First stage of the real parser pipeline (lexer → layout → parse). Comments
//! (line `--`, nested block `{- -}`) and string/char literals are resolved
//! here, so no later stage can ever mistake `-- exercise the option` for a
//! ledger action.

/// 1-based source position of a token's first character.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pos {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TokenKind {
    /// Lowercase-initial identifier, possibly qualified: `foo`, `Map.lookup`.
    LowerId {
        qualifier: Option<String>,
        name: String,
    },
    /// Uppercase-initial identifier, possibly qualified: `Foo`, `DA.Set.Set`.
    UpperId {
        qualifier: Option<String>,
        name: String,
    },
    /// Symbolic operator: `+`, `<-`, `->`, `=`, `=>`, `::`, `.`, `\`, ...
    Op(String),
    IntLit(String),
    DecimalLit(String),
    StringLit(String),
    CharLit(String),
    LParen,
    RParen,
    LBracket,
    RBracket,
    LBrace,
    RBrace,
    Comma,
    Semi,
    Backtick,
    /// Layout-inserted virtual open brace (block start).
    VLBrace,
    /// Layout-inserted virtual close brace (block end).
    VRBrace,
    /// Layout-inserted virtual semicolon (new item at block indentation).
    VSemi,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub(crate) kind: TokenKind,
    pub(crate) pos: Pos,
    /// Byte offset of the token's first character in the source.
    /// Virtual layout tokens are zero-width (`start == end`).
    pub(crate) start: usize,
    /// Byte offset one past the token's last character.
    pub(crate) end: usize,
}

impl Token {
    pub const fn kind(&self) -> &TokenKind {
        &self.kind
    }

    pub const fn pos(&self) -> Pos {
        self.pos
    }

    pub const fn start(&self) -> usize {
        self.start
    }

    pub const fn end(&self) -> usize {
        self.end
    }

    /// Layout-inserted tokens carry no source bytes (they are zero-width);
    /// AST node-span computation skips them so spans tile the real source.
    pub const fn is_virtual(&self) -> bool {
        matches!(
            self.kind,
            TokenKind::VLBrace | TokenKind::VRBrace | TokenKind::VSemi
        )
    }
}

/// Source text the lexer consumes but the parser never sees.
///
/// Carries exact byte spans so a printer can re-attach comments to nearby AST
/// nodes (which already have positions) and reproduce the original bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TriviaKind {
    /// `-- ...` to end of line (newline not included).
    LineComment,
    /// `{- ... -}`, possibly nested; unterminated runs to EOF.
    BlockComment,
    /// `#ifdef`/`#endif`/... preprocessor line at column 1.
    CppDirective,
    /// A run of N whitespace-only lines between tokens/comments.
    BlankLines(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Trivia {
    pub(crate) kind: TriviaKind,
    /// Exact source slice (delimiters included; empty for `BlankLines`).
    pub(crate) text: String,
    pub(crate) pos: Pos,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

impl Trivia {
    pub const fn kind(&self) -> &TriviaKind {
        &self.kind
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub const fn pos(&self) -> Pos {
        self.pos
    }

    pub const fn start(&self) -> usize {
        self.start
    }

    pub const fn end(&self) -> usize {
        self.end
    }
}

/// A lexical error. The scan must survive these: the caller reports the
/// diagnostic and works with the tokens produced so far.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexError {
    pub kind: LexErrorKind,
    pub pos: Pos,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.kind.fmt(f)
    }
}

impl std::error::Error for LexError {}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LexErrorKind {
    UnexpectedCharacter(char),
    UnterminatedBlockComment,
    UnterminatedStringLiteral,
    UnterminatedStringGap,
    InvalidEscapeSequence(char),
    StraySingleQuote,
    CharacterLiteralWrongLength,
    HexLiteralMissingDigits,
    DecimalExponentMissingDigits,
}

impl std::fmt::Display for LexErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedCharacter(c) => write!(f, "unexpected character '{c}'"),
            Self::UnterminatedBlockComment => f.write_str("unterminated block comment"),
            Self::UnterminatedStringLiteral => f.write_str("unterminated string literal"),
            Self::UnterminatedStringGap => f.write_str("unterminated string gap"),
            Self::InvalidEscapeSequence(c) => write!(f, "invalid escape sequence \\{c}"),
            Self::StraySingleQuote => f.write_str("stray single quote"),
            Self::CharacterLiteralWrongLength => {
                f.write_str("character literal must contain exactly one character")
            }
            Self::HexLiteralMissingDigits => f.write_str("hex literal requires at least one digit"),
            Self::DecimalExponentMissingDigits => {
                f.write_str("decimal exponent requires at least one digit")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexOutput {
    pub tokens: Vec<Token>,
    pub errors: Vec<LexError>,
}

impl LexOutput {
    pub fn into_parts(self) -> (Vec<Token>, Vec<LexError>) {
        (self.tokens, self.errors)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LexWithTriviaOutput {
    pub tokens: Vec<Token>,
    pub trivia: Vec<Trivia>,
    pub errors: Vec<LexError>,
}

impl LexWithTriviaOutput {
    pub fn into_parts(self) -> (Vec<Token>, Vec<Trivia>, Vec<LexError>) {
        (self.tokens, self.trivia, self.errors)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RenderLosslessError {
    OverlappingSpans {
        start: usize,
    },
    UncoveredBytes {
        start: usize,
        end: usize,
        text: String,
    },
    UncoveredTail {
        start: usize,
        text: String,
    },
}

impl std::fmt::Display for RenderLosslessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::OverlappingSpans { start } => write!(f, "overlapping spans at byte {start}"),
            Self::UncoveredBytes { start, end, text } => write!(
                f,
                "bytes {start}..{end} lost (not covered by any token/trivia): {text:?}"
            ),
            Self::UncoveredTail { start, text } => {
                write!(f, "bytes {start}.. lost at EOF: {text:?}")
            }
        }
    }
}

impl std::error::Error for RenderLosslessError {}

const fn is_symbol_char(c: char) -> bool {
    matches!(
        c,
        '!' | '#'
            | '$'
            | '%'
            | '&'
            | '*'
            | '+'
            | '.'
            | '/'
            | '<'
            | '='
            | '>'
            | '?'
            | '@'
            | '\\'
            | '^'
            | '|'
            | '-'
            | '~'
            | ':'
    )
}

fn is_ident_start(c: char) -> bool {
    c.is_alphabetic() || c == '_'
}

fn is_ident_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_' || c == '\''
}

pub(crate) const TAB_STOP: usize = 8;

struct Lexer<'a> {
    chars: Vec<char>,
    src: &'a str,
    i: usize,
    byte: usize,
    line: usize,
    column: usize,
    tokens: Vec<Token>,
    trivia: Vec<Trivia>,
    errors: Vec<LexError>,
}

pub fn lex(source: &str) -> LexOutput {
    let LexWithTriviaOutput { tokens, errors, .. } = lex_with_trivia(source);
    LexOutput { tokens, errors }
}

pub fn lex_with_trivia(source: &str) -> LexWithTriviaOutput {
    let mut lexer = Lexer {
        chars: source.chars().collect(),
        src: source,
        i: 0,
        byte: 0,
        line: 1,
        column: 1,
        tokens: Vec::new(),
        trivia: Vec::new(),
        errors: Vec::new(),
    };
    lexer.scan_tokens();
    let mut trivia = lexer.trivia;
    add_blank_line_trivia(source, &lexer.tokens, &mut trivia);
    trivia.sort_by_key(|t| t.start);
    LexWithTriviaOutput {
        tokens: lexer.tokens,
        trivia,
        errors: lexer.errors,
    }
}

/// Reconstruct the source from token and trivia spans.
///
/// `Ok` only when the spans tile the file — every non-whitespace byte inside
/// exactly one token or comment span — in which case the result is
/// byte-identical to `source`. This is the lossless-trivia oracle for the
/// formatter.
pub fn render_lossless(
    source: &str,
    tokens: &[Token],
    trivia: &[Trivia],
) -> Result<String, RenderLosslessError> {
    let mut items: Vec<(usize, usize)> = tokens
        .iter()
        .filter(|t| {
            !matches!(
                t.kind,
                TokenKind::VLBrace | TokenKind::VRBrace | TokenKind::VSemi
            )
        })
        .map(|t| (t.start, t.end))
        .chain(
            trivia
                .iter()
                .filter(|t| !matches!(t.kind, TriviaKind::BlankLines(_)))
                .map(|t| (t.start, t.end)),
        )
        .collect();
    items.sort_unstable();
    let mut out = String::with_capacity(source.len());
    let mut prev = 0usize;
    for (start, end) in items {
        if start < prev {
            return Err(RenderLosslessError::OverlappingSpans { start });
        }
        let gap = &source[prev..start];
        if !gap.chars().all(char::is_whitespace) {
            return Err(RenderLosslessError::UncoveredBytes {
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
        return Err(RenderLosslessError::UncoveredTail {
            start: prev,
            text: tail.to_string(),
        });
    }
    out.push_str(tail);
    Ok(out)
}

/// A blank line is a whitespace-only line lying entirely between spans (so a
/// blank-looking line inside a block comment or multiline string does not
/// count). Emitted as one `BlankLines(n)` per maximal run so the printer can
/// preserve paragraph breaks.
fn add_blank_line_trivia(source: &str, tokens: &[Token], trivia: &mut Vec<Trivia>) {
    let mut spans: Vec<(usize, usize)> = tokens
        .iter()
        .map(|t| (t.start, t.end))
        .chain(trivia.iter().map(|t| (t.start, t.end)))
        .collect();
    spans.sort_unstable();
    let bytes = source.as_bytes();
    let mut blanks = Vec::new();
    let mut gap_start = 0usize;
    let emit_gap = |from: usize, to: usize, out: &mut Vec<Trivia>| {
        // Newline offsets inside the gap. Full lines between two newlines
        // (or before the first newline when the gap starts the file) are
        // blank by construction: the gap holds no token/comment bytes, and
        // any stray non-whitespace there fails the lossless render anyway.
        let newlines: Vec<usize> = (from..to).filter(|&i| bytes[i] == b'\n').collect();
        // Interior gap: the partial line before the first newline belongs to
        // the preceding token's line, so only lines between newlines count.
        // A gap at byte 0 has no such partial line — line 1 itself is blank.
        let count = if from == 0 {
            newlines.len()
        } else {
            newlines.len().saturating_sub(1)
        };
        if count == 0 {
            return;
        }
        let region_start = if from == 0 { 0 } else { newlines[0] + 1 };
        let region_end = newlines[newlines.len() - 1] + 1;
        let line = source[..region_start].matches('\n').count() + 1;
        out.push(Trivia {
            kind: TriviaKind::BlankLines(count),
            text: String::new(),
            pos: Pos { line, column: 1 },
            start: region_start,
            end: region_end,
        });
    };
    for &(s, e) in &spans {
        if s > gap_start {
            emit_gap(gap_start, s, &mut blanks);
        }
        gap_start = gap_start.max(e);
    }
    if source.len() > gap_start {
        emit_gap(gap_start, source.len(), &mut blanks);
    }
    trivia.extend(blanks);
}

impl<'a> Lexer<'a> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.i).copied()
    }

    fn peek_at(&self, n: usize) -> Option<char> {
        self.chars.get(self.i + n).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.chars.get(self.i).copied()?;
        self.i += 1;
        self.byte += c.len_utf8();
        match c {
            '\n' => {
                self.line += 1;
                self.column = 1;
            }
            '\t' => {
                // Tab advances to the next multiple-of-8 stop, matching GHC,
                // so mixed tabs/spaces don't silently corrupt layout.
                self.column = ((self.column - 1) / TAB_STOP + 1) * TAB_STOP + 1;
            }
            _ => self.column += 1,
        }
        Some(c)
    }

    const fn pos(&self) -> Pos {
        Pos {
            line: self.line,
            column: self.column,
        }
    }

    /// `start` is the token's first byte; its end is wherever the cursor is
    /// now, so call this immediately after consuming the token.
    fn push(&mut self, tok: TokenKind, pos: Pos, start: usize) {
        self.tokens.push(Token {
            kind: tok,
            pos,
            start,
            end: self.byte,
        });
    }

    fn push_trivia(&mut self, kind: TriviaKind, pos: Pos, start: usize) {
        self.trivia.push(Trivia {
            kind,
            text: self.src[start..self.byte].to_string(),
            pos,
            start,
            end: self.byte,
        });
    }

    fn error(&mut self, kind: LexErrorKind, pos: Pos) {
        self.errors.push(LexError { kind, pos });
    }

    fn scan_tokens(&mut self) {
        while let Some(c) = self.peek() {
            let pos = self.pos();
            let start = self.byte;
            match c {
                ' ' | '\t' | '\n' | '\r' => {
                    self.bump();
                }
                '(' => {
                    self.bump();
                    self.push(TokenKind::LParen, pos, start);
                }
                ')' => {
                    self.bump();
                    self.push(TokenKind::RParen, pos, start);
                }
                '[' => {
                    self.bump();
                    self.push(TokenKind::LBracket, pos, start);
                }
                ']' => {
                    self.bump();
                    self.push(TokenKind::RBracket, pos, start);
                }
                ',' => {
                    self.bump();
                    self.push(TokenKind::Comma, pos, start);
                }
                ';' => {
                    self.bump();
                    self.push(TokenKind::Semi, pos, start);
                }
                '`' => {
                    self.bump();
                    self.push(TokenKind::Backtick, pos, start);
                }
                '{' => {
                    if self.peek_at(1) == Some('-') {
                        self.block_comment(pos);
                    } else {
                        self.bump();
                        self.push(TokenKind::LBrace, pos, start);
                    }
                }
                '}' => {
                    self.bump();
                    self.push(TokenKind::RBrace, pos, start);
                }
                // CPP preprocessor directive (#ifdef/#endif/#include...) at
                // column 1 — daml-prim/stdlib sources use {-# LANGUAGE CPP #-};
                // directives are line-based, skip the whole line.
                '#' if self.column == 1
                    && self.peek_at(1).is_some_and(|c| c.is_ascii_lowercase()) =>
                {
                    while self.peek().is_some_and(|c| c != '\n') {
                        self.bump();
                    }
                    self.push_trivia(TriviaKind::CppDirective, pos, start);
                }
                '"' => self.string_lit(pos),
                '\'' => self.char_lit(pos),
                c if c.is_ascii_digit() => self.number(pos),
                c if is_ident_start(c) => self.identifier(pos),
                c if is_symbol_char(c) => self.operator(pos),
                _ => {
                    self.bump();
                    self.error(LexErrorKind::UnexpectedCharacter(c), pos);
                }
            }
        }
    }

    /// `{- ... -}`, nested as in Haskell. Unterminated comment is an error
    /// but consumes to EOF (no hang, no panic).
    fn block_comment(&mut self, pos: Pos) {
        let start = self.byte;
        self.bump(); // {
        self.bump(); // -
        let mut depth = 1usize;
        while depth > 0 {
            match self.peek() {
                None => {
                    self.error(LexErrorKind::UnterminatedBlockComment, pos);
                    self.push_trivia(TriviaKind::BlockComment, pos, start);
                    return;
                }
                Some('{') if self.peek_at(1) == Some('-') => {
                    self.bump();
                    self.bump();
                    depth += 1;
                }
                Some('-') if self.peek_at(1) == Some('}') => {
                    self.bump();
                    self.bump();
                    depth -= 1;
                }
                Some(_) => {
                    self.bump();
                }
            }
        }
        self.push_trivia(TriviaKind::BlockComment, pos, start);
    }

    fn string_lit(&mut self, pos: Pos) {
        let start = self.byte;
        self.bump(); // opening "
        let mut value = String::new();
        loop {
            match self.peek() {
                None | Some('\n') => {
                    self.error(LexErrorKind::UnterminatedStringLiteral, pos);
                    break;
                }
                Some('"') => {
                    self.bump();
                    break;
                }
                Some('\\') => {
                    let escape_pos = self.pos();
                    self.bump();
                    match self.peek() {
                        // String gap: backslash, whitespace, backslash.
                        Some(w) if w.is_whitespace() => {
                            while self.peek().is_some_and(|c| c.is_whitespace()) {
                                self.bump();
                            }
                            if self.peek() == Some('\\') {
                                self.bump();
                            } else {
                                self.error(LexErrorKind::UnterminatedStringGap, pos);
                                break;
                            }
                        }
                        Some(e) => {
                            self.bump();
                            match unescape(e) {
                                Some(c) => value.push(c),
                                None => {
                                    self.error(LexErrorKind::InvalidEscapeSequence(e), escape_pos);
                                    value.push(e);
                                }
                            }
                        }
                        None => {
                            self.error(LexErrorKind::UnterminatedStringLiteral, pos);
                            break;
                        }
                    }
                }
                Some(c) => {
                    self.bump();
                    value.push(c);
                }
            }
        }
        self.push(TokenKind::StringLit(value), pos, start);
    }

    /// `'a'`, `'\n'`, `'\x41'`. A lone `'` that doesn't close within a few
    /// chars is not a char literal (identifiers consume their own primes, so
    /// this only triggers at expression positions).
    fn char_lit(&mut self, pos: Pos) {
        let start = self.byte;
        // Lookahead: find closing quote within a short window.
        let mut j = self.i + 1;
        let mut escaped = false;
        let mut ok = false;
        let window_end = (self.i + 12).min(self.chars.len());
        while j < window_end {
            match self.chars[j] {
                '\\' if !escaped => escaped = true,
                '\'' if !escaped => {
                    ok = j > self.i + 1;
                    break;
                }
                '\n' => break,
                _ => escaped = false,
            }
            j += 1;
        }
        if !ok {
            self.bump();
            self.error(LexErrorKind::StraySingleQuote, pos);
            return;
        }
        self.bump(); // opening '
        let mut value = String::new();
        while self.peek() != Some('\'') {
            let c = self.bump().unwrap();
            if c == '\\' {
                let escape_pos = Pos {
                    line: self.line,
                    column: self.column.saturating_sub(1),
                };
                if let Some(e) = self.bump() {
                    match unescape(e) {
                        Some(c) => value.push(c),
                        None => {
                            self.error(LexErrorKind::InvalidEscapeSequence(e), escape_pos);
                            value.push(e);
                        }
                    }
                }
            } else {
                value.push(c);
            }
        }
        self.bump(); // closing '
        if value.chars().count() != 1 {
            self.error(LexErrorKind::CharacterLiteralWrongLength, pos);
        }
        self.push(TokenKind::CharLit(value), pos, start);
    }

    fn number(&mut self, pos: Pos) {
        let start = self.byte;
        let mut text = String::new();
        if self.peek() == Some('0') && matches!(self.peek_at(1), Some('x' | 'X')) {
            text.push(self.bump().unwrap());
            text.push(self.bump().unwrap());
            let mut has_hex_digit = false;
            while self
                .peek()
                .is_some_and(|c| c.is_ascii_hexdigit() || c == '_')
            {
                let c = self.bump().unwrap();
                has_hex_digit |= c.is_ascii_hexdigit();
                text.push(c);
            }
            if !has_hex_digit {
                self.error(LexErrorKind::HexLiteralMissingDigits, pos);
            }
            self.push(TokenKind::IntLit(text), pos, start);
            return;
        }
        while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '_') {
            text.push(self.bump().unwrap());
        }
        let mut decimal = false;
        // `1.5` is a decimal but `1..5` or `1.foo` is not.
        if self.peek() == Some('.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
            decimal = true;
            text.push(self.bump().unwrap());
            while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '_') {
                text.push(self.bump().unwrap());
            }
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            decimal = true;
            if self.peek_at(1).is_some_and(|c| c.is_ascii_digit())
                || (matches!(self.peek_at(1), Some('+' | '-'))
                    && self.peek_at(2).is_some_and(|c| c.is_ascii_digit()))
            {
                text.push(self.bump().unwrap());
                if matches!(self.peek(), Some('+' | '-')) {
                    text.push(self.bump().unwrap());
                }
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    text.push(self.bump().unwrap());
                }
            } else {
                text.push(self.bump().unwrap());
                if matches!(self.peek(), Some('+' | '-')) {
                    text.push(self.bump().unwrap());
                }
                self.error(LexErrorKind::DecimalExponentMissingDigits, pos);
            }
        }
        if decimal {
            self.push(TokenKind::DecimalLit(text), pos, start);
        } else {
            self.push(TokenKind::IntLit(text), pos, start);
        }
    }

    /// Identifiers, with greedy qualification: `DA.Set.fromList` is one
    /// token (qualifier "DA.Set", name "fromList").
    fn identifier(&mut self, pos: Pos) {
        let start = self.byte;
        let mut segments: Vec<String> = Vec::new();
        loop {
            let mut seg = String::new();
            while self.peek().is_some_and(is_ident_char) {
                seg.push(self.bump().unwrap());
            }
            let seg_is_upper = seg.chars().next().is_some_and(|c| c.is_uppercase());
            segments.push(seg);
            // Continue qualification only after an Upper segment: `Foo.bar`
            // is qualified, `foo.bar` is composition/projection.
            if seg_is_upper
                && self.peek() == Some('.')
                && self.peek_at(1).is_some_and(is_ident_start)
            {
                self.bump(); // .
                continue;
            }
            break;
        }
        let name = segments.pop().unwrap();
        let qualifier = if segments.is_empty() {
            None
        } else {
            Some(segments.join("."))
        };
        let tok = if name.chars().next().is_some_and(|c| c.is_uppercase()) {
            TokenKind::UpperId { qualifier, name }
        } else {
            TokenKind::LowerId { qualifier, name }
        };
        self.push(tok, pos, start);
    }

    fn operator(&mut self, pos: Pos) {
        let start = self.i;
        let byte_start = self.byte;
        while self.peek().is_some_and(is_symbol_char) {
            // `{-` inside an operator run can't happen ({ isn't a symbol
            // char), but `--` comment detection needs the full run first.
            self.bump();
        }
        let text: String = self.chars[start..self.i].iter().collect();
        // A run of 2+ dashes and nothing else is a line comment (Haskell
        // rule: `-->` is an operator, `--` and `---` start comments).
        if text.len() >= 2 && text.chars().all(|c| c == '-') {
            while self.peek().is_some_and(|c| c != '\n') {
                self.bump();
            }
            self.push_trivia(TriviaKind::LineComment, pos, byte_start);
            return;
        }
        self.push(TokenKind::Op(text), pos, byte_start);
    }
}

const fn unescape(c: char) -> Option<char> {
    match c {
        'n' => Some('\n'),
        't' => Some('\t'),
        'r' => Some('\r'),
        '0' => Some('\0'),
        'a' => Some('\u{07}'),
        'b' => Some('\u{08}'),
        'f' => Some('\u{0c}'),
        'v' => Some('\u{0b}'),
        '"' => Some('"'),
        '\'' => Some('\''),
        '\\' => Some('\\'),
        '&' => Some('&'),
        // DAML follows Haskell-style text escapes, including numeric escapes
        // (`\123`, `\o173`, `\x7B`) and named ASCII escapes (`\NUL`, `\SOH`,
        // ...). This lexer preserves source spans rather than fully decoding
        // multi-character escapes here, so accept the leading escape character
        // and let the remaining source characters flow through unchanged.
        '1'..='9' | 'o' | 'x' | 'A'..='Z' => Some(c),
        _ => None,
    }
}

impl TokenKind {
    /// The identifier text if this is an unqualified lowercase identifier —
    /// how the parser checks for (contextual) keywords.
    pub const fn keyword(&self) -> Option<&str> {
        match self {
            Self::LowerId {
                qualifier: None,
                name,
            } => Some(name.as_str()),
            _ => None,
        }
    }

    pub fn is_keyword(&self, kw: &str) -> bool {
        self.keyword() == Some(kw)
    }

    pub fn is_op(&self, op: &str) -> bool {
        matches!(self, Self::Op(o) if o == op)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<TokenKind> {
        let (tokens, errors) = lex(src).into_parts();
        assert!(errors.is_empty(), "lex errors: {errors:?}");
        tokens.into_iter().map(|t| t.kind).collect()
    }

    fn lex_error_messages(src: &str) -> Vec<String> {
        let (_, errors) = lex(src).into_parts();
        errors.into_iter().map(|e| e.to_string()).collect()
    }

    fn lower(name: &str) -> TokenKind {
        TokenKind::LowerId {
            qualifier: None,
            name: name.to_string(),
        }
    }

    fn upper(name: &str) -> TokenKind {
        TokenKind::UpperId {
            qualifier: None,
            name: name.to_string(),
        }
    }

    #[test]
    fn line_comment_with_keywords_produces_no_tokens() {
        assert_eq!(toks("-- electing to exercise the option"), vec![]);
        assert_eq!(toks("--- template Foo"), vec![]);
    }

    #[test]
    fn arrow_like_operator_is_not_comment() {
        assert_eq!(
            toks("a --> b"),
            vec![lower("a"), TokenKind::Op("-->".into()), lower("b")]
        );
    }

    #[test]
    fn nested_block_comment() {
        assert_eq!(toks("{- outer {- inner -} still -} x"), vec![lower("x")]);
    }

    #[test]
    fn string_with_keyword_and_escapes() {
        assert_eq!(
            toks(r#""template \"Foo\" \n""#),
            vec![TokenKind::StringLit("template \"Foo\" \n".into())]
        );
    }

    #[test]
    fn qualified_identifiers() {
        assert_eq!(
            toks("DA.Set.fromList Map.Map foo"),
            vec![
                TokenKind::LowerId {
                    qualifier: Some("DA.Set".into()),
                    name: "fromList".into()
                },
                TokenKind::UpperId {
                    qualifier: Some("Map".into()),
                    name: "Map".into()
                },
                lower("foo"),
            ]
        );
    }

    #[test]
    fn numbers() {
        assert_eq!(
            toks("42 1.5 0x1F 2e3 1_000"),
            vec![
                TokenKind::IntLit("42".into()),
                TokenKind::DecimalLit("1.5".into()),
                TokenKind::IntLit("0x1F".into()),
                TokenKind::DecimalLit("2e3".into()),
                TokenKind::IntLit("1_000".into()),
            ]
        );
    }

    #[test]
    fn malformed_hex_literal_reports_error() {
        assert_eq!(
            lex_error_messages("0x 0x_"),
            vec![
                "hex literal requires at least one digit",
                "hex literal requires at least one digit",
            ]
        );
    }

    #[test]
    fn malformed_decimal_exponent_reports_error() {
        assert_eq!(
            lex_error_messages("1e 1e+ 1e-"),
            vec![
                "decimal exponent requires at least one digit",
                "decimal exponent requires at least one digit",
                "decimal exponent requires at least one digit",
            ]
        );
    }

    #[test]
    fn enum_from_to_is_not_decimal() {
        assert_eq!(
            toks("[1..5]"),
            vec![
                TokenKind::LBracket,
                TokenKind::IntLit("1".into()),
                TokenKind::Op("..".into()),
                TokenKind::IntLit("5".into()),
                TokenKind::RBracket,
            ]
        );
    }

    #[test]
    fn primes_stay_in_identifier_and_char_lit_works() {
        assert_eq!(
            toks(r"foo' 'a' '\n'"),
            vec![
                lower("foo'"),
                TokenKind::CharLit("a".into()),
                TokenKind::CharLit("\n".into())
            ]
        );
    }

    #[test]
    fn invalid_escape_sequences_report_errors() {
        assert_eq!(
            lex_error_messages(r#""\q" '\q'"#),
            vec!["invalid escape sequence \\q", "invalid escape sequence \\q"]
        );
    }

    #[test]
    fn multi_character_char_literal_reports_error() {
        let (tokens, errors) = lex("'ab'").into_parts();
        assert_eq!(
            tokens.iter().map(|t| t.kind.clone()).collect::<Vec<_>>(),
            vec![TokenKind::CharLit("ab".into())]
        );
        assert_eq!(
            errors.iter().map(|e| e.to_string()).collect::<Vec<_>>(),
            vec!["character literal must contain exactly one character".to_string()]
        );
    }

    #[test]
    fn operators_and_punctuation() {
        assert_eq!(
            toks("x <- f (y, z) `div` 2"),
            vec![
                lower("x"),
                TokenKind::Op("<-".into()),
                lower("f"),
                TokenKind::LParen,
                lower("y"),
                TokenKind::Comma,
                lower("z"),
                TokenKind::RParen,
                TokenKind::Backtick,
                lower("div"),
                TokenKind::Backtick,
                TokenKind::IntLit("2".into()),
            ]
        );
    }

    #[test]
    fn spans_are_one_based() {
        let (tokens, _) = lex("ab\n  cd").into_parts();
        assert_eq!(tokens[0].pos, Pos { line: 1, column: 1 });
        assert_eq!(tokens[1].pos, Pos { line: 2, column: 3 });
    }

    #[test]
    fn tab_advances_to_stop() {
        let (tokens, _) = lex("\tx").into_parts();
        assert_eq!(tokens[0].pos, Pos { line: 1, column: 9 });
    }

    #[test]
    fn unterminated_string_is_error_not_hang() {
        let (_, errors) = lex("x = \"oops\ny").into_parts();
        assert_eq!(errors.len(), 1);
    }

    #[test]
    fn unterminated_block_comment_is_error_not_hang() {
        let (_, errors) = lex("{- never closed").into_parts();
        assert_eq!(errors.len(), 1);
    }

    fn trivia_of(src: &str) -> Vec<Trivia> {
        let (_, trivia, _) = lex_with_trivia(src).into_parts();
        trivia
    }

    /// The lossless oracle on one source: spans must tile the file and the
    /// reconstruction must be byte-identical.
    fn assert_round_trip(src: &str) {
        let (tokens, trivia, errors) = lex_with_trivia(src).into_parts();
        assert!(errors.is_empty(), "lex errors: {errors:?}");
        assert_eq!(
            render_lossless(src, &tokens, &trivia).as_deref(),
            Ok(src),
            "round trip failed for {src:?}"
        );
    }

    #[test]
    fn line_comment_becomes_trivia_with_exact_text_and_span() {
        let src = "x = 1 -- electing to exercise\ny = 2\n";
        let trivia = trivia_of(src);
        assert_eq!(trivia.len(), 1);
        assert_eq!(trivia[0].kind, TriviaKind::LineComment);
        assert_eq!(trivia[0].text, "-- electing to exercise");
        assert_eq!(&src[trivia[0].start..trivia[0].end], trivia[0].text);
        assert_eq!(trivia[0].pos, Pos { line: 1, column: 7 });
    }

    #[test]
    fn nested_block_comment_becomes_one_trivia() {
        let src = "{- outer {- inner -} still -} x";
        let trivia = trivia_of(src);
        assert_eq!(trivia.len(), 1);
        assert_eq!(trivia[0].kind, TriviaKind::BlockComment);
        assert_eq!(trivia[0].text, "{- outer {- inner -} still -}");
    }

    #[test]
    fn unterminated_block_comment_still_yields_trivia_to_eof() {
        let (_, trivia, errors) = lex_with_trivia("x {- never closed").into_parts();
        assert_eq!(errors.len(), 1);
        assert_eq!(trivia.len(), 1);
        assert_eq!(trivia[0].text, "{- never closed");
    }

    #[test]
    fn blank_lines_between_items_counted() {
        let src = "x = 1\n\n\ny = 2\n";
        let trivia = trivia_of(src);
        assert_eq!(trivia.len(), 1);
        assert_eq!(trivia[0].kind, TriviaKind::BlankLines(2));
        assert_eq!(trivia[0].pos, Pos { line: 2, column: 1 });
    }

    #[test]
    fn blank_line_at_file_start_counted() {
        let trivia = trivia_of("\nx = 1\n");
        assert_eq!(trivia.len(), 1);
        assert_eq!(trivia[0].kind, TriviaKind::BlankLines(1));
        assert_eq!(trivia[0].pos, Pos { line: 1, column: 1 });
    }

    #[test]
    fn blank_looking_lines_inside_block_comment_are_not_blank_trivia() {
        let src = "x = 1 {- a\n\nb -}\ny = 2\n";
        let trivia = trivia_of(src);
        assert_eq!(trivia.len(), 1, "{trivia:?}");
        assert_eq!(trivia[0].kind, TriviaKind::BlockComment);
    }

    #[test]
    fn blank_line_between_comments_counted() {
        let src = "-- a\n\n-- b\nx = 1\n";
        let kinds: Vec<_> = trivia_of(src).into_iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TriviaKind::LineComment,
                TriviaKind::BlankLines(1),
                TriviaKind::LineComment,
            ]
        );
    }

    #[test]
    fn cpp_directive_becomes_trivia() {
        let src = "#ifdef DAML_BIGNUMERIC\nx = 1\n#endif\n";
        let trivia = trivia_of(src);
        assert_eq!(trivia.len(), 2);
        assert!(trivia.iter().all(|t| t.kind == TriviaKind::CppDirective));
        assert_eq!(trivia[0].text, "#ifdef DAML_BIGNUMERIC");
    }

    #[test]
    fn round_trip_is_byte_identical() {
        assert_round_trip("module M where\n\n-- doc\nf : Int -> Int\nf x = x + 1\n");
        assert_round_trip("x = \"tem\\\"plate \\n\" {- block {- nested -} -}\r\ny = 'a'\r\n");
        assert_round_trip("\tärger = [1..5] -- ütf\n");
        assert_round_trip("s = \"gap \\  \\ here\"\n");
        assert_round_trip("#ifdef X\nf = 0x1F\n#endif\n");
        assert_round_trip("\n\n  \nf = 1  \n   ");
        assert_round_trip("");
    }

    #[test]
    fn render_lossless_detects_lost_bytes() {
        let src = "x = 1 -- comment\n";
        let (tokens, mut trivia, _) = lex_with_trivia(src).into_parts();
        trivia.clear(); // simulate a lexer that drops the comment
        assert!(render_lossless(src, &tokens, &trivia).is_err());
    }

    #[test]
    fn unicode_identifier() {
        assert_eq!(
            toks("ärger = 1"),
            vec![
                lower("ärger"),
                TokenKind::Op("=".into()),
                TokenKind::IntLit("1".into())
            ]
        );
        let _ = upper("Ülf"); // helper used
    }
}
