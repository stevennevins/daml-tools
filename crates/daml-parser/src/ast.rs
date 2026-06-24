//! Typed, lossless parse tree produced by the recursive-descent parser
//! (src/parse.rs).
//!
//! Every node carries a source position and byte span. Downstream crates
//! consume this tree directly: daml-fmt re-prints layout from the spans, and
//! daml-lint lowers it onto its own rule-facing IR.

pub use crate::lexer::{Identifier, ModuleName, Operator, Pos};

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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl Span {
    #[must_use]
    pub const fn new(start: usize, end: usize) -> Self {
        Self { start, end }
    }

    /// True when the span is well-formed (`start <= end`).
    #[must_use]
    pub const fn is_valid(&self) -> bool {
        self.start <= self.end
    }

    /// True for a zero-width but still valid span.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start == self.end
    }

    #[must_use]
    pub const fn range(&self) -> std::ops::Range<usize> {
        self.start..self.end
    }

    #[must_use]
    pub fn get<'a>(&self, source: &'a str) -> Option<&'a str> {
        if self.start <= self.end
            && self.end <= source.len()
            && source.is_char_boundary(self.start)
            && source.is_char_boundary(self.end)
        {
            Some(&source[self.start..self.end])
        } else {
            None
        }
    }

    /// `self` fully contains `other`.
    #[must_use]
    pub const fn contains(&self, other: &Self) -> bool {
        self.is_valid() && other.is_valid() && self.start <= other.start && other.end <= self.end
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

/// Side of an operator section.
///
/// `(+ 1)` stores `SectionSide::Right`, while `(1 +)` stores
/// `SectionSide::Left`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum SectionSide {
    /// Right section: operator followed by right operand.
    Right,
    /// Left section: left operand followed by operator.
    Left,
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
pub struct FieldAssign {
    pub name: Identifier,
    /// None for record puns (`Foo with owner` meaning `owner = owner`)
    /// and `..` wildcards.
    pub value: Option<Expr>,
    pub pos: Pos,
    pub span: Span,
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
    /// Right operator section like `(+ 1)` / left section `(1 +)`.
    Section {
        op: Operator,
        operand: Option<Box<Self>>,
        side: SectionSide,
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
            | Self::Constrained(_, span) => *span,
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
            | Self::Constrained(_, s) => *s = span,
        }
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldDecl {
    pub name: Identifier,
    /// Structured field type parsed from the token stream. `None` when the type
    /// could not be parsed cleanly (analysis treats it as unknown).
    pub ty: Option<Type>,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub return_ty: Option<Type>,
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
        /// Structured key type. `None` if absent or not cleanly parseable.
        ty: Option<Type>,
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
    /// Template it is for (from `for Foo`); the enclosing template when
    /// declared inside one.
    pub for_template: ModuleName,
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
    pub ty: Option<Type>,
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

/// Parse diagnostic — never fatal; the scan continues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseDiagnostic {
    pub message: String,
    pub pos: Pos,
    /// Byte span of the offending region. The end is the actionable addition
    /// over `pos`-alone; zero-width when only a point is known (lex errors,
    /// EOF).
    pub span: Span,
    pub category: DiagnosticCategory,
}

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
            | Self::Section { pos, .. }
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
            | Self::Section { span, .. }
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
                    .map(|f| {
                        f.value.as_ref().map_or_else(
                            || f.name.to_string(),
                            |v| format!("{} = {}", f.name, v.render()),
                        )
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
            Self::Section {
                op, operand, side, ..
            } => match (operand, side) {
                (Some(e), SectionSide::Left) => format!("({} {})", e.render(), op),
                (Some(e), SectionSide::Right) => format!("({} {})", op, e.render()),
                (None, _) => format!("({op})"),
            },
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
            | Self::Section { .. }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pos() -> Pos {
        Pos { line: 1, column: 1 }
    }

    fn span(start: usize, end: usize) -> Span {
        Span::new(start, end)
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

    #[test]
    fn expr_render_keeps_normalized_application_and_projection_shape() {
        let projection = Expr::BinOp {
            op: ".".into(),
            lhs: Box::new(Expr::Var {
                qualifier: None,
                name: "this".into(),
                pos: pos(),
                span: span(0, 4),
            }),
            rhs: Box::new(Expr::Var {
                qualifier: None,
                name: "note".into(),
                pos: pos(),
                span: span(5, 9),
            }),
            pos: pos(),
            span: span(0, 9),
        };

        let expr = Expr::App {
            func: Box::new(Expr::Var {
                qualifier: None,
                name: "length".into(),
                pos: pos(),
                span: span(0, 6),
            }),
            args: vec![projection],
            pos: pos(),
            span: span(0, 16),
        };

        assert_eq!(expr.render(), "length (this.note)");
    }

    #[test]
    fn section_render_depends_on_section_side() {
        let expr_left = Expr::Section {
            op: "+".into(),
            operand: Some(Box::new(Expr::Var {
                qualifier: None,
                name: "x".into(),
                pos: pos(),
                span: span(0, 1),
            })),
            side: SectionSide::Left,
            pos: pos(),
            span: span(0, 4),
        };
        let expr_right = Expr::Section {
            op: "+".into(),
            operand: Some(Box::new(Expr::Lit {
                kind: LitKind::Int,
                text: "1".to_string(),
                pos: pos(),
                span: span(0, 1),
            })),
            side: SectionSide::Right,
            pos: pos(),
            span: span(0, 4),
        };

        assert_eq!(expr_left.render(), "(x +)");
        assert_eq!(expr_right.render(), "(+ 1)");
    }

    #[test]
    fn pat_render_preserves_collection_shape() {
        let pat = Pat::Tuple {
            items: vec![
                Pat::Var {
                    name: "owner".into(),
                    pos: pos(),
                    span: span(1, 6),
                },
                Pat::List {
                    items: Vec::new(),
                    pos: pos(),
                    span: span(8, 10),
                },
            ],
            pos: pos(),
            span: span(0, 11),
        };

        assert_eq!(pat.render(), "(owner, [])");
    }
}
