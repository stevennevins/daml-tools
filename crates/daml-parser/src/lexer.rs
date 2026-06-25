//! DAML lexer: source text → tokens with spans.
//!
//! First stage of the real parser pipeline (lexer → layout → parse). Comments
//! (line `--`, nested block `{- -}`) and string/char literals are resolved
//! here, so no later stage can ever mistake `-- exercise the option` for a
//! ledger action.

/// A small domain type for identifier-like text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Identifier(String);

impl Identifier {
    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for Identifier {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for Identifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<String> for Identifier {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Identifier {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<Identifier> for String {
    fn from(value: Identifier) -> Self {
        value.0
    }
}

impl std::ops::Deref for Identifier {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Identifier {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq<&str> for Identifier {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<Identifier> for &str {
    fn eq(&self, other: &Identifier) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<String> for Identifier {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<Identifier> for String {
    fn eq(&self, other: &Identifier) -> bool {
        self.as_str() == other.as_str()
    }
}

/// A small domain type for symbolic operator text.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Operator(String);

impl Operator {
    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for Operator {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for Operator {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<String> for Operator {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Operator {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<Operator> for String {
    fn from(value: Operator) -> Self {
        value.0
    }
}

impl std::ops::Deref for Operator {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for Operator {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq<&str> for Operator {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<Operator> for &str {
    fn eq(&self, other: &Operator) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<String> for Operator {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<Operator> for String {
    fn eq(&self, other: &Operator) -> bool {
        self.as_str() == other.as_str()
    }
}

/// A small domain type for module-style qualified names (`DA.Map`, `Daml.Foo`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct ModuleName(String);

impl ModuleName {
    #[must_use]
    pub const fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl AsRef<str> for ModuleName {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl std::fmt::Display for ModuleName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<String> for ModuleName {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for ModuleName {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<Identifier> for ModuleName {
    fn from(value: Identifier) -> Self {
        Self(value.0)
    }
}

impl From<ModuleName> for String {
    fn from(value: ModuleName) -> Self {
        value.0
    }
}

impl std::ops::Deref for ModuleName {
    type Target = str;
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

impl std::borrow::Borrow<str> for ModuleName {
    fn borrow(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq<&str> for ModuleName {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl PartialEq<ModuleName> for &str {
    fn eq(&self, other: &ModuleName) -> bool {
        *self == other.as_str()
    }
}

impl PartialEq<String> for ModuleName {
    fn eq(&self, other: &String) -> bool {
        self.as_str() == other.as_str()
    }
}

impl PartialEq<ModuleName> for String {
    fn eq(&self, other: &ModuleName) -> bool {
        self.as_str() == other.as_str()
    }
}

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
        qualifier: Option<ModuleName>,
        name: Identifier,
    },
    /// Uppercase-initial identifier, possibly qualified: `Foo`, `DA.Set.Set`.
    UpperId {
        qualifier: Option<ModuleName>,
        name: Identifier,
    },
    /// Symbolic operator: `+`, `<-`, `->`, `=`, `=>`, `::`, `.`, `\`, ...
    Op(Operator),
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
    #[must_use]
    pub const fn kind(&self) -> &TokenKind {
        &self.kind
    }

    #[must_use]
    pub const fn pos(&self) -> Pos {
        self.pos
    }

    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }

    #[must_use]
    pub const fn end(&self) -> usize {
        self.end
    }

    /// Layout-inserted tokens carry no source bytes (they are zero-width);
    /// AST node-span computation skips them so spans tile the real source.
    #[must_use]
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
    #[must_use]
    pub const fn kind(&self) -> &TriviaKind {
        &self.kind
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub const fn pos(&self) -> Pos {
        self.pos
    }

    #[must_use]
    pub const fn start(&self) -> usize {
        self.start
    }

    #[must_use]
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

impl LexError {
    /// Byte span of the offending range, where available.
    #[must_use]
    pub fn byte_range_in(&self, source: &str) -> std::ops::Range<usize> {
        let start = byte_of_pos(source, self.pos);
        if start >= source.len() {
            return start..start;
        }

        match &self.kind {
            LexErrorKind::UnexpectedCharacter(c) => {
                let end = (start + c.len_utf8()).min(source.len());
                start..end
            }
            LexErrorKind::UnterminatedStringLiteral
            | LexErrorKind::UnterminatedStringGap
            | LexErrorKind::StraySingleQuote => {
                let end = (start + 1).min(source.len());
                start..end
            }
            LexErrorKind::UnterminatedBlockComment => start..source.len(),
            LexErrorKind::InvalidEscapeSequence(_) => {
                let first = char_len_at(source, start).unwrap_or(0);
                let mut end = (start + first).min(source.len());
                if let Some(second) = source.get(end..).and_then(|s| s.chars().next()) {
                    end = (end + second.len_utf8()).min(source.len());
                }
                start..end
            }
            LexErrorKind::CharacterLiteralWrongLength => start..end_char_lit_error(source, start),
            LexErrorKind::HexLiteralMissingDigits => start..end_hex_missing_digits(source, start),
            LexErrorKind::DecimalExponentMissingDigits => {
                start..end_decimal_exponent_missing_digits(source, start)
            }
        }
    }
}
impl std::error::Error for LexError {}

#[inline]
fn char_len_at(source: &str, byte: usize) -> Option<usize> {
    source
        .get(byte..)
        .and_then(|s| s.chars().next())
        .map(char::len_utf8)
}

fn end_char_lit_error(source: &str, start: usize) -> usize {
    if start >= source.len() {
        return start;
    }
    if char_len_at(source, start).is_none() {
        return start;
    }

    let mut i = start + char_len_at(source, start).unwrap_or(0);
    let mut escaped = false;
    while i < source.len() {
        let len = char_len_at(source, i).unwrap_or(0);
        if len == 0 {
            return start + 1;
        }
        let ch = source[i..i + len]
            .chars()
            .next()
            .expect("positive char_len_at guarantees one UTF-8 scalar");
        i += len;

        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }
        if ch == '\'' {
            return i;
        }
    }

    start + 1
}

fn end_hex_missing_digits(source: &str, start: usize) -> usize {
    if start >= source.len() {
        return start;
    }
    let after_prefix = start.saturating_add(2);
    let prefix = source.get(start..after_prefix).unwrap_or("");
    if prefix != "0x" && prefix != "0X" {
        return start;
    }

    let mut end = after_prefix;
    while let Some(len) = char_len_at(source, end) {
        if source.get(end..end + len) != Some("_") {
            break;
        }
        end += len;
    }
    end
}

fn end_decimal_exponent_missing_digits(source: &str, start: usize) -> usize {
    let mut byte = start;
    while byte < source.len() {
        let Some(len) = char_len_at(source, byte) else {
            return start;
        };
        if len == 0 {
            return start;
        }
        let ch = source[byte..byte + len]
            .chars()
            .next()
            .expect("positive char_len_at guarantees one UTF-8 scalar");
        if matches!(ch, 'e' | 'E') {
            let mut end = byte + len;
            if let Some(sign_len) = char_len_at(source, end) {
                let sign = source[end..end + sign_len]
                    .chars()
                    .next()
                    .expect("positive char_len_at guarantees one UTF-8 scalar");
                if matches!(sign, '+' | '-') {
                    end += sign_len;
                }
            }
            return end;
        }

        if ch.is_whitespace() {
            return start;
        }
        byte += len;
    }

    start
}

/// Byte offset of a 1-based (line, column) position.
#[inline]
fn byte_of_pos(source: &str, pos: Pos) -> usize {
    let mut line = 1usize;
    let mut col = 1usize;
    for (idx, ch) in source.char_indices() {
        if line == pos.line && col == pos.column {
            return idx;
        }
        match ch {
            '\n' => {
                line += 1;
                col = 1;
            }
            '\t' => col = ((col - 1) / TAB_STOP + 1) * TAB_STOP + 1,
            _ => col += 1,
        }
    }
    source.len()
}

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
    #[must_use]
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
    #[must_use]
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
    capture_trivia: bool,
    tokens: Vec<Token>,
    trivia: Vec<Trivia>,
    errors: Vec<LexError>,
}

impl<'a> Lexer<'a> {
    fn new(source: &'a str, capture_trivia: bool) -> Self {
        Self {
            chars: source.chars().collect(),
            src: source,
            i: 0,
            byte: 0,
            line: 1,
            column: 1,
            capture_trivia,
            tokens: Vec::new(),
            trivia: Vec::new(),
            errors: Vec::new(),
        }
    }
}

/// Lex `source` into tokens and lexical errors only.
///
/// Skips trivia allocation and blank-line processing. Use [`lex_with_trivia`]
/// when callers also need comments and trivia for lossless rendering.
#[must_use]
pub fn lex(source: &str) -> LexOutput {
    let mut lexer = Lexer::new(source, false);
    lexer.scan_tokens();
    LexOutput {
        tokens: lexer.tokens,
        errors: lexer.errors,
    }
}

/// Lex `source` into tokens, trivia, and lexical errors.
#[must_use]
pub fn lex_with_trivia(source: &str) -> LexWithTriviaOutput {
    let mut lexer = Lexer::new(source, true);
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
///
/// # Errors
///
/// Returns [`RenderLosslessError`] when token/trivia spans overlap, leave gaps,
/// or extend past the end of `source`.
#[must_use = "handle render errors instead of discarding"]
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

    /// Next char when `peek` / `peek_at` already confirmed input remains.
    fn bump_after_peek(&mut self) -> char {
        self.bump()
            .expect("lexer cursor guarded by peek/peek_at before bump")
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
        if !self.capture_trivia {
            return;
        }
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
            let c = self.bump_after_peek();
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
            text.push(self.bump_after_peek());
            text.push(self.bump_after_peek());
            let mut has_hex_digit = false;
            while self
                .peek()
                .is_some_and(|c| c.is_ascii_hexdigit() || c == '_')
            {
                let c = self.bump_after_peek();
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
            text.push(self.bump_after_peek());
        }
        let mut decimal = false;
        // `1.5` is a decimal but `1..5` or `1.foo` is not.
        if self.peek() == Some('.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
            decimal = true;
            text.push(self.bump_after_peek());
            while self.peek().is_some_and(|c| c.is_ascii_digit() || c == '_') {
                text.push(self.bump_after_peek());
            }
        }
        if matches!(self.peek(), Some('e' | 'E')) {
            decimal = true;
            if self.peek_at(1).is_some_and(|c| c.is_ascii_digit())
                || (matches!(self.peek_at(1), Some('+' | '-'))
                    && self.peek_at(2).is_some_and(|c| c.is_ascii_digit()))
            {
                text.push(self.bump_after_peek());
                if matches!(self.peek(), Some('+' | '-')) {
                    text.push(self.bump_after_peek());
                }
                while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                    text.push(self.bump_after_peek());
                }
            } else {
                text.push(self.bump_after_peek());
                if matches!(self.peek(), Some('+' | '-')) {
                    text.push(self.bump_after_peek());
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
                seg.push(self.bump_after_peek());
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
        let name = segments
            .pop()
            .expect("identifier loop always records at least one segment");
        let qualifier = if segments.is_empty() {
            None
        } else {
            Some(segments.join(".").into())
        };
        let tok = if name.chars().next().is_some_and(|c| c.is_uppercase()) {
            TokenKind::UpperId {
                qualifier,
                name: name.into(),
            }
        } else {
            TokenKind::LowerId {
                qualifier,
                name: name.into(),
            }
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
        self.push(TokenKind::Op(text.into()), pos, byte_start);
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
    #[must_use]
    pub const fn keyword(&self) -> Option<&str> {
        match self {
            Self::LowerId {
                qualifier: None,
                name,
            } => Some(name.as_str()),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_keyword(&self, kw: &str) -> bool {
        self.keyword() == Some(kw)
    }

    #[must_use]
    pub fn is_op(&self, op: &str) -> bool {
        matches!(self, Self::Op(o) if o.as_str() == op)
    }
}

// Tokenization, trivia, and lex/lex_with_trivia parity contracts for the lexer phase.
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
            name: name.into(),
        }
    }

    fn upper(name: &str) -> TokenKind {
        TokenKind::UpperId {
            qualifier: None,
            name: name.into(),
        }
    }

    #[test]
    fn identifier_as_ref_str() {
        let identifier = Identifier::from("value");
        let operator = Operator::from("+");
        let module = ModuleName::from("DA.Map");

        assert_eq!(identifier.as_ref(), "value");
        assert_eq!(operator.as_ref(), "+");
        assert_eq!(module.as_ref(), "DA.Map");
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

    /// Parse-only `lex` must emit the same tokens as `lex_with_trivia`; only
    /// trivia differs. Comments and blank lines must not change tokenization.
    fn assert_lex_tokens_match(src: &str) {
        let (parse_tokens, parse_errors) = lex(src).into_parts();
        let (trivia_tokens, _, trivia_errors) = lex_with_trivia(src).into_parts();
        assert_eq!(parse_errors, trivia_errors, "lex errors differ for {src:?}");
        assert_eq!(
            parse_tokens.len(),
            trivia_tokens.len(),
            "token count for {src:?}"
        );
        for (a, b) in parse_tokens.iter().zip(trivia_tokens.iter()) {
            assert_eq!(a.kind, b.kind, "token kind for {src:?}");
            assert_eq!(a.pos, b.pos, "token pos for {src:?}");
            assert_eq!(a.start, b.start, "token start for {src:?}");
            assert_eq!(a.end, b.end, "token end for {src:?}");
        }
    }

    #[test]
    fn lex_and_lex_with_trivia_emit_identical_tokens() {
        assert_lex_tokens_match("x = 1 -- electing to exercise\ny = 2\n");
        assert_lex_tokens_match("module M where\n\n-- doc\nf : Int -> Int\nf x = x + 1\n");
        assert_lex_tokens_match("{- outer {- inner -} still -} x");
        assert_lex_tokens_match("#ifdef DAML_BIGNUMERIC\nx = 1\n#endif\n");
        assert_lex_tokens_match("\n\n  \nf = 1  \n   ");
    }

    #[test]
    fn lex_with_trivia_preserves_lossless_comment_and_blank_line_render() {
        let sources = [
            "x = 1 -- electing to exercise\ny = 2\n",
            "x = 1\n\n\ny = 2\n",
            "-- a\n\n-- b\nx = 1\n",
        ];
        for src in sources {
            let (tokens, trivia, errors) = lex_with_trivia(src).into_parts();
            assert!(errors.is_empty(), "lex errors for {src:?}: {errors:?}");
            assert_eq!(
                render_lossless(src, &tokens, &trivia).as_deref(),
                Ok(src),
                "lossless render failed for {src:?}"
            );
        }
    }
}
