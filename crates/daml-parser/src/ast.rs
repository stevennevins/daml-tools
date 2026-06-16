//! Typed, lossless parse tree produced by the recursive-descent parser
//! (src/parse.rs).
//!
//! Every node carries a source position and byte span. Downstream crates
//! consume this tree directly: daml-fmt re-prints layout from the spans, and
//! daml-lint lowers it onto its own rule-facing IR.

pub use crate::lexer::Pos;

/// Byte span of an AST node: `[start, end)` into the original source, same
/// basis as `Token::start`/`Token::end`. Covers every (non-virtual) token
/// that belongs to the node — first token's `start` to last token's `end`.
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
    pub fn new(start: usize, end: usize) -> Span {
        Span { start, end }
    }

    pub fn is_empty(&self) -> bool {
        self.start >= self.end
    }

    /// `self` fully contains `other`.
    pub fn contains(&self, other: &Span) -> bool {
        self.start <= other.start && other.end <= self.end
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum LitKind {
    Int,
    Decimal,
    Text,
    Char,
}

#[derive(Debug, Clone)]
pub struct FieldAssign {
    pub name: String,
    /// None for record puns (`Foo with owner` meaning `owner = owner`)
    /// and `..` wildcards.
    pub value: Option<Expr>,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Alt {
    pub pat: Pat,
    pub body: Expr,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Binding {
    /// Left-hand side: a variable with parameters, or a destructuring pattern.
    pub pat: Pat,
    /// Parameter patterns when the LHS is a function binding (`f x y = ...`).
    pub params: Vec<Pat>,
    pub expr: Expr,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Pat {
    Var {
        name: String,
        pos: Pos,
        span: Span,
    },
    Wild {
        pos: Pos,
        span: Span,
    },
    Con {
        qualifier: Option<String>,
        name: String,
        args: Vec<Pat>,
        pos: Pos,
        span: Span,
    },
    Tuple {
        items: Vec<Pat>,
        pos: Pos,
        span: Span,
    },
    List {
        items: Vec<Pat>,
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
        name: String,
        pat: Box<Pat>,
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

#[derive(Debug, Clone)]
pub enum Expr {
    /// Lowercase variable reference, possibly qualified.
    Var {
        qualifier: Option<String>,
        name: String,
        pos: Pos,
        span: Span,
    },
    /// Constructor / data-constructor reference, possibly qualified.
    Con {
        qualifier: Option<String>,
        name: String,
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
        func: Box<Expr>,
        args: Vec<Expr>,
        pos: Pos,
        span: Span,
    },
    /// Binary operator application with source-level operator text.
    BinOp {
        op: String,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        pos: Pos,
        span: Span,
    },
    /// Unary negation.
    Neg {
        expr: Box<Expr>,
        pos: Pos,
        span: Span,
    },
    Lambda {
        params: Vec<Pat>,
        body: Box<Expr>,
        pos: Pos,
        span: Span,
    },
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
        pos: Pos,
        span: Span,
    },
    Case {
        scrutinee: Box<Expr>,
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
        body: Box<Expr>,
        pos: Pos,
        span: Span,
    },
    /// `base with f = e, ...` — record construction when base is a Con,
    /// record update otherwise.
    Record {
        base: Box<Expr>,
        fields: Vec<FieldAssign>,
        pos: Pos,
        span: Span,
    },
    Tuple {
        items: Vec<Expr>,
        pos: Pos,
        span: Span,
    },
    List {
        items: Vec<Expr>,
        pos: Pos,
        span: Span,
    },
    /// `try <body> catch <alts>`
    Try {
        body: Box<Expr>,
        handlers: Vec<Alt>,
        pos: Pos,
        span: Span,
    },
    /// Right operator section like `(+ 1)` / left section `(1 +)`.
    Section {
        op: String,
        operand: Option<Box<Expr>>,
        left: bool,
        pos: Pos,
        span: Span,
    },
    /// Expression the parser could not understand; raw text preserved so
    /// a parse failure degrades to the shim's behavior instead of dying.
    Error { raw: String, pos: Pos, span: Span },
}

#[derive(Debug, Clone)]
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

/// Structured Daml type, parsed from the real token stream (not from
/// `type_text`). Scoped to the forms the corpus actually contains; it exists so
/// downstream analysis can tell a type *application* from a *function arrow*
/// from an atomic constructor — a distinction a string matcher structurally
/// cannot make. It carries no span and is never rendered, so it is invisible to
/// daml-fmt (which slices layout from `type_text` and the byte spans).
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    /// Type constructor, possibly qualified: `Party`, `DA.Map.Map`.
    Con {
        qualifier: Option<String>,
        name: String,
    },
    /// Type application, head applied to one or more args: `ContractId Foo`,
    /// `Map Text Int`, `Script ()`. Type-level nat literals (the `10` in
    /// `Numeric 10`) are NOT types, so they are dropped from the arg list — a
    /// `Numeric 10` collapses to the bare head `Con "Numeric"`.
    App(Box<Type>, Vec<Type>),
    /// List type `[T]`.
    List(Box<Type>),
    /// Tuple type `(a, b, ...)`.
    Tuple(Vec<Type>),
    /// Function type `a -> b` (right-associative).
    Fun(Box<Type>, Box<Type>),
    /// Lowercase type variable: `a`, `n`.
    Var(String),
    /// The unit type `()`.
    Unit,
    /// A constrained type `C a => T`: the context is dropped (no detector
    /// reasons about constraints), the body `T` is kept.
    Constrained(Box<Type>),
}

#[derive(Debug, Clone)]
pub struct FieldDecl {
    pub name: String,
    /// Type rendered back to source-ish text (`ContractId Foo`, `[Text]`).
    /// This is the slice daml-fmt and the lossless oracle rely on; it is NOT
    /// repurposed. The structured `ty` below is the analysis truth.
    pub type_text: String,
    /// Structured form of `type_text`, parsed from the token stream. `None`
    /// when the type could not be parsed cleanly (analysis treats it as
    /// unknown). Additive and span-free — see [`Type`].
    pub ty: Option<Type>,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Consuming {
    Consuming,
    NonConsuming,
    PreConsuming,
    PostConsuming,
}

#[derive(Debug, Clone)]
pub struct ChoiceDecl {
    pub name: String,
    pub consuming: Consuming,
    pub return_type_text: String,
    /// Structured form of `return_type_text` (see [`Type`]); `None` if it could
    /// not be parsed cleanly or the choice declared no return type.
    pub return_ty: Option<Type>,
    pub params: Vec<FieldDecl>,
    /// Comma-separated controller expressions.
    pub controllers: Vec<Expr>,
    /// Choice observers, if any.
    pub observers: Vec<Expr>,
    pub body: Option<Expr>,
    pub pos: Pos,
    pub span: Span,
    /// Line of the last token of the choice (body_raw slicing).
    pub end_line: usize,
}

#[derive(Debug, Clone)]
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
        type_text: String,
        /// Structured form of `type_text` (see [`Type`]); `None` if absent or
        /// not cleanly parseable.
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

#[derive(Debug, Clone)]
pub struct InterfaceInstanceDecl {
    /// Interface being implemented (`Disclosure.I`).
    pub interface_name: String,
    /// Template it is for (from `for Foo`); the enclosing template when
    /// declared inside one.
    pub for_template: String,
    /// Method implementations: name → bound expression.
    pub methods: Vec<Binding>,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TemplateDecl {
    pub name: String,
    pub fields: Vec<FieldDecl>,
    pub body: Vec<TemplateBodyDecl>,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct InterfaceDecl {
    pub name: String,
    /// Interfaces this interface requires (`requires Lockable.I, ...`).
    pub requires: Vec<String>,
    pub viewtype: Option<String>,
    /// Method signatures: name and type text.
    pub methods: Vec<FieldDecl>,
    pub choices: Vec<ChoiceDecl>,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct FunctionDecl {
    pub name: String,
    pub type_text: Option<String>,
    pub equations: Vec<Equation>,
    pub pos: Pos,
    /// Span of the function's first appearance (signature or first equation).
    /// Convenience anchor; a multi-equation function's precise ranges are the
    /// per-`Equation` spans, since equations need not be contiguous in source.
    pub span: Span,
    /// Span of the standalone type signature `name : Type`, if one was seen.
    pub sig_span: Option<Span>,
    /// Line of the last token of the last equation (body_raw slicing).
    pub end_line: usize,
}

#[derive(Debug, Clone)]
pub struct ImportDecl {
    pub module_name: String,
    pub qualified: bool,
    pub alias: Option<String>,
    pub pos: Pos,
    pub span: Span,
}

/// One alternative of a `data`/`newtype` declaration: a constructor with its
/// payload. Additive analysis truth alongside the decl's opaque `keyword`/`name`;
/// like [`Type`] it is span-bearing but never rendered, so it stays invisible to
/// daml-fmt (which lays out from byte spans).
///
/// The structured form models the common corpus shapes (record `with`/`{}`
/// constructors, positional constructors, enum alternatives). It is partial by
/// design: rather than record a half-truth, the parser leaves a whole decl's
/// `constructors` empty (opaque) when it meets a shape it does not model —
/// infix constructors (`data T = Int :+: Int`), strictness bangs
/// (`data T = T !Int`), GADT (`data T where`), and existential/context
/// constructors (`forall a. MkT a`). One additional accepted gap keeps the
/// *first* constructor only: a single-line record sum where each alternative is
/// a `with` block (`data T = A with x : Int | B with y : Int`); the multi-line
/// corpus form parses fully.
#[derive(Debug, Clone)]
pub struct DataConstructor {
    pub name: String,
    /// Record fields when the constructor uses record syntax
    /// (`Asset with amount : Decimal` or `Foo { x : Int }`). Empty for
    /// positional and nullary constructors.
    pub fields: Vec<FieldDecl>,
    /// Positional argument types for non-record constructors
    /// (`Node Int Text` → `[Int, Text]`, `Money Decimal` → `[Decimal]`). Empty
    /// for record and nullary constructors, or when the arguments did not parse
    /// cleanly (the constructor is then recorded name-only).
    pub arg_types: Vec<Type>,
    pub pos: Pos,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub enum Decl {
    Template(TemplateDecl),
    Interface(InterfaceDecl),
    Function(FunctionDecl),
    /// data/type/class/instance/exception — recorded with name + span.
    TypeDef {
        keyword: String,
        name: String,
        /// Constructors of a `data`/`newtype` declaration, structured. Empty for
        /// `type` synonyms, `class`/`instance`/`exception`, and any body that
        /// did not parse cleanly (the decl stays opaque, as before).
        constructors: Vec<DataConstructor>,
        /// Aliased type of a `type` synonym (`type Name = T`), structured.
        /// `None` for non-synonyms or an unparseable right-hand side.
        synonym: Option<Type>,
        /// Class names from a `deriving (...)` clause, flattened. Empty when
        /// there is no deriving clause.
        deriving: Vec<String>,
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

#[derive(Debug, Clone)]
pub struct Module {
    pub name: String,
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

/// Why a [`ParseDiagnostic`] fired. Lets a consumer separate syntax the parser
/// deliberately does not model (still safe, just unanalyzed) from a genuine
/// malformation, a recursion-limit degradation, or a lexical error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    pub fn as_str(self) -> &'static str {
        match self {
            DiagnosticCategory::SkippedDecl => "skipped-declaration",
            DiagnosticCategory::Malformed => "malformed",
            DiagnosticCategory::UnsupportedSyntax => "unsupported-syntax",
            DiagnosticCategory::RecursionLimit => "recursion-limit",
            DiagnosticCategory::Lex => "lexical-error",
        }
    }
}

/// Parse diagnostic — never fatal; the scan continues.
#[derive(Debug, Clone)]
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
    pub fn pos(&self) -> Pos {
        match self {
            Expr::Var { pos, .. }
            | Expr::Con { pos, .. }
            | Expr::Lit { pos, .. }
            | Expr::App { pos, .. }
            | Expr::BinOp { pos, .. }
            | Expr::Neg { pos, .. }
            | Expr::Lambda { pos, .. }
            | Expr::If { pos, .. }
            | Expr::Case { pos, .. }
            | Expr::Do { pos, .. }
            | Expr::LetIn { pos, .. }
            | Expr::Record { pos, .. }
            | Expr::Tuple { pos, .. }
            | Expr::List { pos, .. }
            | Expr::Try { pos, .. }
            | Expr::Section { pos, .. }
            | Expr::Error { pos, .. } => *pos,
        }
    }

    /// Byte span covering the whole expression.
    pub fn span(&self) -> Span {
        match self {
            Expr::Var { span, .. }
            | Expr::Con { span, .. }
            | Expr::Lit { span, .. }
            | Expr::App { span, .. }
            | Expr::BinOp { span, .. }
            | Expr::Neg { span, .. }
            | Expr::Lambda { span, .. }
            | Expr::If { span, .. }
            | Expr::Case { span, .. }
            | Expr::Do { span, .. }
            | Expr::LetIn { span, .. }
            | Expr::Record { span, .. }
            | Expr::Tuple { span, .. }
            | Expr::List { span, .. }
            | Expr::Try { span, .. }
            | Expr::Section { span, .. }
            | Expr::Error { span, .. } => *span,
        }
    }

    /// Render back to compact source-like text (raw-field compatibility).
    pub fn render(&self) -> String {
        match self {
            Expr::Var {
                qualifier, name, ..
            }
            | Expr::Con {
                qualifier, name, ..
            } => match qualifier {
                Some(q) => format!("{}.{}", q, name),
                None => name.clone(),
            },
            Expr::Lit { kind, text, .. } => match kind {
                LitKind::Text => format!("{:?}", text),
                LitKind::Char => format!("'{}'", text),
                _ => text.clone(),
            },
            Expr::App { func, args, .. } => {
                let mut s = func.render_atomic();
                for a in args {
                    s.push(' ');
                    s.push_str(&a.render_atomic());
                }
                s
            }
            Expr::BinOp { op, lhs, rhs, .. } => {
                if op == "." {
                    // Record projection / composition: `account.custodian`.
                    format!("{}.{}", lhs.render_atomic(), rhs.render_atomic())
                } else {
                    format!("{} {} {}", lhs.render_atomic(), op, rhs.render_atomic())
                }
            }
            Expr::Neg { expr, .. } => format!("-{}", expr.render_atomic()),
            Expr::Lambda { params, body, .. } => {
                let ps: Vec<String> = params.iter().map(|p| p.render()).collect();
                format!("\\{} -> {}", ps.join(" "), body.render())
            }
            Expr::If {
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
            Expr::Case {
                scrutinee, alts, ..
            } => {
                let arms: Vec<String> = alts
                    .iter()
                    .map(|a| format!("{} -> {}", a.pat.render(), a.body.render()))
                    .collect();
                format!("case {} of {}", scrutinee.render(), arms.join("; "))
            }
            Expr::Do { stmts, .. } => {
                let body: Vec<String> = stmts.iter().map(render_do_stmt).collect();
                format!("do {}", body.join("; "))
            }
            Expr::LetIn { bindings, body, .. } => {
                let bs: Vec<String> = bindings.iter().map(render_binding).collect();
                format!("let {} in {}", bs.join("; "), body.render())
            }
            Expr::Record { base, fields, .. } => {
                let fs: Vec<String> = fields
                    .iter()
                    .map(|f| match &f.value {
                        Some(v) => format!("{} = {}", f.name, v.render()),
                        None => f.name.clone(),
                    })
                    .collect();
                format!("{} with {}", base.render_atomic(), fs.join("; "))
            }
            Expr::Tuple { items, .. } => {
                let xs: Vec<String> = items.iter().map(|e| e.render()).collect();
                format!("({})", xs.join(", "))
            }
            Expr::List { items, .. } => {
                let xs: Vec<String> = items.iter().map(|e| e.render()).collect();
                format!("[{}]", xs.join(", "))
            }
            Expr::Try { body, handlers, .. } => {
                let hs: Vec<String> = handlers
                    .iter()
                    .map(|a| format!("{} -> {}", a.pat.render(), a.body.render()))
                    .collect();
                format!("try {} catch {}", body.render(), hs.join("; "))
            }
            Expr::Section {
                op, operand, left, ..
            } => match (operand, left) {
                (Some(e), true) => format!("({} {})", e.render(), op),
                (Some(e), false) => format!("({} {})", op, e.render()),
                (None, _) => format!("({})", op),
            },
            Expr::Error { raw, .. } => raw.clone(),
        }
    }

    /// Render with parentheses if this expression wouldn't survive as an
    /// application argument.
    fn render_atomic(&self) -> String {
        match self {
            Expr::Var { .. }
            | Expr::Con { .. }
            | Expr::Lit { .. }
            | Expr::Tuple { .. }
            | Expr::List { .. }
            | Expr::Section { .. }
            | Expr::Error { .. } => self.render(),
            _ => format!("({})", self.render()),
        }
    }

    /// The head of an application spine: for `Foo.exercise cid X`, the
    /// `Foo.exercise` Var. For non-apps, the expression itself.
    pub fn app_head(&self) -> &Expr {
        match self {
            Expr::App { func, .. } => func.app_head(),
            _ => self,
        }
    }

    /// Application arguments, empty for non-apps.
    pub fn app_args(&self) -> &[Expr] {
        match self {
            Expr::App { args, .. } => args,
            _ => &[],
        }
    }

    /// Is the head an unqualified variable with this exact name?
    pub fn head_is(&self, name: &str) -> bool {
        matches!(
            self.app_head(),
            Expr::Var { qualifier: None, name: n, .. } if n == name
        )
    }
}

pub fn render_do_stmt(s: &DoStmt) -> String {
    match s {
        DoStmt::Bind { pat, expr, .. } => format!("{} <- {}", pat.render(), expr.render()),
        DoStmt::Let { bindings, .. } => {
            let bs: Vec<String> = bindings.iter().map(render_binding).collect();
            format!("let {}", bs.join("; "))
        }
        DoStmt::Expr { expr, .. } => expr.render(),
    }
}

pub fn render_binding(b: &Binding) -> String {
    let mut s = b.pat.render();
    for p in &b.params {
        s.push(' ');
        s.push_str(&p.render());
    }
    format!("{} = {}", s, b.expr.render())
}

impl Pat {
    pub fn pos(&self) -> Pos {
        match self {
            Pat::Var { pos, .. }
            | Pat::Wild { pos, .. }
            | Pat::Con { pos, .. }
            | Pat::Tuple { pos, .. }
            | Pat::List { pos, .. }
            | Pat::Lit { pos, .. }
            | Pat::As { pos, .. }
            | Pat::Other { pos, .. } => *pos,
        }
    }

    /// Byte span covering the whole pattern.
    pub fn span(&self) -> Span {
        match self {
            Pat::Var { span, .. }
            | Pat::Wild { span, .. }
            | Pat::Con { span, .. }
            | Pat::Tuple { span, .. }
            | Pat::List { span, .. }
            | Pat::Lit { span, .. }
            | Pat::As { span, .. }
            | Pat::Other { span, .. } => *span,
        }
    }

    pub fn render(&self) -> String {
        match self {
            Pat::Var { name, .. } => name.clone(),
            Pat::Wild { .. } => "_".to_string(),
            Pat::Con {
                qualifier,
                name,
                args,
                ..
            } => {
                let head = match qualifier {
                    Some(q) => format!("{}.{}", q, name),
                    None => name.clone(),
                };
                if args.is_empty() {
                    head
                } else {
                    let parts: Vec<String> = args.iter().map(|p| p.render()).collect();
                    format!("({} {})", head, parts.join(" "))
                }
            }
            Pat::Tuple { items, .. } => {
                let xs: Vec<String> = items.iter().map(|p| p.render()).collect();
                format!("({})", xs.join(", "))
            }
            Pat::List { items, .. } => {
                let xs: Vec<String> = items.iter().map(|p| p.render()).collect();
                format!("[{}]", xs.join(", "))
            }
            Pat::Lit { kind, text, .. } => match kind {
                LitKind::Text => format!("{:?}", text),
                LitKind::Char => format!("'{}'", text),
                _ => text.clone(),
            },
            Pat::As { name, pat, .. } => format!("{}@{}", name, pat.render()),
            Pat::Other { raw, .. } => raw.clone(),
        }
    }
}
