use daml_parser::ast::{Span as ParserSpan, Type};
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Span {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct SourceSpan {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
    /// UTF-16 code-unit offset into `DamlModule.source`, suitable for
    /// JavaScript's `module.source.slice(start, end)`.
    pub start: usize,
    pub end: usize,
    /// Parser byte offsets into the UTF-8 source.
    pub byte_start: usize,
    pub byte_end: usize,
}

pub(crate) struct SourceMap<'a> {
    file: &'a Path,
    source: &'a str,
    line_starts: Vec<usize>,
    byte_to_utf16: Vec<usize>,
}

impl<'a> SourceMap<'a> {
    pub(crate) fn new(file: &'a Path, source: &'a str) -> Self {
        let mut line_starts = vec![0];
        for (idx, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_starts.push(idx + 1);
            }
        }
        let mut byte_to_utf16 = vec![0; source.len() + 1];
        let mut utf16 = 0usize;
        let mut prev = 0usize;
        for (idx, ch) in source.char_indices() {
            for slot in byte_to_utf16.iter_mut().take(idx).skip(prev) {
                *slot = utf16;
            }
            let char_end = idx + ch.len_utf8();
            for slot in byte_to_utf16.iter_mut().take(char_end).skip(idx) {
                *slot = utf16;
            }
            utf16 += ch.len_utf16();
            prev = char_end;
        }
        for slot in byte_to_utf16.iter_mut().take(source.len() + 1).skip(prev) {
            *slot = utf16;
        }
        Self {
            file,
            source,
            line_starts,
            byte_to_utf16,
        }
    }

    fn line_column_at_byte(&self, byte: usize) -> (usize, usize) {
        let byte = byte.min(self.source.len());
        let line_idx = match self.line_starts.binary_search(&byte) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };
        let line_start = self.line_starts[line_idx];
        (
            line_idx + 1,
            self.source[line_start..byte].chars().count() + 1,
        )
    }

    fn source_span(&self, span: ParserSpan) -> SourceSpan {
        let byte_start = span.start.min(self.source.len());
        let byte_end = span.end.min(self.source.len()).max(byte_start);
        let (line, column) = self.line_column_at_byte(byte_start);
        SourceSpan {
            file: self.file.to_path_buf(),
            line,
            column,
            start: self.byte_to_utf16[byte_start],
            end: self.byte_to_utf16[byte_end],
            byte_start,
            byte_end,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum TypeNode {
    Con {
        qualifier: Option<String>,
        name: String,
        span: SourceSpan,
    },
    App {
        head: Box<Self>,
        args: Vec<Self>,
        span: SourceSpan,
    },
    List {
        inner: Box<Self>,
        span: SourceSpan,
    },
    Tuple {
        items: Vec<Self>,
        span: SourceSpan,
    },
    Fun {
        param: Box<Self>,
        result: Box<Self>,
        span: SourceSpan,
    },
    Var {
        name: String,
        span: SourceSpan,
    },
    Unit {
        span: SourceSpan,
    },
    Constrained {
        body: Box<Self>,
        span: SourceSpan,
    },
}

impl TypeNode {
    pub(crate) fn from_type(t: &Type, source_map: &SourceMap<'_>) -> Self {
        let span = || source_map.source_span(t.span());
        match t {
            Type::Con {
                qualifier, name, ..
            } => Self::Con {
                qualifier: qualifier.clone(),
                name: name.clone(),
                span: span(),
            },
            Type::App(head, args, _) => Self::App {
                head: Box::new(Self::from_type(head, source_map)),
                args: args
                    .iter()
                    .map(|arg| Self::from_type(arg, source_map))
                    .collect(),
                span: span(),
            },
            Type::List(inner, _) => Self::List {
                inner: Box::new(Self::from_type(inner, source_map)),
                span: span(),
            },
            Type::Tuple(items, _) => Self::Tuple {
                items: items
                    .iter()
                    .map(|item| Self::from_type(item, source_map))
                    .collect(),
                span: span(),
            },
            Type::Fun(param, result, _) => Self::Fun {
                param: Box::new(Self::from_type(param, source_map)),
                result: Box::new(Self::from_type(result, source_map)),
                span: span(),
            },
            Type::Var(name, _) => Self::Var {
                name: name.clone(),
                span: span(),
            },
            Type::Unit(_) => Self::Unit { span: span() },
            Type::Constrained(body, _) => Self::Constrained {
                body: Box::new(Self::from_type(body, source_map)),
                span: span(),
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum DamlType {
    Party,
    Text,
    Decimal,
    Int,
    Bool,
    Date,
    Time,
    ContractId(Box<Self>),
    List(Box<Self>),
    Optional(Box<Self>),
    TextMap(Box<Self>),
    Map(Box<Self>, Box<Self>),
    Named(String),
    Unit,
    Unknown,
}

impl DamlType {
    /// Map a structured parser [`Type`] to the coarse rule-facing
    /// classification. Total. This is the single source of truth for
    /// `Field.type_` / `Choice.return_type`: it decides on real structure
    /// (application vs arrow vs atom), so unlike the old string reparse it can
    /// never confuse `Script ()` (an application) or `Int -> Int` (a function)
    /// for one opaque `Named`.
    pub fn from_type(t: &Type) -> Self {
        match t {
            Type::Con {
                qualifier, name, ..
            } => Self::con(qualifier.as_deref(), name),
            Type::App(head, args, _) => match head.as_ref() {
                Type::Con {
                    qualifier, name, ..
                } => Self::apply(qualifier.as_deref(), name, args),
                // Application with a non-constructor head (a type variable, a
                // function): nothing classifies these — opaque.
                _ => Self::Unknown,
            },
            Type::List(inner, _) => Self::List(Box::new(Self::from_type(inner))),
            Type::Unit(_) => Self::Unit,
            // A constraint context carries nothing a detector reasons about;
            // classify by the body.
            Type::Constrained(body, _) => Self::from_type(body),
            // Tuples, arrows and bare type variables carry no money/collection
            // meaning — the buckets the old matcher lumped into Unknown or a
            // misleading Named. Now they are *known* to be these shapes.
            Type::Tuple(_, _) | Type::Fun(_, _, _) | Type::Var(_, _) => Self::Unknown,
        }
    }

    /// Build the opaque `Named` for an unrecognized constructor, keeping its
    /// qualifier so a user type stays fully spelled (`Lib.Mod.Imported`, not
    /// `Imported`) — matching how the old string matcher carried the qualified
    /// text. No detector reads the name; it is rule-facing data only.
    fn named(qualifier: Option<&str>, name: &str) -> Self {
        Self::Named(qualifier.map_or_else(|| name.to_string(), |q| format!("{q}.{name}")))
    }

    /// A nullary constructor: the scalar builtins, the `Numeric`/`Decimal` money
    /// family, else an opaque `Named`. Classification keys on the bare
    /// constructor name and IGNORES the qualifier — deliberately matching all
    /// import spellings of a stdlib type (`Map`, `DA.Map.Map`, the common alias
    /// `Map.Map`). The old string matcher only recognized a fixed set of
    /// spellings and missed the aliased forms; keying on the tail is the
    /// intentional, more-complete behavior. Tradeoff: it is tail-WIDE, not
    /// alias-specific — a user type whose tail name collides with a stdlib
    /// collection (`MyMod.Map`) would also be classified as that collection.
    /// That is unidiomatic and absent from the corpus; the heuristic favors
    /// catching the real aliased collections.
    fn con(qualifier: Option<&str>, name: &str) -> Self {
        match name {
            "Party" => Self::Party,
            "Text" => Self::Text,
            "Decimal" => Self::Decimal,
            "Int" => Self::Int,
            "Bool" => Self::Bool,
            "Date" => Self::Date,
            "Time" => Self::Time,
            // `Decimal` is exactly `Numeric 10`; the whole fixed-point family is
            // the money type the monetary detectors care about. A bare `Numeric`
            // is `Numeric <nat>` with the nat literal dropped by the parser.
            "Numeric" => Self::Decimal,
            _ => Self::named(qualifier, name),
        }
    }

    /// An applied constructor `name arg...`, keyed on the tail name (qualifier
    /// ignored, see [`con`]). `List`/`Set` are NOT a list case here: a real list
    /// is `[T]` (`Type::List`); `Set X` is modelled as an unbounded collection
    /// so unbounded-fields flags it. Any other applied constructor (`Foo Bar`)
    /// is opaque `Named` carrying the head (application args are not part of the
    /// name — no detector reads it).
    fn apply(qualifier: Option<&str>, name: &str, args: &[Type]) -> Self {
        let first = || args.first().map(Self::from_type).unwrap_or(Self::Unknown);
        match name {
            "ContractId" => Self::ContractId(Box::new(first())),
            "Optional" => Self::Optional(Box::new(first())),
            "TextMap" => Self::TextMap(Box::new(first())),
            // The fixed-point money family: `Numeric n`.
            "Numeric" => Self::Decimal,
            // Unbounded keyed collections: key + value when both are present.
            "Map" | "GenMap" => {
                let k = first();
                let v = args.get(1).map(Self::from_type).unwrap_or(Self::Unknown);
                Self::Map(Box::new(k), Box::new(v))
            }
            // No dedicated Set variant; model as an unbounded collection (List).
            "Set" => Self::List(Box::new(first())),
            _ => Self::named(qualifier, name),
        }
    }

    pub const fn is_decimal(&self) -> bool {
        matches!(self, Self::Decimal)
    }

    pub const fn is_text(&self) -> bool {
        matches!(self, Self::Text)
    }

    pub const fn is_textmap(&self) -> bool {
        matches!(self, Self::TextMap(_))
    }

    pub const fn is_list(&self) -> bool {
        matches!(self, Self::List(_))
    }

    pub const fn is_map(&self) -> bool {
        matches!(self, Self::Map(_, _))
    }

    pub fn is_unbounded(&self) -> bool {
        match self {
            // An `Optional Text` / `Optional [a]` is still unbounded when present.
            Self::Optional(inner) => inner.is_unbounded(),
            _ => self.is_text() || self.is_textmap() || self.is_list() || self.is_map(),
        }
    }
}

/// Lightweight source position for expression-level nodes (1-based). The
/// enclosing module fixes the file; repeating the path on every node would
/// bloat the JSON handed to rule scripts.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
pub struct SrcPos {
    pub line: usize,
    pub column: usize,
}

/// Expression AST exposed to rule scripts. Serialized as tagged unions:
/// `{ "App": {...} }`, `{ "Lit": {...} }`, ... mirrored by daml-lint.d.ts.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum Expr {
    /// Variable reference: `amount`, `Map.lookup` (qualifier "Map").
    Var {
        name: String,
        qualifier: Option<String>,
        span: SrcPos,
    },
    /// Constructor or type reference in expression position: `Some`, `Iou`.
    Con {
        name: String,
        qualifier: Option<String>,
        span: SrcPos,
    },
    /// Literal; kind is "Int" | "Decimal" | "Text" | "Char".
    Lit {
        kind: String,
        value: String,
        span: SrcPos,
    },
    /// Application, flattened: `f a b` has two args.
    App {
        func: Box<Self>,
        args: Vec<Self>,
        span: SrcPos,
    },
    /// Binary operator with source-level operator text (`+`, `/`, `&&`,
    /// `` `div` `` for backtick application, `..` for ranges).
    BinOp {
        op: String,
        lhs: Box<Self>,
        rhs: Box<Self>,
        span: SrcPos,
    },
    Neg {
        expr: Box<Self>,
        span: SrcPos,
    },
    Lambda {
        params: Vec<String>,
        body: Box<Self>,
        span: SrcPos,
    },
    If {
        cond: Box<Self>,
        then_branch: Box<Self>,
        else_branch: Box<Self>,
        span: SrcPos,
    },
    Case {
        scrutinee: Box<Self>,
        alts: Vec<CaseAlt>,
        span: SrcPos,
    },
    /// Nested do block, lowered to statements like a choice body.
    DoBlock {
        statements: Vec<Statement>,
        span: SrcPos,
    },
    LetIn {
        bindings: Vec<LetBinding>,
        body: Box<Self>,
        span: SrcPos,
    },
    /// Record construction or update: `Foo with x = 1`, `this with owner`.
    Record {
        base: Box<Self>,
        fields: Vec<RecordField>,
        span: SrcPos,
    },
    Tuple {
        items: Vec<Self>,
        span: SrcPos,
    },
    List {
        items: Vec<Self>,
        span: SrcPos,
    },
    /// Anything without a structured encoding (operator sections,
    /// comprehension qualifiers, recovered parse errors). `raw` preserves
    /// the source text.
    Unknown {
        raw: String,
        span: SrcPos,
    },
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct CaseAlt {
    /// Pattern rendered to source text (`Some x`, `[]`, `_`).
    pub pattern: String,
    pub body: Expr,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct LetBinding {
    pub name: String,
    pub value: Expr,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct RecordField {
    pub name: String,
    /// None for punned fields (`Foo with owner`) and `..` spreads.
    pub value: Option<Expr>,
}

// ----- Expr analysis ------------------------------------------------------
//
// Detectors decide on the structured `Expr` tree, not on rendered text. Walking
// the tree makes a whole class of lexical bugs vanish: `not (...)`, `a || b`,
// operator direction, and `> 0` mentions hiding in comments / string literals
// all stop mattering, because the tree carries the real meaning.

impl Expr {
    /// Flatten a top-level `&&` conjunction into its leaf conditions. Only
    /// these are *guaranteed* to hold: anything under `||`, `not (...)`, or an
    /// `if` is returned as one opaque leaf, so it can never masquerade as a
    /// guarantee.
    pub(crate) fn conjuncts(&self) -> Vec<&Self> {
        fn go<'a>(e: &'a Expr, out: &mut Vec<&'a Expr>) {
            match e {
                Expr::BinOp { op, lhs, rhs, .. } if op == "&&" => {
                    go(lhs, out);
                    go(rhs, out);
                }
                _ => out.push(e),
            }
        }
        let mut out = Vec::new();
        go(self, &mut out);
        out
    }

    /// A plain value reference rendered to a dotted string: `amount`,
    /// `this.amount` (record projection is `BinOp "."`), `Map.lookup`. None for
    /// anything that is not a variable / constructor / projection chain.
    pub(crate) fn ref_string(&self) -> Option<String> {
        match self {
            Self::Var {
                name, qualifier, ..
            }
            | Self::Con {
                name, qualifier, ..
            } => Some(
                qualifier
                    .as_ref()
                    .map_or_else(|| name.clone(), |q| format!("{}.{}", q, name)),
            ),
            Self::BinOp { op, lhs, rhs, .. } if op == "." => {
                Some(format!("{}.{}", lhs.ref_string()?, rhs.ref_string()?))
            }
            _ => None,
        }
    }

    /// True if this expression refers to `name`: a bare reference, or a
    /// `this.<name>` / `self.<name>` projection (the implicit-record forms a
    /// choice body may use for a template field).
    pub(crate) fn refers_to(&self, name: &str) -> bool {
        self.ref_string()
            .is_some_and(|s| s == name || strip_implicit_self(&s) == strip_implicit_self(name))
    }

    /// True if this is a numeric literal equal to zero (`0`, `0.0`, `0.`).
    pub(crate) fn is_zero_lit(&self) -> bool {
        match self {
            Self::Lit { kind, value, .. } if kind == "Int" || kind == "Decimal" => {
                let t = value.trim();
                !t.is_empty() && t.bytes().all(|b| b == b'0' || b == b'.') && t.contains('0')
            }
            _ => false,
        }
    }

    /// True if this is a non-zero numeric literal (a safe divisor — `x / 2.0`,
    /// or a strictly-positive floor like `0.01`). Negative literals are spelled
    /// `Neg(Lit)`, so a bare `Lit` is always non-negative.
    pub(crate) fn is_nonzero_numeric_lit(&self) -> bool {
        matches!(self, Self::Lit { kind, .. } if kind == "Int" || kind == "Decimal")
            && !self.is_zero_lit()
    }

    /// True if this divides safely: a non-zero numeric literal, or the negation
    /// of one. A negated literal is `Neg(Lit)` (`x / (-2.0)`), which is just as
    /// safe a divisor as `x / 2.0` — but `-0.0` / `-0` is still zero and unsafe.
    ///
    /// Distinct from [`is_nonzero_numeric_lit`], which deliberately rejects
    /// negative literals so they cannot pose as a `>= 0` or `== positive` bound.
    pub(crate) fn is_nonzero_numeric_divisor(&self) -> bool {
        match self {
            Self::Neg { expr, .. } => expr.is_nonzero_numeric_divisor(),
            _ => self.is_nonzero_numeric_lit(),
        }
    }

    /// True if this is a non-negative numeric literal (`0`, `0.01`, `100.0`).
    /// Negative literals are spelled `Neg(Lit)` and do not match.
    pub(crate) fn is_nonneg_numeric_lit(&self) -> bool {
        matches!(self, Self::Lit { kind, .. } if kind == "Int" || kind == "Decimal")
    }

    /// Best-effort source-ish rendering, for evidence / messages only.
    pub(crate) fn render_text(&self) -> String {
        match self {
            Self::Var { .. } | Self::Con { .. } => self.ref_string().unwrap_or_default(),
            Self::Lit { value, .. } => value.clone(),
            Self::Neg { expr, .. } => format!("-{}", expr.render_text()),
            Self::BinOp { op, lhs, rhs, .. } if op == "." => {
                format!("{}.{}", lhs.render_text(), rhs.render_text())
            }
            Self::BinOp { op, lhs, rhs, .. } => {
                format!("{} {} {}", lhs.render_text(), op, rhs.render_text())
            }
            Self::App { func, args, .. } => {
                let mut s = func.render_text();
                for a in args {
                    s.push(' ');
                    s.push_str(&a.render_text());
                }
                s
            }
            Self::Tuple { items, .. } => format!("({})", render_join(items, ", ")),
            Self::List { items, .. } => format!("[{}]", render_join(items, ", ")),
            Self::Unknown { raw, .. } => raw.clone(),
            _ => "…".to_string(),
        }
    }

    /// The head of an application spine: `f a b` → `f`, `query @T x` → `query`.
    pub(crate) fn app_head(&self) -> &Self {
        match self {
            Self::App { func, .. } => func.app_head(),
            other => other,
        }
    }
}

fn render_join(items: &[Expr], sep: &str) -> String {
    items
        .iter()
        .map(|i| i.render_text())
        .collect::<Vec<_>>()
        .join(sep)
}

/// Strip the implicit-record prefix a choice body may put on a field
/// reference (`this.rate` / `self.rate` → `rate`), so a guard written bare
/// still matches a denominator written through `this`.
fn strip_implicit_self(s: &str) -> &str {
    s.strip_prefix("this.")
        .or_else(|| s.strip_prefix("self."))
        .unwrap_or(s)
}

/// A guaranteed conjunct that bounds `name` to be ≥ 0. The constant side may be
/// ANY non-negative literal: `amount > 100.0` and `amount >= 0.01` bound it
/// positive just as `amount > 0` does. A negative bound (`amount > -5.0`, which
/// the parser spells `Neg(Lit)`) does NOT count. An equality `amount == 5.0`
/// pins the field to a positive constant, so it counts too — but the literal
/// must be NON-ZERO (`== 0.0` still admits zero and stays flagged).
pub(crate) fn is_nonneg_bound(c: &Expr, name: &str) -> bool {
    match c {
        Expr::BinOp { op, lhs, rhs, .. } => match op.as_str() {
            ">" | ">=" => lhs.refers_to(name) && rhs.is_nonneg_numeric_lit(),
            "<" | "<=" => rhs.refers_to(name) && lhs.is_nonneg_numeric_lit(),
            "==" => {
                (lhs.refers_to(name) && rhs.is_nonzero_numeric_lit())
                    || (rhs.refers_to(name) && lhs.is_nonzero_numeric_lit())
            }
            _ => false,
        },
        _ => false,
    }
}

/// A guaranteed conjunct that bounds `name` strictly away from zero — the only
/// thing that makes a division safe. `>= 0` is rejected: zero still divides.
pub(crate) fn is_nonzero_bound(c: &Expr, name: &str) -> bool {
    match c {
        Expr::BinOp { op, lhs, rhs, .. } => match op.as_str() {
            ">" => lhs.refers_to(name) && rhs.is_zero_lit(),
            "<" => rhs.refers_to(name) && lhs.is_zero_lit(),
            "/=" | "!=" => {
                (lhs.refers_to(name) && rhs.is_zero_lit())
                    || (rhs.refers_to(name) && lhs.is_zero_lit())
            }
            _ => false,
        },
        _ => false,
    }
}

/// A guaranteed conjunct that bounds `name` STRICTLY positive: `name > 0`,
/// `0 < name`, or a positive floor `name >= 0.01`. `name >= 0` is rejected — it
/// admits zero (the zero-amount vulnerability).
pub(crate) fn is_strict_positive_bound(c: &Expr, name: &str) -> bool {
    match c {
        Expr::BinOp { op, lhs, rhs, .. } => match op.as_str() {
            ">" => lhs.refers_to(name) && rhs.is_nonneg_numeric_lit(),
            ">=" => lhs.refers_to(name) && rhs.is_nonzero_numeric_lit(),
            "<" => rhs.refers_to(name) && lhs.is_nonneg_numeric_lit(),
            "<=" => rhs.refers_to(name) && lhs.is_nonzero_numeric_lit(),
            _ => false,
        },
        _ => false,
    }
}

/// True if a guaranteed conjunct of `cond` bounds `name` strictly positive.
pub(crate) fn expr_guarantees_strict_positive(cond: &Expr, name: &str) -> bool {
    cond.conjuncts()
        .iter()
        .any(|c| is_strict_positive_bound(c, name))
}

/// A guaranteed conjunct that bounds `length name` / `size name` from ABOVE
/// (`length f < N`, `N >= size f`). A mere lower bound (`length f > 0`) does
/// NOT bound the size and is intentionally rejected.
///
/// The bounding operand must be a real CONSTANT: a non-negative numeric literal
/// or a free identifier (a module-level constant). It must NOT be a sibling
/// template field — the contract creator sets the entire payload, so a field
/// bound like `length tags < cap` is attacker-controlled and bounds nothing
/// (`fields` is the template's field-name set). A relational bound against
/// another collection's length (`length a == length b`) is rejected too, since
/// it forces only equal length, not a constant ceiling.
fn is_size_upper_bound(c: &Expr, name: &str, fields: &[String]) -> bool {
    match c {
        Expr::BinOp { op, lhs, rhs, .. } => match op.as_str() {
            "<" | "<=" => is_size_app(lhs, name) && is_const_size_bound(rhs, fields),
            ">" | ">=" => is_size_app(rhs, name) && is_const_size_bound(lhs, fields),
            // An exact-size constraint `length f == N` bounds the size too.
            "==" => {
                (is_size_app(lhs, name) && is_const_size_bound(rhs, fields))
                    || (is_size_app(rhs, name) && is_const_size_bound(lhs, fields))
            }
            _ => false,
        },
        _ => false,
    }
}

/// True if `e` is a real constant ceiling for a size bound: a non-negative
/// numeric literal, or a plain identifier / projection that is NOT a sibling
/// template field (i.e. a module-level constant). A sibling field is
/// attacker-controlled, and a non-reference expression — notably another
/// `length`/`size` application — is no ceiling at all.
fn is_const_size_bound(e: &Expr, fields: &[String]) -> bool {
    if e.is_nonneg_numeric_lit() {
        return true;
    }
    match e.ref_string() {
        Some(_) => !fields.iter().any(|f| e.refers_to(f)),
        None => false,
    }
}

/// True if `func args` is a `length`/`size` call (at any qualifier) of a single
/// argument referring to `name`.
fn is_size_call(func: &Expr, args: &[Expr], name: &str) -> bool {
    matches!(func, Expr::Var { name: f, .. } if f == "length" || f == "size")
        && args.len() == 1
        && args[0].refers_to(name)
}

/// `length f` / `size f`, at any qualifier (`T.length f`, `Map.size f`).
fn is_size_app(e: &Expr, name: &str) -> bool {
    match e {
        Expr::App { func, args, .. } => is_size_call(func, args, name),
        // `length transfer.note` is `length (transfer.note)`, but `.` (an
        // operator looser than application) makes the parser read it as
        // `(length transfer).note`. Reconstruct the intended `length base.field`
        // and match it against `name`, whether `name` is dotted
        // (`transfer.note`) or, when the base is the template instance
        // (`this`/`self`), the bare field (`note`).
        Expr::BinOp { op, lhs, rhs, .. } if op == "." => match lhs.as_ref() {
            Expr::App { func, args, .. }
                if matches!(func.as_ref(), Expr::Var { name: f, .. } if f == "length" || f == "size")
                    && args.len() == 1 =>
            {
                let base = args[0].ref_string();
                let field = rhs.ref_string();
                match (base, field) {
                    (Some(base), Some(field)) => {
                        format!("{base}.{field}") == name
                            || (matches!(base.as_str(), "this" | "self") && rhs.refers_to(name))
                    }
                    _ => false,
                }
            }
            _ => false,
        },
        _ => false,
    }
}

/// True if a guaranteed conjunct of `cond` bounds `name` strictly away from
/// zero — used to recognize an `assert`/`ensure` that guards a division.
pub(crate) fn expr_guarantees_nonzero(cond: &Expr, name: &str) -> bool {
    cond.conjuncts().iter().any(|c| is_nonzero_bound(c, name))
}

/// True if `e` is `null <name>` / `Foldable.null <name>` — an emptiness test on
/// the list `name`.
fn is_null_app(e: &Expr, name: &str) -> bool {
    matches!(e, Expr::App { func, args, .. }
        if matches!(func.as_ref(), Expr::Var { name: f, .. } if f == "null")
            && args.len() == 1
            && args[0].refers_to(name))
        || matches!(e, Expr::BinOp { op, lhs, rhs, .. } if op == "$"
            && matches!(lhs.as_ref(), Expr::Var { name: f, .. } if f == "null")
            && rhs.refers_to(name))
}

/// A guaranteed conjunct that establishes the list `name` is NON-EMPTY: a strict
/// lower bound on its `length`/`size` (`length p > 0`, `0 < length p`,
/// `length p >= 1`, `1 <= length p`, `length p /= 0`), or a `not (null p)` /
/// `not $ null p` assertion. An UPPER bound (`length p < N`, `length p <= N`)
/// does NOT count — the empty list still satisfies it — and neither does a bound
/// against a non-zero/non-`1` operand spelled as a free identifier (`length p <
/// maxNumInputs`), which is an attacker-controllable ceiling, not a floor.
fn is_nonempty_bound(c: &Expr, name: &str) -> bool {
    match c {
        // `length p > 0`, `length p >= 1`, `0 < length p`, `1 <= length p`.
        Expr::BinOp { op, lhs, rhs, .. } => match op.as_str() {
            ">" => is_size_app(lhs, name) && rhs.is_zero_lit(),
            ">=" => is_size_app(lhs, name) && rhs.is_nonzero_numeric_lit(),
            "<" => is_size_app(rhs, name) && lhs.is_zero_lit(),
            "<=" => is_size_app(rhs, name) && lhs.is_nonzero_numeric_lit(),
            // `length p /= 0` / `0 /= length p`: a count that is never zero.
            "/=" | "!=" => {
                (is_size_app(lhs, name) && rhs.is_zero_lit())
                    || (is_size_app(rhs, name) && lhs.is_zero_lit())
            }
            // `not $ null p`: the `$` splits `not` from its argument `null p`.
            "$" => {
                matches!(lhs.as_ref(), Expr::Var { name: f, .. } if f == "not")
                    && is_null_app(rhs, name)
            }
            _ => false,
        },
        // `not (null p)`.
        Expr::App { func, args, .. } => {
            matches!(func.as_ref(), Expr::Var { name: f, .. } if f == "not")
                && args.len() == 1
                && is_null_app(&args[0], name)
        }
        _ => false,
    }
}

/// True if a guaranteed conjunct of `cond` proves the list `name` is non-empty.
pub(crate) fn expr_guarantees_nonempty(cond: &Expr, name: &str) -> bool {
    cond.conjuncts().iter().any(|c| is_nonempty_bound(c, name))
}

/// True if a guaranteed conjunct of `cond` bounds `length name` / `size name`
/// only from ABOVE — `length p < N`, `length p <= N`, `N > length p`, or a count
/// compared against any non-zero ceiling (a literal or a free identifier such as
/// `maxNumInputs`). This is the "max-count but no min-count" anti-pattern: the
/// empty list passes such a check, so an upper bound on its own leaves the
/// zero-input vulnerability open.
pub(crate) fn expr_has_size_upper_bound(cond: &Expr, name: &str) -> bool {
    cond.conjuncts().iter().any(|c| match c {
        Expr::BinOp { op, lhs, rhs, .. } => match op.as_str() {
            // `length p < N` / `length p <= N` — but `length p < 1` / `<= 0`
            // actually forces emptiness, not a ceiling on a non-empty list; treat
            // a literal `1`/`0` floor-ish operand conservatively as not an
            // upper-bound-only guard so we never flag it as max-only.
            "<" | "<=" => is_size_app(lhs, name) && is_ceiling_operand(rhs),
            ">" | ">=" => is_size_app(rhs, name) && is_ceiling_operand(lhs),
            _ => false,
        },
        _ => false,
    })
}

/// True if `e` is a real ceiling operand for an upper-bound count check: a
/// non-negative numeric literal, or a free identifier / projection (a
/// module-level constant or a field like `maxNumInputs`). A `length`/`size`
/// application is not a constant ceiling.
fn is_ceiling_operand(e: &Expr) -> bool {
    e.is_nonneg_numeric_lit() || e.ref_string().is_some()
}

/// The expressions held directly by a statement (NOT recursing into nested
/// statements; do-blocks are reached through the walkers below).
pub(crate) fn statement_exprs(s: &Statement) -> Vec<&Expr> {
    match s {
        Statement::Let { value, .. } => vec![value],
        Statement::Assert { condition_expr, .. } => vec![condition_expr],
        Statement::Fetch { cid, .. } => vec![cid],
        Statement::Archive { cid, .. } => vec![cid],
        Statement::Create { argument, .. } => vec![argument],
        Statement::Exercise { cid, argument, .. } => argument
            .as_ref()
            .map_or_else(|| vec![cid], |a| vec![cid, a]),
        Statement::Other { expr, .. } => vec![expr],
        // A Branch's only direct expression is the case scrutinee; the arm
        // bodies (and the expressions therein) are reached through the body
        // walkers. TryCatch carries no direct expression at all.
        Statement::Branch { scrutinee, .. } => scrutinee.iter().collect(),
        Statement::TryCatch { .. } => vec![],
    }
}

/// Immediate sub-expressions of `e`, one level deep. A `DoBlock` returns none
/// here — the walkers descend into its statements explicitly.
pub(crate) fn child_exprs(e: &Expr) -> Vec<&Expr> {
    match e {
        Expr::App { func, args, .. } => {
            let mut v = vec![func.as_ref()];
            v.extend(args.iter());
            v
        }
        Expr::BinOp { lhs, rhs, .. } => vec![lhs, rhs],
        Expr::Neg { expr, .. } => vec![expr],
        Expr::Lambda { body, .. } => vec![body],
        Expr::If {
            cond,
            then_branch,
            else_branch,
            ..
        } => vec![cond, then_branch, else_branch],
        Expr::Case {
            scrutinee, alts, ..
        } => {
            let mut v = vec![scrutinee.as_ref()];
            v.extend(alts.iter().map(|a| &a.body));
            v
        }
        Expr::LetIn { bindings, body, .. } => {
            let mut v: Vec<&Expr> = bindings.iter().map(|b| &b.value).collect();
            v.push(body);
            v
        }
        Expr::Record { base, fields, .. } => {
            let mut v = vec![base.as_ref()];
            v.extend(fields.iter().filter_map(|f| f.value.as_ref()));
            v
        }
        Expr::Tuple { items, .. } | Expr::List { items, .. } => items.iter().collect(),
        Expr::Var { .. }
        | Expr::Con { .. }
        | Expr::Lit { .. }
        | Expr::DoBlock { .. }
        | Expr::Unknown { .. } => vec![],
    }
}

/// Visit every expression in a body — the direct statement expressions and,
/// recursively, everything nested inside them (do-blocks, try/catch, case
/// alternatives, lambdas, …).
pub(crate) fn walk_body_exprs<'a>(stmts: &'a [Statement], f: &mut impl FnMut(&'a Expr)) {
    for s in stmts {
        for e in statement_exprs(s) {
            walk_expr(e, f);
        }
        match s {
            Statement::TryCatch {
                try_body,
                catch_body,
                ..
            } => {
                walk_body_exprs(try_body, f);
                walk_body_exprs(catch_body, f);
            }
            Statement::Branch { arms, .. } => {
                for arm in arms {
                    walk_body_exprs(&arm.body, f);
                }
            }
            _ => {}
        }
    }
}

/// Visit `e` and, recursively, every sub-expression within it.
pub(crate) fn for_each_subexpr<'a>(e: &'a Expr, f: &mut impl FnMut(&'a Expr)) {
    walk_expr(e, f);
}

fn walk_expr<'a>(e: &'a Expr, f: &mut impl FnMut(&'a Expr)) {
    f(e);
    if let Expr::DoBlock { statements, .. } = e {
        walk_body_exprs(statements, f);
    }
    for c in child_exprs(e) {
        walk_expr(c, f);
    }
}

/// Visit every statement that UNCONDITIONALLY runs in `stmts`: the top-level
/// statements and, recursively, the bodies of a `TryCatch`. A `Branch` arm is
/// deliberately NOT entered — exactly one arm runs at runtime, so a statement
/// inside an arm does not unconditionally dominate its siblings. Used by guard
/// checks where a conditional `assert` must not count as a guarantee.
pub(crate) fn walk_unconditional_stmts<'a>(
    stmts: &'a [Statement],
    f: &mut impl FnMut(&'a Statement),
) {
    for s in stmts {
        f(s);
        if let Statement::TryCatch {
            try_body,
            catch_body,
            ..
        } = s
        {
            walk_unconditional_stmts(try_body, f);
            walk_unconditional_stmts(catch_body, f);
        }
    }
}

/// Visit every statement in a body, descending through try/catch and through
/// do-blocks nested inside statement expressions.
pub(crate) fn walk_body_stmts<'a>(stmts: &'a [Statement], g: &mut impl FnMut(&'a Statement)) {
    for s in stmts {
        g(s);
        match s {
            Statement::TryCatch {
                try_body,
                catch_body,
                ..
            } => {
                walk_body_stmts(try_body, g);
                walk_body_stmts(catch_body, g);
            }
            Statement::Branch { arms, .. } => {
                for arm in arms {
                    walk_body_stmts(&arm.body, g);
                }
            }
            _ => {}
        }
        for e in statement_exprs(s) {
            walk_nested_do_stmts(e, g);
        }
    }
}

fn walk_nested_do_stmts<'a>(e: &'a Expr, g: &mut impl FnMut(&'a Statement)) {
    if let Expr::DoBlock { statements, .. } = e {
        walk_body_stmts(statements, g);
    }
    for c in child_exprs(e) {
        walk_nested_do_stmts(c, g);
    }
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Field {
    pub name: String,
    pub type_: Option<TypeNode>,
    #[serde(skip_serializing)]
    pub daml_type: DamlType,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct Template {
    pub name: String,
    pub fields: Vec<Field>,
    pub signatory_exprs: Vec<Expr>,
    pub observer_exprs: Vec<Expr>,
    pub ensure_clause: Option<EnsureClause>,
    /// `key <expr> : <Type>` — expression and structured type, if declared.
    pub key_expr: Option<Expr>,
    pub key_type: Option<TypeNode>,
    pub maintainer_exprs: Vec<Expr>,
    pub choices: Vec<Choice>,
    /// Interfaces this template implements (`interface instance I for T`).
    pub interface_instances: Vec<InterfaceInstance>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct InterfaceInstance {
    pub interface_name: String,
    /// Implemented method names, in declaration order.
    pub methods: Vec<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnsureClause {
    pub expr: Expr,
    pub span: Span,
}

impl EnsureClause {
    /// Whether the ensure clause bounds `field_name` to be non-negative.
    /// Decides on the `Expr` tree: only comparisons reachable through top-level
    /// `&&` count, so `not (x > 0)` and `a || x > 0` do NOT guarantee a bound,
    /// and a field named only inside a string literal is never "bounded".
    /// (Non-strict `>= 0` is accepted — a field may allow a zero balance.)
    pub fn has_positive_bound(&self, field_name: &str) -> bool {
        self.expr
            .conjuncts()
            .iter()
            .any(|c| is_nonneg_bound(c, field_name))
    }

    /// Whether the ensure clause bounds the SIZE of `field_name` from above
    /// (`length f < N`). A mere lower bound (`length f > 0`) leaves the field
    /// unbounded and does not count. `fields` is the template's field-name set:
    /// a bound against a sibling field (`length f < cap`) is attacker-controlled
    /// and does not count either.
    pub fn has_size_bound(&self, field_name: &str, fields: &[String]) -> bool {
        self.expr
            .conjuncts()
            .iter()
            .any(|c| is_size_upper_bound(c, field_name, fields))
    }

    /// Whether the ensure clause guarantees `name` is strictly away from zero
    /// (a real division guard); runs before every choice body.
    pub fn guarantees_nonzero(&self, name: &str) -> bool {
        expr_guarantees_nonzero(&self.expr, name)
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct Choice {
    pub name: String,
    pub consuming: bool,
    pub controller_exprs: Vec<Expr>,
    /// Choice observers, if declared.
    pub observer_exprs: Vec<Expr>,
    pub parameters: Vec<Field>,
    pub return_type: Option<TypeNode>,
    pub body: Vec<Statement>,
    pub span: Span,
}

/// Do-statement classification.
///
/// Structured payloads (`value`, `condition_expr`, `cid`, `argument`) are the
/// rule-facing parse tree.
/// `Other.raw` is the deliberate raw-source form for statements with no
/// structured encoding.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub enum Statement {
    Let {
        name: String,
        value: Expr,
        span: SrcPos,
    },
    Assert {
        condition_expr: Expr,
        span: SrcPos,
    },
    Fetch {
        cid: Expr,
        /// Pattern bound by `x <- fetch cid`, if any.
        binder: Option<String>,
        span: SrcPos,
    },
    Archive {
        cid: Expr,
        span: SrcPos,
    },
    Create {
        template_name: String,
        /// The created payload (usually a Record expression).
        argument: Expr,
        binder: Option<String>,
        span: SrcPos,
    },
    Exercise {
        choice_name: String,
        cid: Expr,
        /// The choice argument (usually a Record expression), if present.
        argument: Option<Expr>,
        binder: Option<String>,
        span: SrcPos,
    },
    TryCatch {
        try_body: Vec<Self>,
        catch_body: Vec<Self>,
        span: SrcPos,
    },
    /// An `if`/`case` whose branches are NOT flattened into the parent sequence:
    /// each arm is its own statement scope. Exactly one arm runs at runtime, so an
    /// archive in one arm and a `try` in another are mutually exclusive — an
    /// ordering detector must scan each arm independently, never pairing across
    /// arms (mirrors how `TryCatch` keeps its bodies as separate scopes).
    ///
    /// `scrutinee` is the `case <e> of` expression (None for `if`), and each arm
    /// carries the source pattern it matched (None for the `if` then/else arms),
    /// so a detector can decide on the case shape structurally instead of
    /// re-scanning the body text.
    Branch {
        scrutinee: Option<Expr>,
        arms: Vec<BranchArm>,
        span: SrcPos,
    },
    Other {
        raw: String,
        /// Structured form of the statement expression.
        expr: Expr,
        binder: Option<String>,
        span: SrcPos,
    },
}

/// One arm of a `Statement::Branch`. `pattern` is the rendered case alt pattern
/// (`x :: _`, `[a]`, `_`); None for the then/else arms of an `if`.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct BranchArm {
    pub pattern: Option<String>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Function {
    pub name: String,
    /// Declared type signature, if present.
    pub type_signature: Option<TypeNode>,
    pub body: Vec<Statement>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct Import {
    pub module_name: String,
    pub qualified: bool,
    pub alias: Option<String>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct InterfaceMethod {
    pub name: String,
    pub type_: Option<TypeNode>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct Interface {
    pub name: String,
    /// Interfaces this interface requires.
    pub requires: Vec<String>,
    pub viewtype: Option<String>,
    pub methods: Vec<InterfaceMethod>,
    pub choices: Vec<Choice>,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct DamlModule {
    pub ir_version: u32,
    pub name: String,
    pub file: PathBuf,
    pub source: String,
    pub imports: Vec<Import>,
    pub templates: Vec<Template>,
    pub interfaces: Vec<Interface>,
    pub functions: Vec<Function>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use daml_parser::ast::Span as AstSpan;

    // These exercise the `Type` -> `DamlType` mapping (`from_type`), which
    // replaced the old `from_str` string reparse. The *parsing* of a source
    // string into a `Type` is tested separately in daml-parser's `type_tests`;
    // here we pin only how a parsed shape is classified for the detectors.

    fn con(name: &str) -> Type {
        Type::Con {
            qualifier: None,
            name: name.to_string(),
            span: AstSpan::default(),
        }
    }
    fn app(head: Type, args: Vec<Type>) -> Type {
        Type::App(Box::new(head), args, AstSpan::default())
    }
    fn var(name: &str) -> Type {
        Type::Var(name.to_string(), AstSpan::default())
    }
    fn unit() -> Type {
        Type::Unit(AstSpan::default())
    }
    fn tuple(items: Vec<Type>) -> Type {
        Type::Tuple(items, AstSpan::default())
    }
    fn fun(param: Type, result: Type) -> Type {
        Type::Fun(Box::new(param), Box::new(result), AstSpan::default())
    }
    fn qualified_con(qualifier: &str, name: &str) -> Type {
        Type::Con {
            qualifier: Some(qualifier.into()),
            name: name.into(),
            span: AstSpan::default(),
        }
    }

    // Regression (audit F7): `Optional (ContractId Foo)` must preserve the nested
    // ContractId — rules that recurse for ContractIds depend on it.
    #[test]
    fn nested_contractid_maps_through() {
        let ty = app(
            con("Optional"),
            vec![app(con("ContractId"), vec![con("Foo")])],
        );
        match DamlType::from_type(&ty) {
            DamlType::Optional(inner) => match *inner {
                DamlType::ContractId(c) => {
                    assert!(matches!(*c, DamlType::Named(ref n) if n == "Foo"))
                }
                other => panic!("expected ContractId inside Optional, got {:?}", other),
            },
            other => panic!("expected Optional, got {:?}", other),
        }
        // A bare `ContractId Foo` (grouping parens already unwrapped by the
        // parser) classifies as ContractId.
        assert!(matches!(
            DamlType::from_type(&app(con("ContractId"), vec![con("Foo")])),
            DamlType::ContractId(_)
        ));
        // The unit type classifies as Unit.
        assert!(matches!(DamlType::from_type(&unit()), DamlType::Unit));
    }

    // Regression (sweep F5/F6/F9/F27): the whole `Numeric` fixed-point family is
    // the money type — money detectors must see it as Decimal. (`Numeric 10`
    // parses to a bare `Con "Numeric"` because the nat literal is dropped;
    // `Numeric n` keeps the type variable.)
    #[test]
    fn numeric_family_is_decimal() {
        assert!(DamlType::from_type(&con("Numeric")).is_decimal());
        assert!(DamlType::from_type(&app(con("Numeric"), vec![var("n")])).is_decimal());
        assert!(DamlType::from_type(&con("Decimal")).is_decimal());
        // A Named type that merely starts with "Numeric" is not money.
        assert!(!DamlType::from_type(&con("NumericThing")).is_decimal());
    }

    // Regression (sweep F25): Map/Set/GenMap are unbounded collections, at any
    // qualifier.
    #[test]
    fn map_and_set_are_unbounded() {
        assert!(
            DamlType::from_type(&app(con("Map"), vec![con("Text"), con("Int")])).is_unbounded()
        );
        assert!(
            DamlType::from_type(&app(con("GenMap"), vec![con("Party"), con("Int")])).is_unbounded()
        );
        assert!(DamlType::from_type(&app(con("Set"), vec![con("Party")])).is_unbounded());
        assert!(DamlType::from_type(&app(
            qualified_con("DA.Map", "Map"),
            vec![con("Text"), con("Int")]
        ))
        .is_unbounded());
        // DELIBERATE improvement over the old prefix matcher: the common ALIASED
        // spellings `Map.Map` / `Set.Set` (the qualified imports of the stdlib
        // collections) are recognized too — the old matcher only knew `Map `,
        // `DA.Map.Map `, `Set `, `DA.Set.Set ` and missed these. This is the
        // single corpus finding the swap changed (`seriSet : Set.Set Int`).
        assert!(DamlType::from_type(&app(
            qualified_con("Map", "Map"),
            vec![con("Text"), con("Int")]
        ))
        .is_unbounded());
        assert!(
            DamlType::from_type(&app(qualified_con("Set", "Set"), vec![con("Int")])).is_unbounded()
        );
        // A Named type starting with Map/Set is not a collection.
        assert!(!DamlType::from_type(&con("MapView")).is_unbounded());
    }

    // An unrecognized constructor keeps its qualifier in the rule-facing `Named`
    // payload, so a user type stays fully spelled rather than collapsing to its
    // tail (which would alias two distinct module-qualified types).
    #[test]
    fn named_keeps_qualifier() {
        let qualified = qualified_con("Lib.Mod", "Asset");
        assert_eq!(
            DamlType::from_type(&qualified),
            DamlType::Named("Lib.Mod.Asset".to_string())
        );
        assert_eq!(
            DamlType::from_type(&con("Asset")),
            DamlType::Named("Asset".to_string())
        );
    }

    // Regression (sweep F34): a tuple `(A, B)` is its own shape, never a
    // collection or a misleading Named — it classifies as Unknown.
    #[test]
    fn tuple_type_is_unknown() {
        assert!(matches!(
            DamlType::from_type(&tuple(vec![con("Int"), con("Text")])),
            DamlType::Unknown
        ));
    }

    // The new model's headline win: a *function* type is known to be an arrow,
    // not swallowed into `Named` the way the string matcher did with anything
    // starting uppercase (`Int -> Int` was `Named("Int -> Int")`).
    #[test]
    fn function_type_is_unknown_not_named() {
        let arrow = fun(con("Int"), con("Int"));
        assert!(matches!(DamlType::from_type(&arrow), DamlType::Unknown));
    }

    // Whole-identifier matching is now structural: `Expr::refers_to` compares
    // variable names exactly, so `count` never matches inside `discount`. The
    // intent is exercised end-to-end by the ensure_decimal /
    // unbounded_fields substring-field regression tests.
}
