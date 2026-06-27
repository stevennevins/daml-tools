//! Typed, lossless parse tree produced by the recursive-descent parser
//! (src/parse.rs).
//!
//! Declarations, expressions, patterns, and most parser DTOs carry both a
//! 1-based source `Pos` for their first token and a byte `Span` for their
//! full source extent. `Type` nodes carry spans only because downstream
//! consumers slice type source text but do not currently need line/column
//! anchors per type fragment. Downstream crates consume this tree directly:
//! daml-fmt re-prints layout from the spans, and daml-lint lowers it onto its
//! own rule-facing IR.
//!
//! Parser-created trees are the supported construction path. Public fields are
//! exposed so tools can match the tree directly; vectors preserve source order,
//! `pos` is the first token's position, and `span` is the half-open byte range
//! covering the node's real source tokens.

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
    /// Integer literal.
    Int,
    /// Decimal literal.
    Decimal,
    /// Text/string literal.
    Text,
    /// Character literal.
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
        /// Field being assigned.
        name: Identifier,
        /// Right-hand-side expression after `=`.
        value: Expr,
        /// Position of the field name.
        pos: Pos,
        /// Span of the whole `field = expression` assignment.
        span: Span,
    },
    /// Record pun: `field`, meaning `field = field`.
    Pun {
        /// Punned field name.
        name: Identifier,
        /// Position of the field name.
        pos: Pos,
        /// Span of the field name.
        span: Span,
    },
    /// Record wildcard: `..`.
    Wildcard {
        /// Position of the `..` token.
        pos: Pos,
        /// Span of the `..` token.
        span: Span,
    },
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

/// Boolean or pattern guard qualifier in a guarded case alternative branch.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum GuardQualifier {
    /// Boolean guard expression.
    Bool {
        /// Guard expression.
        expr: Expr,
        /// Position of the guard's first token.
        pos: Pos,
        /// Span of the guard qualifier.
        span: Span,
    },
    /// Pattern guard `pat <- expr`.
    Pattern {
        /// Pattern bound by the guard.
        pat: Pat,
        /// Source expression on the right of `<-`.
        expr: Expr,
        /// Position of the pattern guard's first token.
        pos: Pos,
        /// Span of the pattern guard qualifier.
        span: Span,
    },
}

impl GuardQualifier {
    #[must_use]
    pub const fn span(&self) -> Span {
        match self {
            Self::Bool { span, .. } | Self::Pattern { span, .. } => *span,
        }
    }

    #[must_use]
    pub const fn pos(&self) -> Pos {
        match self {
            Self::Bool { pos, .. } | Self::Pattern { pos, .. } => *pos,
        }
    }
}

/// One guarded or unguarded branch of a case/`try` alternative.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AltBranch {
    /// Comma-separated guard qualifiers before `->`; empty for unguarded branches.
    pub guards: Vec<GuardQualifier>,
    /// Branch body after `->`.
    pub body: Expr,
    /// Position of the branch's first token (`|` or `->`).
    pub pos: Pos,
    /// Span of the whole branch.
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Alt {
    /// Pattern to match before `->` or the first `|`.
    pub pat: Pat,
    /// First branch body for convenience; mirrors `branches[0].body`.
    pub body: Expr,
    /// Source-ordered guarded/unguarded branches for this alternative.
    pub branches: Vec<AltBranch>,
    /// `where` helper bindings attached to this alternative.
    pub where_bindings: Vec<Binding>,
    /// Position of the alternative's first token.
    pub pos: Pos,
    /// Span of the whole alternative.
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    /// Left-hand side: a variable with parameters, or a destructuring pattern.
    pub pat: Pat,
    /// Parameter patterns when the LHS is a function binding (`f x y = ...`).
    pub params: Vec<Pat>,
    /// Right-hand-side expression.
    pub expr: Expr,
    /// Position of the binding's first token.
    pub pos: Pos,
    /// Span of the whole binding.
    pub span: Span,
}

/// Record-pattern field syntax: explicit braces or layout `with`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum RecordPatternSyntax {
    /// `Foo { field = pat; .. }`.
    Braces,
    /// `Foo with field; nested = pat`.
    With,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PatFieldAssign {
    /// Explicit record-pattern assignment: `field = pattern`.
    Assign {
        /// Field being matched.
        name: Identifier,
        /// Pattern bound to the field.
        pat: Pat,
        /// Position of the field name.
        pos: Pos,
        /// Span of the whole `field = pattern` assignment.
        span: Span,
    },
    /// Record-pattern pun: `field`, meaning `field = field`.
    Pun {
        /// Punned field name.
        name: Identifier,
        /// Position of the field name.
        pos: Pos,
        /// Span of the field name.
        span: Span,
    },
    /// Record-pattern wildcard: `..`.
    Wildcard {
        /// Position of the `..` token.
        pos: Pos,
        /// Span of the `..` token.
        span: Span,
    },
}

impl PatFieldAssign {
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
#[non_exhaustive]
pub enum Pat {
    /// Variable pattern.
    Var {
        /// Bound variable name.
        name: Identifier,
        /// Position of the variable token.
        pos: Pos,
        /// Span of the variable token.
        span: Span,
    },
    /// Wildcard pattern (`_`).
    Wild {
        /// Position of the `_` token.
        pos: Pos,
        /// Span of the `_` token.
        span: Span,
    },
    /// Constructor pattern with source-ordered arguments.
    Con {
        /// Optional module qualifier before the constructor name.
        qualifier: Option<ModuleName>,
        /// Constructor name.
        name: Identifier,
        /// Constructor arguments in source order.
        args: Vec<Self>,
        /// Position of the constructor token.
        pos: Pos,
        /// Span of the whole constructor pattern.
        span: Span,
    },
    /// Constructor record pattern with source-ordered fields.
    Record {
        /// Optional module qualifier before the constructor name.
        qualifier: Option<ModuleName>,
        /// Constructor name.
        name: Identifier,
        /// Whether fields used `{..}` or `with`.
        syntax: RecordPatternSyntax,
        /// Field patterns in source order.
        fields: Vec<PatFieldAssign>,
        /// Position of the constructor token.
        pos: Pos,
        /// Span of the whole record pattern.
        span: Span,
    },
    /// Tuple pattern with source-ordered items.
    Tuple {
        /// Tuple items in source order.
        items: Vec<Self>,
        /// Position of the opening parenthesis.
        pos: Pos,
        /// Span from `(` through `)`.
        span: Span,
    },
    /// List pattern with source-ordered items.
    List {
        /// List items in source order.
        items: Vec<Self>,
        /// Position of the opening bracket.
        pos: Pos,
        /// Span from `[` through `]`.
        span: Span,
    },
    /// Literal pattern; `text` is the parser's normalized literal text.
    Lit {
        /// Literal family.
        kind: LitKind,
        /// Normalized literal payload.
        text: String,
        /// Position of the literal token.
        pos: Pos,
        /// Span of the literal token in the source.
        span: Span,
    },
    /// `name@pat`
    As {
        /// Name bound to the whole matched pattern.
        name: Identifier,
        /// Pattern being aliased.
        pat: Box<Self>,
        /// Position of the bound name.
        pos: Pos,
        /// Span of the whole `name@pat` pattern.
        span: Span,
    },
    /// Anything the parser couldn't classify; raw text preserved.
    Other {
        /// Raw source text of the unclassified pattern.
        raw: String,
        /// Position of the raw pattern's first token.
        pos: Pos,
        /// Span of the preserved raw pattern text.
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Expr {
    /// Lowercase variable reference, possibly qualified.
    Var {
        /// Optional module qualifier before the variable name.
        qualifier: Option<ModuleName>,
        /// Variable name.
        name: Identifier,
        /// Position of the variable token.
        pos: Pos,
        /// Span of the variable token.
        span: Span,
    },
    /// Constructor / data-constructor reference, possibly qualified.
    Con {
        /// Optional module qualifier before the constructor name.
        qualifier: Option<ModuleName>,
        /// Constructor name.
        name: Identifier,
        /// Position of the constructor token.
        pos: Pos,
        /// Span of the constructor token.
        span: Span,
    },
    /// Literal expression; `text` is the parser's normalized literal text.
    Lit {
        /// Literal family.
        kind: LitKind,
        /// Normalized literal payload.
        text: String,
        /// Position of the literal token.
        pos: Pos,
        /// Span of the literal token in the source.
        span: Span,
    },
    /// Application, flattened: `f a b c` is one App with three args.
    App {
        /// Function expression being applied.
        func: Box<Self>,
        /// Arguments in source order.
        args: Vec<Self>,
        /// Position of the application's first token.
        pos: Pos,
        /// Span of the whole application.
        span: Span,
    },
    /// Binary operator application with source-level operator text.
    BinOp {
        /// Operator token text.
        op: Operator,
        /// Left operand.
        lhs: Box<Self>,
        /// Right operand.
        rhs: Box<Self>,
        /// Position of the left operand.
        pos: Pos,
        /// Span of the whole infix expression.
        span: Span,
    },
    /// Unary negation.
    Neg {
        /// Negated expression.
        expr: Box<Self>,
        /// Position of the `-` token.
        pos: Pos,
        /// Span of the whole negated expression.
        span: Span,
    },
    /// Lambda expression (`\params -> body`).
    Lambda {
        /// Parameter patterns in source order.
        params: Vec<Pat>,
        /// Lambda body.
        body: Box<Self>,
        /// Position of the lambda token.
        pos: Pos,
        /// Span of the whole lambda expression.
        span: Span,
    },
    /// Conditional expression.
    If {
        /// Condition after `if`.
        cond: Box<Self>,
        /// Expression after `then`.
        then_branch: Box<Self>,
        /// Expression after `else`.
        else_branch: Box<Self>,
        /// Position of the `if` token.
        pos: Pos,
        /// Span of the whole conditional expression.
        span: Span,
    },
    /// Case expression with source-ordered alternatives.
    Case {
        /// Scrutinee after `case`.
        scrutinee: Box<Self>,
        /// Alternatives in source order.
        alts: Vec<Alt>,
        /// Position of the `case` token.
        pos: Pos,
        /// Span of the whole case expression.
        span: Span,
    },
    /// Do block.
    Do {
        /// Statements in source order.
        stmts: Vec<DoStmt>,
        /// Position of the `do` token.
        pos: Pos,
        /// Span of the whole do block.
        span: Span,
    },
    /// Let/in expression.
    LetIn {
        /// Bindings in source order.
        bindings: Vec<Binding>,
        /// Body expression after `in`.
        body: Box<Self>,
        /// Position of the `let` token.
        pos: Pos,
        /// Span of the whole let/in expression.
        span: Span,
    },
    /// `base with f = e, ...` — record construction when base is a Con,
    /// record update otherwise.
    Record {
        /// Constructor or record value being constructed/updated.
        base: Box<Self>,
        /// Field assignments in source order.
        fields: Vec<FieldAssign>,
        /// Position of the base expression.
        pos: Pos,
        /// Span of the whole record expression.
        span: Span,
    },
    /// Tuple expression with source-ordered items.
    Tuple {
        /// Tuple items in source order.
        items: Vec<Self>,
        /// Position of the opening parenthesis.
        pos: Pos,
        /// Span from `(` through `)`.
        span: Span,
    },
    /// List expression with source-ordered items.
    List {
        /// List items in source order.
        items: Vec<Self>,
        /// Position of the opening bracket.
        pos: Pos,
        /// Span from `[` through `]`.
        span: Span,
    },
    /// `try <body> catch <alts>`
    Try {
        /// Body after `try`.
        body: Box<Self>,
        /// Catch handlers in source order.
        handlers: Vec<Alt>,
        /// Position of the `try` token.
        pos: Pos,
        /// Span of the whole try/catch expression.
        span: Span,
    },
    /// Parenthesized operator reference like `(+)`.
    OperatorRef {
        /// Referenced operator.
        op: Operator,
        /// Position of the opening parenthesis.
        pos: Pos,
        /// Span from `(` through `)`.
        span: Span,
    },
    /// Left operator section like `(1 +)`.
    LeftSection {
        /// Section operator.
        op: Operator,
        /// Left operand before the operator.
        operand: Box<Self>,
        /// Position of the opening parenthesis.
        pos: Pos,
        /// Span from `(` through `)`.
        span: Span,
    },
    /// Right operator section like `(+ 1)`.
    RightSection {
        /// Section operator.
        op: Operator,
        /// Right operand after the operator.
        operand: Box<Self>,
        /// Position of the opening parenthesis.
        pos: Pos,
        /// Span from `(` through `)`.
        span: Span,
    },
    /// Expression the parser could not understand; raw text preserved so
    /// a parse failure degrades to the shim's behavior instead of dying.
    Error {
        /// Raw source text preserved for the malformed expression.
        raw: String,
        /// Position of the malformed expression's first token.
        pos: Pos,
        /// Span of the preserved raw expression text.
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum DoStmt {
    /// `pat <- expr`
    Bind {
        /// Pattern before `<-`.
        pat: Pat,
        /// Expression after `<-`.
        expr: Expr,
        /// Position of the statement's first token.
        pos: Pos,
        /// Span of the whole bind statement.
        span: Span,
    },
    /// `let x = e` (no `in`) inside a do block.
    Let {
        /// Let bindings in source order.
        bindings: Vec<Binding>,
        /// Position of the `let` token.
        pos: Pos,
        /// Span of the whole let statement.
        span: Span,
    },
    /// Bare expression statement.
    Expr {
        /// Statement expression.
        expr: Expr,
        /// Position of the expression's first token.
        pos: Pos,
        /// Span of the expression statement.
        span: Span,
    },
}

/// Structured Daml type, parsed from the real token stream.
///
/// Scoped to the forms the corpus actually contains; it exists so consumers can
/// tell a type *application* from a *function arrow* from an
/// atomic constructor — a distinction a string matcher structurally cannot make.
/// Every node carries a byte span so consumers can render exact source text from
/// `(source, span)`. Unlike declarations, expressions, and patterns, type nodes
/// do not carry a separate [`Pos`]; use [`Type::span`] and source line mapping
/// when a line/column anchor is required for a type fragment.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Type {
    /// Type constructor, possibly qualified: `Party`, `DA.Map.Map`.
    Con {
        /// Optional module qualifier before the constructor name.
        qualifier: Option<ModuleName>,
        /// Constructor name.
        name: Identifier,
        /// Span of the constructor token in the source.
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
        /// Literal family.
        kind: LitKind,
        /// Normalized literal payload.
        text: String,
        /// Span of the literal token in the source.
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
    /// Field or method name.
    pub name: Identifier,
    /// Structured field type parse state.
    pub ty: TypeAnnotation,
    /// Position of the field/method name.
    pub pos: Pos,
    /// Span of the full field declaration (`name : Type` when present).
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Consuming {
    /// Daml `consuming` choice.
    Consuming,
    /// Daml `nonconsuming` choice.
    NonConsuming,
    /// Legacy/pre-Daml-3 `preconsuming` spelling.
    PreConsuming,
    /// Legacy/pre-Daml-3 `postconsuming` spelling.
    PostConsuming,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceDecl {
    /// Choice name.
    pub name: Identifier,
    /// Consuming mode parsed from the choice header.
    pub consuming: Consuming,
    /// Structured return type parse state.
    pub return_ty: TypeAnnotation,
    /// Choice parameter fields in source order.
    pub params: Vec<FieldDecl>,
    /// Comma-separated controller expressions.
    pub controllers: Vec<Expr>,
    /// Choice observers, if any.
    pub observers: Vec<Expr>,
    /// Choice authority expressions from `authority` metadata clauses.
    pub authority_exprs: Vec<Expr>,
    /// Choice body after `do`; `None` when the parser did not find one.
    pub body: Option<Expr>,
    /// Position of the `choice` token.
    pub pos: Pos,
    /// Span of the whole choice declaration.
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum TemplateBodyDecl {
    /// `signatory` clause with source-ordered party expressions.
    Signatory {
        /// Party expressions in source order.
        parties: Vec<Expr>,
        /// Position of the `signatory` token.
        pos: Pos,
        /// Span of the whole clause.
        span: Span,
    },
    /// `observer` clause with source-ordered party expressions.
    Observer {
        /// Party expressions in source order.
        parties: Vec<Expr>,
        /// Position of the `observer` token.
        pos: Pos,
        /// Span of the whole clause.
        span: Span,
    },
    /// `ensure` clause.
    Ensure {
        /// Predicate expression after `ensure`.
        expr: Expr,
        /// Position of the `ensure` token.
        pos: Pos,
        /// Span of the whole clause.
        span: Span,
    },
    /// Template `key` declaration.
    Key {
        /// Key expression.
        expr: Expr,
        /// Structured key type parse state.
        ty: TypeAnnotation,
        /// Position of the `key` token.
        pos: Pos,
        /// Span of the whole key declaration.
        span: Span,
    },
    /// `maintainer` clause.
    Maintainer {
        /// Maintainer expression.
        expr: Expr,
        /// Position of the `maintainer` token.
        pos: Pos,
        /// Span of the whole clause.
        span: Span,
    },
    /// `choice` declaration.
    Choice(ChoiceDecl),
    /// `interface instance` declaration nested in a template.
    InterfaceInstance(InterfaceInstanceDecl),
    /// `agreement`, `let` blocks, deprecated `controller ... can`, etc.
    Other {
        /// Raw source text preserved for unsupported/malformed body syntax.
        raw: String,
        /// Position of the raw body's first token.
        pos: Pos,
        /// Span of the preserved raw body text.
        span: Span,
    },
}

/// One item in an `interface instance ... where` body, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum InterfaceInstanceBodyItem {
    /// `view = <expr>` binding for the interface view implementation.
    View {
        /// View expression.
        expr: Expr,
        /// Position of the `view` token.
        pos: Pos,
        /// Span of the whole `view = ...` item.
        span: Span,
    },
    /// An ordinary interface method implementation.
    Method(Binding),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceInstanceDecl {
    /// Interface being implemented (`Disclosure.I`).
    pub interface_name: ModuleName,
    /// Explicit template from `for Foo`; `None` when omitted (the enclosing
    /// template when declared inside one).
    pub for_template: Option<ModuleName>,
    /// View and method implementations in source order.
    pub items: Vec<InterfaceInstanceBodyItem>,
    /// Position of the `interface instance` clause.
    pub pos: Pos,
    /// Span of the whole interface instance declaration.
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateDecl {
    /// Template name.
    pub name: Identifier,
    /// Template fields in source order.
    pub fields: Vec<FieldDecl>,
    /// Template body declarations in source order.
    pub body: Vec<TemplateBodyDecl>,
    /// Position of the `template` token.
    pub pos: Pos,
    /// Span of the whole template declaration.
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterfaceDecl {
    /// Interface name.
    pub name: Identifier,
    /// Interfaces this interface requires (`requires Lockable.I, ...`).
    pub requires: Vec<ModuleName>,
    /// Optional view type name from `viewtype`.
    pub viewtype: Option<ModuleName>,
    /// Method signatures in source order.
    pub methods: Vec<FieldDecl>,
    /// Interface choices in source order.
    pub choices: Vec<ChoiceDecl>,
    /// Position of the `interface` token.
    pub pos: Pos,
    /// Span of the whole interface declaration.
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Equation {
    /// Parameter patterns in source order.
    pub params: Vec<Pat>,
    /// Unguarded body or first guarded body for convenience.
    pub body: Expr,
    /// Guarded equations keep their guards as (guard, body) pairs; `body`
    /// then holds the first guarded body for convenience.
    pub guards: Vec<(Expr, Expr)>,
    /// `where` helper bindings attached to this equation.
    pub where_bindings: Vec<Binding>,
    /// Position of the equation's first token.
    pub pos: Pos,
    /// Span of this equation only.
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionDecl {
    /// Function name.
    pub name: Identifier,
    /// Standalone signature type parse state.
    pub ty: TypeAnnotation,
    /// Equations for this function in source order.
    pub equations: Vec<Equation>,
    /// Position of the function's first appearance.
    pub pos: Pos,
    /// Span of the function's first appearance (signature or first equation).
    /// Convenience anchor; a multi-equation function's precise ranges are the
    /// per-`Equation` spans, since equations need not be contiguous in source.
    pub span: Span,
    /// Span of the standalone type signature `name : Type`, if one was seen.
    pub sig_span: Option<Span>,
}

/// Fixity associativity keyword (`infix`, `infixl`, `infixr`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FixityAssoc {
    Infix,
    InfixL,
    InfixR,
}

/// Operator or backtick-quoted name in a fixity declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FixityTarget {
    /// Symbolic operator such as `===` or `>=>`.
    Operator(Operator),
    /// Backtick-quoted identifier such as `` `Pair` ``.
    Backtick(Identifier),
}

/// Top-level fixity declaration (`infix[l|r]? n op [, op ...]`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixityDecl {
    pub assoc: FixityAssoc,
    pub precedence: u8,
    pub operators: Vec<FixityTarget>,
    pub pos: Pos,
    pub span: Span,
}

/// Source package label on a package-qualified import (`import "pkg" Module`).
///
/// Holds the decoded string literal value and its source span. This is source
/// syntax only; it is not resolved to an LF `PackageId`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportPackageLabel {
    /// Decoded package label text from the string literal.
    pub value: String,
    /// Span of the string literal token, including quotes.
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportDecl {
    /// Imported module path.
    pub module_name: ModuleName,
    /// Whether the import is qualified.
    pub style: ImportStyle,
    /// Optional module alias from `as`.
    pub alias: Option<ModuleName>,
    /// Optional package label from `import "pkg" Module` source syntax.
    pub package_label: Option<ImportPackageLabel>,
    /// Position of the `import` token.
    pub pos: Pos,
    /// Span of the whole import declaration.
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Decl {
    /// Template declaration.
    Template(TemplateDecl),
    /// Interface declaration.
    Interface(InterfaceDecl),
    /// Function signature/equations grouped by name.
    Function(FunctionDecl),
    /// data/type/class/instance/exception — recorded with name + span.
    TypeDef {
        /// Declaration keyword (`data`, `type`, `class`, ...).
        keyword: String,
        /// Declared type/class/instance name as parsed.
        name: Identifier,
        /// Position of the declaration keyword.
        pos: Pos,
        /// Span of the declaration header/body consumed by the parser.
        span: Span,
    },
    /// Top-level fixity declaration.
    Fixity(FixityDecl),
    /// Top-level syntax the parser recognizes but intentionally does not model.
    UnsupportedSyntax {
        /// Why the declaration is unsupported.
        kind: UnsupportedSyntaxKind,
        /// Raw source text of the declaration.
        raw: String,
        /// Position of the declaration's first token.
        pos: Pos,
        /// Span of the declaration text.
        span: Span,
    },
    /// Anything unparseable at the top level (diagnostic already emitted).
    Unknown {
        /// Raw source text of the skipped declaration.
        raw: String,
        /// Position of the skipped declaration's first token.
        pos: Pos,
        /// Span of the skipped declaration text.
        span: Span,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    /// Module name from the header, or `Unknown` when the source has no header.
    pub name: ModuleName,
    /// Position of the `module` keyword, or the start of the fallback module.
    pub pos: Pos,
    /// Whole-module extent: `[0, source.len())`. Container for all decls.
    pub span: Span,
    /// Span of the `module M (...) where` header clause; empty when the file
    /// has no module header. Lets the span oracle treat header tokens as
    /// covered without a dedicated header node.
    pub header: Span,
    /// Imports in source order.
    pub imports: Vec<ImportDecl>,
    /// Top-level declarations in source order.
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
    PatternSynonym,
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
                let arms: Vec<String> = alts.iter().map(render_alt).collect();
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
                let hs: Vec<String> = handlers.iter().map(render_alt).collect();
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

fn render_guard_qualifier(guard: &GuardQualifier) -> String {
    match guard {
        GuardQualifier::Bool { expr, .. } => expr.render(),
        GuardQualifier::Pattern { pat, expr, .. } => {
            format!("{} <- {}", pat.render(), expr.render())
        }
    }
}

fn render_alt(alt: &Alt) -> String {
    let mut rendered = if alt.branches.len() == 1 && alt.branches[0].guards.is_empty() {
        format!("{} -> {}", alt.pat.render(), alt.branches[0].body.render())
    } else {
        let mut parts = vec![alt.pat.render()];
        for branch in &alt.branches {
            let guards: Vec<String> = branch.guards.iter().map(render_guard_qualifier).collect();
            parts.push(format!(
                "| {} -> {}",
                guards.join(", "),
                branch.body.render()
            ));
        }
        parts.join(" ")
    };
    if !alt.where_bindings.is_empty() {
        use std::fmt::Write;
        let bindings: Vec<String> = alt.where_bindings.iter().map(render_binding).collect();
        let _ = write!(rendered, " where {}", bindings.join("; "));
    }
    rendered
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
            | Self::Record { pos, .. }
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
            | Self::Record { span, .. }
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
            Self::Record {
                qualifier,
                name,
                syntax,
                fields,
                ..
            } => {
                let head = qualifier
                    .as_ref()
                    .map_or_else(|| name.to_string(), |q| format!("{q}.{name}"));
                let fs: Vec<String> = fields
                    .iter()
                    .map(|f| match f {
                        PatFieldAssign::Assign { name, pat, .. } => {
                            format!("{} = {}", name, pat.render())
                        }
                        PatFieldAssign::Pun { name, .. } => name.to_string(),
                        PatFieldAssign::Wildcard { .. } => "..".to_string(),
                    })
                    .collect();
                match syntax {
                    RecordPatternSyntax::Braces => format!("{} {{ {} }}", head, fs.join(", ")),
                    RecordPatternSyntax::With => format!("{} with {}", head, fs.join("; ")),
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
