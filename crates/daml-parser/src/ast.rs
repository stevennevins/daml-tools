//! Typed, lossless parse tree produced by the recursive-descent parser
//! (src/parse.rs).
//!
//! Every node carries a source position and byte span. Downstream crates
//! consume this tree directly: daml-fmt re-prints layout from the spans, and
//! daml-lint lowers it onto its own rule-facing IR.

pub use crate::lexer::{ByteOffset, Identifier, ModuleName, Operator, Pos};

/// Byte span of an AST node.
///
/// `[start, end)` into the original source, same basis as `Token::start`/
/// `Token::end`. Covers every (non-virtual) token that belongs to the node —
/// first token's `start` to last token's `end`.
///
/// Invariants the parser maintains (checked over the corpus by
/// `render_from_ast`): a child's span is contained in its parent's span, and
/// sibling spans are ordered and non-overlapping. Trivia (comments, blank
/// lines) live *between* sibling spans.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Span {
    /// Inclusive byte offset at which the node starts.
    pub start: ByteOffset,
    /// Exclusive byte offset at which the node ends.
    pub end: ByteOffset,
}

impl Span {
    /// Create a span from already-typed byte offsets.
    ///
    /// Use [`Self::from_usize`] only at parser/source-slicing boundaries where
    /// raw byte offsets are being converted deliberately.
    ///
    /// ```compile_fail
    /// use daml_parser::ast::Span;
    ///
    /// let _ = Span::new(1usize, 2usize);
    /// ```
    #[must_use]
    pub const fn new(start: ByteOffset, end: ByteOffset) -> Self {
        Self { start, end }
    }

    /// Convert raw byte offsets into a typed parser span.
    #[must_use]
    pub const fn from_usize(start: usize, end: usize) -> Self {
        Self {
            start: ByteOffset::new(start),
            end: ByteOffset::new(end),
        }
    }

    /// Raw start byte offset for source slicing and external interop.
    #[must_use]
    pub const fn start_usize(self) -> usize {
        self.start.get()
    }

    /// Raw exclusive end byte offset for source slicing and external interop.
    #[must_use]
    pub const fn end_usize(self) -> usize {
        self.end.get()
    }

    /// True when the span is well-formed (`start <= end`).
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.start.get() <= self.end.get()
    }

    /// True for a zero-width but still valid span.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start.get() == self.end.get()
    }

    #[must_use]
    pub const fn range(&self) -> std::ops::Range<usize> {
        self.start.get()..self.end.get()
    }

    #[must_use]
    pub fn get<'a>(&self, source: &'a str) -> Option<&'a str> {
        let start = self.start.get();
        let end = self.end.get();
        if start <= end
            && end <= source.len()
            && source.is_char_boundary(start)
            && source.is_char_boundary(end)
        {
            Some(&source[start..end])
        } else {
            None
        }
    }

    /// `self` fully contains `other`.
    #[must_use]
    pub const fn contains(&self, other: &Self) -> bool {
        self.is_valid()
            && other.is_valid()
            && self.start.get() <= other.start.get()
            && other.end.get() <= self.end.get()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum LitKind {
    Int,
    Decimal,
    Text,
    Char,
}

/// Import syntax style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ImportStyle {
    /// Qualified import (`import qualified Foo.Bar`, `import Foo.Bar qualified`).
    Qualified,
    /// Unqualified import (`import Foo.Bar`, `import Foo.Bar as Baz`).
    Unqualified,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldAssign {
    /// Explicit record assignment: `field = expression`.
    Assign {
        name: Identifier,
        value: Expr,
        pos: Pos,
        span: Span,
    },
    /// Record pun: `field`, meaning `field = field`.
    Pun {
        name: Identifier,
        pos: Pos,
        span: Span,
    },
    /// Record wildcard: `..`.
    Wildcard { pos: Pos, span: Span },
}

impl FieldAssign {
    #[must_use]
    pub const fn pos(&self) -> Pos {
        match self {
            Self::Assign { pos, .. } | Self::Pun { pos, .. } | Self::Wildcard { pos, .. } => *pos,
        }
    }

    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Assign { span, .. } | Self::Pun { span, .. } | Self::Wildcard { span, .. } => {
                *span
            }
        }
    }

    #[must_use]
    pub const fn name(&self) -> Option<&Identifier> {
        match self {
            Self::Assign { name, .. } | Self::Pun { name, .. } => Some(name),
            Self::Wildcard { .. } => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alt {
    pub pat: Pat,
    pub body: Expr,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// Left-hand side: a variable with parameters, or a destructuring pattern.
    pub pat: Pat,
    /// Parameter patterns when the LHS is a function binding (`f x y = ...`).
    pub params: Vec<Pat>,
    pub expr: Expr,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Pat {
    Var {
        name: Identifier,
        pos: Pos,
        span: Span,
    },
    Wild {
        pos: Pos,
        span: Span,
    },
    Con {
        qualifier: Option<ModuleName>,
        name: Identifier,
        args: Vec<Self>,
        pos: Pos,
        span: Span,
    },
    Tuple {
        items: Vec<Self>,
        pos: Pos,
        span: Span,
    },
    List {
        items: Vec<Self>,
        pos: Pos,
        span: Span,
    },
    Lit {
        kind: LitKind,
        text: String,
        pos: Pos,
        span: Span,
    },
    /// `name@pat`
    As {
        name: Identifier,
        pat: Box<Self>,
        pos: Pos,
        span: Span,
    },
    /// Anything the parser couldn't classify; raw text preserved.
    Other {
        raw: String,
        pos: Pos,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Expr {
    /// Lowercase variable reference, possibly qualified.
    Var {
        qualifier: Option<ModuleName>,
        name: Identifier,
        pos: Pos,
        span: Span,
    },
    /// Constructor / data-constructor reference, possibly qualified.
    Con {
        qualifier: Option<ModuleName>,
        name: Identifier,
        pos: Pos,
        span: Span,
    },
    Lit {
        kind: LitKind,
        text: String,
        pos: Pos,
        span: Span,
    },
    /// Application, flattened: `f a b c` is one App with three args.
    App {
        func: Box<Self>,
        args: Vec<Self>,
        pos: Pos,
        span: Span,
    },
    /// Binary operator application with source-level operator text.
    BinOp {
        op: Operator,
        lhs: Box<Self>,
        rhs: Box<Self>,
        pos: Pos,
        span: Span,
    },
    /// Unary negation.
    Neg {
        expr: Box<Self>,
        pos: Pos,
        span: Span,
    },
    Lambda {
        params: Vec<Pat>,
        body: Box<Self>,
        pos: Pos,
        span: Span,
    },
    If {
        cond: Box<Self>,
        then_branch: Box<Self>,
        else_branch: Box<Self>,
        pos: Pos,
        span: Span,
    },
    Case {
        scrutinee: Box<Self>,
        alts: Vec<Alt>,
        pos: Pos,
        span: Span,
    },
    Do {
        stmts: Vec<DoStmt>,
        pos: Pos,
        span: Span,
    },
    LetIn {
        bindings: Vec<Binding>,
        body: Box<Self>,
        pos: Pos,
        span: Span,
    },
    /// `base with f = e, ...` — record construction when base is a Con,
    /// record update otherwise.
    Record {
        base: Box<Self>,
        fields: Vec<FieldAssign>,
        pos: Pos,
        span: Span,
    },
    Tuple {
        items: Vec<Self>,
        pos: Pos,
        span: Span,
    },
    List {
        items: Vec<Self>,
        pos: Pos,
        span: Span,
    },
    /// `try <body> catch <alts>`
    Try {
        body: Box<Self>,
        handlers: Vec<Alt>,
        pos: Pos,
        span: Span,
    },
    /// Parenthesized operator reference like `(+)`.
    OperatorRef { op: Operator, pos: Pos, span: Span },
    /// Left operator section like `(1 +)`.
    LeftSection {
        op: Operator,
        operand: Box<Self>,
        pos: Pos,
        span: Span,
    },
    /// Right operator section like `(+ 1)`.
    RightSection {
        op: Operator,
        operand: Box<Self>,
        pos: Pos,
        span: Span,
    },
    /// Expression the parser could not understand; raw text preserved so
    /// a parse failure degrades to the shim's behavior instead of dying.
    Error { raw: String, pos: Pos, span: Span },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DoStmt {
    /// `pat <- expr`
    Bind {
        pat: Pat,
        expr: Expr,
        pos: Pos,
        span: Span,
    },
    /// `let x = e` (no `in`) inside a do block.
    Let {
        bindings: Vec<Binding>,
        pos: Pos,
        span: Span,
    },
    /// Bare expression statement.
    Expr { expr: Expr, pos: Pos, span: Span },
}

/// Structured Daml type, parsed from the real token stream.
///
/// Scoped to the forms the corpus actually contains; it exists so consumers can
/// tell a type *application* from a *function arrow* from an
/// atomic constructor — a distinction a string matcher structurally cannot make.
/// Every node carries a byte span so consumers can render exact source text from
/// `(source, span)`.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Type {
    /// Type constructor, possibly qualified: `Party`, `DA.Map.Map`.
    Con {
        qualifier: Option<ModuleName>,
        name: Identifier,
        span: Span,
    },
    /// Type application, head applied to one or more args: `ContractId Foo`,
    /// `Map Text Int`, `Script ()`. Type-level nat literals (the `10` in
    /// `Numeric 10`) are NOT types, so they are dropped from the arg list — a
    /// `Numeric 10` collapses to the bare head `Con "Numeric"`.
    App(Box<Self>, Vec<Self>, Span),
    /// List type `[T]`.
    List(Box<Self>, Span),
    /// Tuple type `(a, b, ...)`.
    Tuple(Vec<Self>, Span),
    /// Function type `a -> b` (right-associative).
    Fun(Box<Self>, Box<Self>, Span),
    /// Lowercase type variable: `a`, `n`.
    Var(Identifier, Span),
    /// The unit type `()`.
    Unit(Span),
    /// A constrained type `C a => T`: the context is not modeled, the body `T`
    /// is kept.
    Constrained(Box<Self>, Span),
    /// Type-level string or char literal, e.g. the `"observers"` in
    /// `HasField "observers" t PartiesMap`.
    Lit {
        kind: LitKind,
        text: String,
        span: Span,
    },
}

/// Type equality intentionally ignores source spans.
///
/// Spans describe where equivalent type syntax appeared in a source file; they
/// are not part of structural type identity used by parser consumers.
impl PartialEq for Type {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::Con {
                    qualifier: aq,
                    name: an,
                    ..
                },
                Self::Con {
                    qualifier: bq,
                    name: bn,
                    ..
                },
            ) => aq == bq && an == bn,
            (Self::App(ah, aa, _), Self::App(bh, ba, _)) => ah == bh && aa == ba,
            (Self::List(a, _), Self::List(b, _))
            | (Self::Constrained(a, _), Self::Constrained(b, _)) => a == b,
            (Self::Tuple(a, _), Self::Tuple(b, _)) => a == b,
            (Self::Fun(al, ar, _), Self::Fun(bl, br, _)) => al == bl && ar == br,
            (Self::Var(a, _), Self::Var(b, _)) => a == b,
            (Self::Unit(_), Self::Unit(_)) => true,
            (
                Self::Lit {
                    kind: ak, text: at, ..
                },
                Self::Lit {
                    kind: bk, text: bt, ..
                },
            ) => ak == bk && at == bt,
            _ => false,
        }
    }
}

impl Eq for Type {}

impl Type {
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Con { span, .. }
            | Self::App(_, _, span)
            | Self::List(_, span)
            | Self::Tuple(_, span)
            | Self::Fun(_, _, span)
            | Self::Var(_, span)
            | Self::Unit(span)
            | Self::Constrained(_, span)
            | Self::Lit { span, .. } => *span,
        }
    }

    pub(crate) const fn with_span(mut self, span: Span) -> Self {
        match &mut self {
            Self::Con { span: s, .. }
            | Self::App(_, _, s)
            | Self::List(_, s)
            | Self::Tuple(_, s)
            | Self::Fun(_, _, s)
            | Self::Var(_, s)
            | Self::Unit(s)
            | Self::Constrained(_, s)
            | Self::Lit { span: s, .. } => *s = span,
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TypeAnnotation {
    /// The source construct did not include a type annotation.
    Absent,
    /// The source construct included a type annotation that parsed cleanly.
    Present(Type),
    /// The source construct included a type annotation, but the parser could
    /// not model it as a [`Type`]. Diagnostics carry the detailed message.
    Malformed { span: Span },
}

impl TypeAnnotation {
    #[must_use]
    pub const fn as_type(&self) -> Option<&Type> {
        match self {
            Self::Present(ty) => Some(ty),
            Self::Absent | Self::Malformed { .. } => None,
        }
    }

    #[must_use]
    pub const fn is_absent(&self) -> bool {
        matches!(self, Self::Absent)
    }

    #[must_use]
    pub const fn is_malformed(&self) -> bool {
        matches!(self, Self::Malformed { .. })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDecl {
    pub name: Identifier,
    /// Structured field type parse state.
    pub ty: TypeAnnotation,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Consuming {
    Consuming,
    NonConsuming,
    PreConsuming,
    PostConsuming,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceDecl {
    pub name: Identifier,
    pub consuming: Consuming,
    /// Structured return type. `None` if it could not be parsed cleanly or the
    /// choice declared no return type.
    pub return_ty: TypeAnnotation,
    pub params: Vec<FieldDecl>,
    /// Comma-separated controller expressions.
    pub controllers: Vec<Expr>,
    /// Choice observers, if any.
    pub observers: Vec<Expr>,
    pub body: Option<Expr>,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TemplateBodyDecl {
    Signatory {
        parties: Vec<Expr>,
        pos: Pos,
        span: Span,
    },
    Observer {
        parties: Vec<Expr>,
        pos: Pos,
        span: Span,
    },
    Ensure {
        expr: Expr,
        pos: Pos,
        span: Span,
    },
    Key {
        expr: Expr,
        /// Structured key type parse state.
        ty: TypeAnnotation,
        pos: Pos,
        span: Span,
    },
    Maintainer {
        expr: Expr,
        pos: Pos,
        span: Span,
    },
    Choice(ChoiceDecl),
    InterfaceInstance(InterfaceInstanceDecl),
    /// `agreement`, `let` blocks, deprecated `controller ... can`, etc.
    Other {
        raw: String,
        pos: Pos,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceInstanceDecl {
    /// Interface being implemented (`Disclosure.I`).
    pub interface_name: ModuleName,
    /// Explicit template from `for Foo`; `None` when omitted (the enclosing
    /// template when declared inside one).
    pub for_template: Option<ModuleName>,
    /// Method implementations: name → bound expression.
    pub methods: Vec<Binding>,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateDecl {
    pub name: Identifier,
    pub fields: Vec<FieldDecl>,
    pub body: Vec<TemplateBodyDecl>,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceDecl {
    pub name: Identifier,
    /// Interfaces this interface requires (`requires Lockable.I, ...`).
    pub requires: Vec<ModuleName>,
    pub viewtype: Option<ModuleName>,
    /// Method signatures: name and type text.
    pub methods: Vec<FieldDecl>,
    pub choices: Vec<ChoiceDecl>,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Equation {
    pub params: Vec<Pat>,
    pub body: Expr,
    /// Guarded equations keep their guards as (guard, body) pairs; `body`
    /// then holds the first guarded body for convenience.
    pub guards: Vec<(Expr, Expr)>,
    /// `where` helper bindings attached to this equation.
    pub where_bindings: Vec<Binding>,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionDecl {
    pub name: Identifier,
    pub ty: TypeAnnotation,
    pub equations: Vec<Equation>,
    pub pos: Pos,
    /// Span of the function's first appearance (signature or first equation).
    /// Convenience anchor; a multi-equation function's precise ranges are the
    /// per-`Equation` spans, since equations need not be contiguous in source.
    pub span: Span,
    /// Span of the standalone type signature `name : Type`, if one was seen.
    pub sig_span: Option<Span>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDecl {
    pub module_name: ModuleName,
    pub style: ImportStyle,
    pub alias: Option<ModuleName>,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Decl {
    Template(TemplateDecl),
    Interface(InterfaceDecl),
    Function(FunctionDecl),
    /// data/type/class/instance/exception — recorded with name + span.
    TypeDef {
        keyword: String,
        name: Identifier,
        pos: Pos,
        span: Span,
    },
    /// Anything unparseable at the top level (diagnostic already emitted).
    Unknown {
        raw: String,
        pos: Pos,
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub name: ModuleName,
    pub pos: Pos,
    /// Whole-module extent: `[0, source.len())`. Container for all decls.
    pub span: Span,
    /// Span of the `module M (...) where` header clause; empty when the file
    /// has no module header. Lets the span oracle treat header tokens as
    /// covered without a dedicated header node.
    pub header: Span,
    pub imports: Vec<ImportDecl>,
    pub decls: Vec<Decl>,
}

/// Why a [`ParseDiagnostic`] fired.
///
/// Lets a consumer separate syntax the parser deliberately does not model (still
/// safe, just unanalyzed) from a genuine malformation, a recursion-limit
/// degradation, or a lexical error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum DiagnosticCategory {
    /// A whole declaration could not be parsed and was skipped to the next item.
    SkippedDecl,
    /// A malformed expression, pattern, or expected-token error inside an
    /// otherwise-recognized construct.
    Malformed,
    /// A construct the parser intentionally does not support, e.g. legacy
    /// `controller ... can` choice syntax.
    UnsupportedSyntax,
    /// Expression/pattern nesting exceeded the recursion bound and was degraded
    /// to raw text.
    RecursionLimit,
    /// A lexical error (unterminated string/comment, stray character).
    Lex,
}

impl DiagnosticCategory {
    /// Stable kebab-case tag for machine-readable output (JSON/SARIF) and logs.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SkippedDecl => "skipped-declaration",
            Self::Malformed => "malformed",
            Self::UnsupportedSyntax => "unsupported-syntax",
            Self::RecursionLimit => "recursion-limit",
            Self::Lex => "lexical-error",
        }
    }
}

impl std::fmt::Display for DiagnosticCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Machine-readable reason a [`ParseDiagnostic`] fired.
///
/// Keep [`ParseDiagnostic::message`] for presentation. Match on this enum when
/// downstream code needs stable behavior for recoverable parser failures.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ParseDiagnosticKind {
    /// The lexer reported malformed source; the original lexical kind is
    /// preserved so callers do not need to parse the human message.
    Lex(crate::lexer::LexErrorKind),
    /// The parser expected a specific token or token class and recovered.
    ExpectedToken(ExpectedToken),
    /// A type annotation was present but could not be parsed as a `Type`.
    MalformedTypeAnnotation(TypeAnnotationContext),
    /// A recognized construct was malformed in a way that is not only an
    /// expected-token miss.
    MalformedSyntax(MalformedSyntaxKind),
    /// A whole declaration could not be parsed and was skipped.
    SkippedDeclaration(SkippedDeclarationReason),
    /// The source used syntax this parser intentionally does not model.
    UnsupportedSyntax(UnsupportedSyntaxKind),
    /// Expression or pattern nesting exceeded the parser recursion bound.
    RecursionLimit { limit: u32 },
}

impl ParseDiagnosticKind {
    /// Coarse diagnostic class retained for stable JSON/SARIF tags.
    #[must_use]
    pub const fn category(&self) -> DiagnosticCategory {
        match self {
            Self::Lex(_) => DiagnosticCategory::Lex,
            Self::ExpectedToken(_)
            | Self::MalformedTypeAnnotation(_)
            | Self::MalformedSyntax(_) => DiagnosticCategory::Malformed,
            Self::SkippedDeclaration(_) => DiagnosticCategory::SkippedDecl,
            Self::UnsupportedSyntax(_) => DiagnosticCategory::UnsupportedSyntax,
            Self::RecursionLimit { .. } => DiagnosticCategory::RecursionLimit,
        }
    }
}

/// Expected token or token class for a recoverable parser diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ExpectedToken {
    WhereAfterModuleHeader,
    ModuleNameAfterImport,
    TemplateNameAfterInterfaceInstanceFor,
    FieldNameTypePair,
    EqualsAfterGuard,
    EqualsOrGuardedRightHandSide,
    ProjectionFieldAfterDot,
    ThenKeyword,
    ElseKeyword,
    OfKeywordInCaseExpression,
    ArrowInGuardedCaseAlternative,
    ArrowInCaseAlternative,
}

/// The declaration context whose type annotation was malformed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TypeAnnotationContext {
    Field,
    Key,
    Choice,
    InterfaceMethod,
    Function,
}

impl TypeAnnotationContext {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Field => "field",
            Self::Key => "key",
            Self::Choice => "choice",
            Self::InterfaceMethod => "interface method",
            Self::Function => "function",
        }
    }
}

/// Recoverable malformed syntax cases that are not just missing one token.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MalformedSyntaxKind {
    FunctionEquation,
    FunctionParameterPattern,
    LambdaParameter,
}

/// Why a whole declaration was skipped.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SkippedDeclarationReason {
    TopLevelPatternBinding,
    UnrecognizedDeclaration,
}

/// Unsupported syntax families surfaced by the parser.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum UnsupportedSyntaxKind {
    LegacyControllerCan,
}

/// Parse diagnostic — never fatal under tolerant parsing.
///
/// Under [`crate::parse::parse_module`] the scan continues. Strict callers that
/// use [`crate::parse::parse_module_strict`] or
/// [`crate::parse::ParseModuleResult::into_result`] treat any diagnostic as
/// [`crate::parse::ParseModuleError`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDiagnostic {
    /// Machine-readable reason for the diagnostic.
    pub kind: ParseDiagnosticKind,
    /// Human-readable presentation message. Use [`Self::kind`] for logic.
    pub message: String,
    pub pos: Pos,
    /// Byte span of the offending region. The end is the actionable addition
    /// over `pos`-alone; zero-width when only a point is known (lex errors,
    /// EOF).
    pub span: Span,
    pub category: DiagnosticCategory,
}

impl ParseDiagnostic {
    #[must_use]
    pub fn new(
        kind: ParseDiagnosticKind,
        message: impl Into<String>,
        pos: Pos,
        span: Span,
    ) -> Self {
        let category = kind.category();
        Self {
            kind,
            message: message.into(),
            pos,
            span,
            category,
        }
    }

    /// Human-readable presentation message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }

    /// Machine-readable diagnostic reason.
    #[must_use]
    pub const fn kind(&self) -> &ParseDiagnosticKind {
        &self.kind
    }

    /// Coarse recovery category.
    #[must_use]
    pub const fn category(&self) -> DiagnosticCategory {
        self.category
    }
}

impl std::fmt::Display for ParseDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.message.fmt(f)
    }
}

impl std::error::Error for ParseDiagnostic {}

impl Expr {
    #[must_use]
    pub const fn pos(&self) -> Pos {
        match self {
            Self::Var { pos, .. }
            | Self::Con { pos, .. }
            | Self::Lit { pos, .. }
            | Self::App { pos, .. }
            | Self::BinOp { pos, .. }
            | Self::Neg { pos, .. }
            | Self::Lambda { pos, .. }
            | Self::If { pos, .. }
            | Self::Case { pos, .. }
            | Self::Do { pos, .. }
            | Self::LetIn { pos, .. }
            | Self::Record { pos, .. }
            | Self::Tuple { pos, .. }
            | Self::List { pos, .. }
            | Self::Try { pos, .. }
            | Self::OperatorRef { pos, .. }
            | Self::LeftSection { pos, .. }
            | Self::RightSection { pos, .. }
            | Self::Error { pos, .. } => *pos,
        }
    }

    /// Byte span covering the whole expression.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Var { span, .. }
            | Self::Con { span, .. }
            | Self::Lit { span, .. }
            | Self::App { span, .. }
            | Self::BinOp { span, .. }
            | Self::Neg { span, .. }
            | Self::Lambda { span, .. }
            | Self::If { span, .. }
            | Self::Case { span, .. }
            | Self::Do { span, .. }
            | Self::LetIn { span, .. }
            | Self::Record { span, .. }
            | Self::Tuple { span, .. }
            | Self::List { span, .. }
            | Self::Try { span, .. }
            | Self::OperatorRef { span, .. }
            | Self::LeftSection { span, .. }
            | Self::RightSection { span, .. }
            | Self::Error { span, .. } => *span,
        }
    }

    /// Render back to compact, source-*like* text for diagnostics and `raw`
    /// fields.
    ///
    /// This is **lossy and normalizing**, not byte-faithful: original layout is
    /// dropped (e.g. `do`/`let` statements are joined with `; `), operators and
    /// spacing are normalized, and comments/trivia are gone. Use it for a quick
    /// human-readable echo of an expression; for source-exact reconstruction use
    /// the node's [`span`](Self::span) into the original text (that is how
    /// `daml-fmt` and [`crate::ast_span::render_from_ast`] stay lossless).
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::Var {
                qualifier, name, ..
            }
            | Self::Con {
                qualifier, name, ..
            } => qualifier
                .as_ref()
                .map_or_else(|| name.to_string(), |q| format!("{q}.{name}")),
            Self::Lit { kind, text, .. } => match kind {
                LitKind::Text => format!("{text:?}"),
                LitKind::Char => format!("'{text}'"),
                _ => text.clone(),
            },
            Self::App { func, args, .. } => {
                let mut s = func.render_atomic();
                for a in args {
                    s.push(' ');
                    s.push_str(&a.render_atomic());
                }
                s
            }
            Self::BinOp { op, lhs, rhs, .. } => {
                if *op == "." {
                    // Record projection / composition: `account.custodian`.
                    format!("{}.{}", lhs.render_atomic(), rhs.render_atomic())
                } else {
                    format!("{} {} {}", lhs.render_atomic(), op, rhs.render_atomic())
                }
            }
            Self::Neg { expr, .. } => format!("-{}", expr.render_atomic()),
            Self::Lambda { params, body, .. } => {
                let ps: Vec<String> = params.iter().map(|p| p.render()).collect();
                format!("\\{} -> {}", ps.join(" "), body.render())
            }
            Self::If {
                cond,
                then_branch,
                else_branch,
                ..
            } => format!(
                "if {} then {} else {}",
                cond.render(),
                then_branch.render(),
                else_branch.render()
            ),
            Self::Case {
                scrutinee, alts, ..
            } => {
                let arms: Vec<String> = alts
                    .iter()
                    .map(|a| format!("{} -> {}", a.pat.render(), a.body.render()))
                    .collect();
                format!("case {} of {}", scrutinee.render(), arms.join("; "))
            }
            Self::Do { stmts, .. } => {
                let body: Vec<String> = stmts.iter().map(render_do_stmt).collect();
                format!("do {}", body.join("; "))
            }
            Self::LetIn { bindings, body, .. } => {
                let bs: Vec<String> = bindings.iter().map(render_binding).collect();
                format!("let {} in {}", bs.join("; "), body.render())
            }
            Self::Record { base, fields, .. } => {
                let fs: Vec<String> = fields
                    .iter()
                    .map(|f| match f {
                        FieldAssign::Assign { name, value, .. } => {
                            format!("{} = {}", name, value.render())
                        }
                        FieldAssign::Pun { name, .. } => name.to_string(),
                        FieldAssign::Wildcard { .. } => "..".to_string(),
                    })
                    .collect();
                format!("{} with {}", base.render_atomic(), fs.join("; "))
            }
            Self::Tuple { items, .. } => {
                let xs: Vec<String> = items.iter().map(|e| e.render()).collect();
                format!("({})", xs.join(", "))
            }
            Self::List { items, .. } => {
                let xs: Vec<String> = items.iter().map(|e| e.render()).collect();
                format!("[{}]", xs.join(", "))
            }
            Self::Try { body, handlers, .. } => {
                let hs: Vec<String> = handlers
                    .iter()
                    .map(|a| format!("{} -> {}", a.pat.render(), a.body.render()))
                    .collect();
                format!("try {} catch {}", body.render(), hs.join("; "))
            }
            Self::OperatorRef { op, .. } => format!("({op})"),
            Self::LeftSection { op, operand, .. } => format!("({} {})", operand.render(), op),
            Self::RightSection { op, operand, .. } => format!("({} {})", op, operand.render()),
            Self::Error { raw, .. } => raw.clone(),
        }
    }

    /// Render with parentheses if this expression wouldn't survive as an
    /// application argument.
    fn render_atomic(&self) -> String {
        match self {
            Self::Var { .. }
            | Self::Con { .. }
            | Self::Lit { .. }
            | Self::Tuple { .. }
            | Self::List { .. }
            | Self::OperatorRef { .. }
            | Self::LeftSection { .. }
            | Self::RightSection { .. }
            | Self::Error { .. } => self.render(),
            _ => format!("({})", self.render()),
        }
    }

    /// The head of an application spine: for `Foo.exercise cid X`, the
    /// `Foo.exercise` Var. For non-apps, the expression itself.
    #[must_use]
    pub fn application_head(&self) -> &Self {
        match self {
            Self::App { func, .. } => func.application_head(),
            _ => self,
        }
    }

    /// Application arguments, empty for non-apps. The `App` spine is flattened
    /// (see the [`App`](Self::App) variant), so for `f a b c` this returns all
    /// three arguments `[a, b, c]`, not a single curried layer.
    #[must_use]
    pub fn application_args(&self) -> &[Self] {
        match self {
            Self::App { args, .. } => args,
            _ => &[],
        }
    }
}

fn render_do_stmt(s: &DoStmt) -> String {
    match s {
        DoStmt::Bind { pat, expr, .. } => format!("{} <- {}", pat.render(), expr.render()),
        DoStmt::Let { bindings, .. } => {
            let bs: Vec<String> = bindings.iter().map(render_binding).collect();
            format!("let {}", bs.join("; "))
        }
        DoStmt::Expr { expr, .. } => expr.render(),
    }
}

fn render_binding(b: &Binding) -> String {
    let mut s = b.pat.render();
    for p in &b.params {
        s.push(' ');
        s.push_str(&p.render());
    }
    format!("{} = {}", s, b.expr.render())
}

impl Pat {
    #[must_use]
    pub const fn pos(&self) -> Pos {
        match self {
            Self::Var { pos, .. }
            | Self::Wild { pos, .. }
            | Self::Con { pos, .. }
            | Self::Tuple { pos, .. }
            | Self::List { pos, .. }
            | Self::Lit { pos, .. }
            | Self::As { pos, .. }
            | Self::Other { pos, .. } => *pos,
        }
    }

    /// Byte span covering the whole pattern.
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Var { span, .. }
            | Self::Wild { span, .. }
            | Self::Con { span, .. }
            | Self::Tuple { span, .. }
            | Self::List { span, .. }
            | Self::Lit { span, .. }
            | Self::As { span, .. }
            | Self::Other { span, .. } => *span,
        }
    }

    /// Render back to compact, source-*like* text. Lossy and normalizing in the
    /// same way as [`Expr::render`]; use the node's [`span`](Self::span) for
    /// byte-faithful text.
    #[must_use]
    pub fn render(&self) -> String {
        match self {
            Self::Var { name, .. } => name.to_string(),
            Self::Wild { .. } => "_".to_string(),
            Self::Con {
                qualifier,
                name,
                args,
                ..
            } => {
                let head = qualifier
                    .as_ref()
                    .map_or_else(|| name.to_string(), |q| format!("{q}.{name}"));
                if args.is_empty() {
                    head
                } else {
                    let parts: Vec<String> = args.iter().map(|p| p.render()).collect();
                    format!("({} {})", head, parts.join(" "))
                }
            }
            Self::Tuple { items, .. } => {
                let xs: Vec<String> = items.iter().map(|p| p.render()).collect();
                format!("({})", xs.join(", "))
            }
            Self::List { items, .. } => {
                let xs: Vec<String> = items.iter().map(|p| p.render()).collect();
                format!("[{}]", xs.join(", "))
            }
            Self::Lit { kind, text, .. } => match kind {
                LitKind::Text => format!("{text:?}"),
                LitKind::Char => format!("'{text}'"),
                _ => text.clone(),
            },
            Self::As { name, pat, .. } => format!("{}@{}", name, pat.render()),
            Self::Other { raw, .. } => raw.clone(),
        }
    }
}

impl std::fmt::Display for Expr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render())
    }
}

impl std::fmt::Display for Pat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.render())
    }
}

// Span invariants for the lossless AST tile layer; render-shape tests live in integration tests.
#[cfg(test)]
mod tests {
    use super::*;

    fn span(start: usize, end: usize) -> Span {
        Span::from_usize(start, end)
    }

    #[test]
    fn span_distinguishes_empty_from_invalid() {
        assert!(span(3, 3).is_valid());
        assert!(span(3, 3).is_empty());

        assert!(!span(4, 3).is_valid());
        assert!(!span(4, 3).is_empty());
    }

    #[test]
    fn contains_rejects_invalid_spans() {
        let parent = span(1, 10);

        assert!(parent.contains(&span(3, 7)));
        assert!(!parent.contains(&span(7, 3)));
        assert!(!span(10, 1).contains(&span(3, 7)));
    }

    #[test]
    fn span_range_and_get_share_source_bytes_safely() {
        let source = "foo: Int";

        assert_eq!(span(0, 3).range(), 0..3);
        assert_eq!(span(0, 3).get(source), Some("foo"));
        assert_eq!(span(3, 7).get(source), Some(": In"));
        assert!(span(3, 100).get(source).is_none());
    }
}
