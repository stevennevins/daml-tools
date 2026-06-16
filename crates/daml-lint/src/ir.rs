use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct Span {
    pub file: PathBuf,
    pub line: usize,
    pub column: usize,
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
    ContractId(Box<DamlType>),
    List(Box<DamlType>),
    Optional(Box<DamlType>),
    TextMap(Box<DamlType>),
    Map(Box<DamlType>, Box<DamlType>),
    Named(String),
    Unit,
    Unknown,
}

impl DamlType {
    // Inherent `from_str`: total (returns DamlType, never fails), so it does not
    // fit std::str::FromStr's `Result` signature.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> DamlType {
        // Strip a single matched outer pair of grouping parens, so
        // `Optional (ContractId Foo)` recurses into `ContractId Foo` instead of
        // collapsing to Unknown. Never strips the unit type `()`.
        fn strip_grouping_parens(s: &str) -> &str {
            let t = s.trim();
            if t.len() < 2 || !t.starts_with('(') || !t.ends_with(')') {
                return s;
            }
            let mut depth = 0usize;
            for (i, c) in t.char_indices() {
                match c {
                    '(' => depth += 1,
                    ')' => {
                        depth -= 1;
                        if depth == 0 {
                            // The opening paren must match the LAST char; `(a)(b)`
                            // is not a single grouping pair.
                            if i != t.len() - 1 {
                                return s;
                            }
                            let inner = t[1..t.len() - 1].trim();
                            // Keep `()` (unit) and tuples `(a, b)` intact — only
                            // strip a genuine grouping paren around one type.
                            return if inner.is_empty() || has_top_level_comma(inner) {
                                s
                            } else {
                                inner
                            };
                        }
                    }
                    _ => {}
                }
            }
            s
        }
        let s = strip_grouping_parens(s.trim());
        if s == "Party" {
            DamlType::Party
        } else if s == "Text" {
            DamlType::Text
        } else if s == "Decimal" {
            DamlType::Decimal
        } else if s == "Int" {
            DamlType::Int
        } else if s == "Bool" {
            DamlType::Bool
        } else if s == "Date" {
            DamlType::Date
        } else if s == "Time" {
            DamlType::Time
        } else if s == "()" {
            DamlType::Unit
        } else if let Some(inner) = s.strip_prefix("ContractId ") {
            DamlType::ContractId(Box::new(DamlType::from_str(inner)))
        } else if let Some(inner) = s.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            DamlType::List(Box::new(DamlType::from_str(inner)))
        } else if let Some(inner) = s.strip_prefix("Optional ") {
            DamlType::Optional(Box::new(DamlType::from_str(inner)))
        } else if let Some(inner) = s.strip_prefix("TextMap ") {
            DamlType::TextMap(Box::new(DamlType::from_str(inner)))
        } else if s == "Numeric" || s.starts_with("Numeric ") {
            // Decimal is exactly Numeric 10; the whole fixed-point family is the
            // money type the monetary detectors care about.
            DamlType::Decimal
        } else if let Some(rest) = s
            .strip_prefix("GenMap ")
            .or_else(|| s.strip_prefix("Map "))
            .or_else(|| s.strip_prefix("DA.Map.Map "))
            .or_else(|| s.strip_prefix("DA.Map.GenMap "))
        {
            let (k, v) = split_top_level_ws(rest);
            DamlType::Map(
                Box::new(DamlType::from_str(k)),
                Box::new(DamlType::from_str(v)),
            )
        } else if let Some(inner) = s
            .strip_prefix("Set ")
            .or_else(|| s.strip_prefix("DA.Set.Set "))
        {
            // No dedicated Set variant; model as an unbounded collection (List)
            // so unbounded-fields flags it.
            DamlType::List(Box::new(DamlType::from_str(inner)))
        } else if s.starts_with(char::is_uppercase) {
            DamlType::Named(s.to_string())
        } else {
            DamlType::Unknown
        }
    }

    pub fn is_decimal(&self) -> bool {
        matches!(self, DamlType::Decimal)
    }

    pub fn is_text(&self) -> bool {
        matches!(self, DamlType::Text)
    }

    pub fn is_textmap(&self) -> bool {
        matches!(self, DamlType::TextMap(_))
    }

    pub fn is_list(&self) -> bool {
        matches!(self, DamlType::List(_))
    }

    pub fn is_map(&self) -> bool {
        matches!(self, DamlType::Map(_, _))
    }

    pub fn is_unbounded(&self) -> bool {
        match self {
            // An `Optional Text` / `Optional [a]` is still unbounded when present.
            DamlType::Optional(inner) => inner.is_unbounded(),
            _ => self.is_text() || self.is_textmap() || self.is_list() || self.is_map(),
        }
    }
}

/// Split `s` at its first top-level (not inside `()`/`[]`) space into
/// (head, rest). Used to peel one type argument off `Map k v`.
fn split_top_level_ws(s: &str) -> (&str, &str) {
    let mut depth = 0i32;
    for (i, b) in s.bytes().enumerate() {
        match b {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b' ' if depth == 0 => return (s[..i].trim(), s[i + 1..].trim()),
            _ => {}
        }
    }
    (s.trim(), "")
}

/// True if `s` has a top-level (depth-0) comma — i.e. it is a tuple body, not a
/// single grouped type.
fn has_top_level_comma(s: &str) -> bool {
    let mut depth = 0i32;
    for b in s.bytes() {
        match b {
            b'(' | b'[' => depth += 1,
            b')' | b']' => depth -= 1,
            b',' if depth == 0 => return true,
            _ => {}
        }
    }
    false
}

/// Lightweight source position for expression-level nodes (1-based). The
/// enclosing module fixes the file; repeating the path on every node would
/// bloat the JSON handed to rule scripts.
#[derive(Debug, Clone, Copy, Serialize)]
pub struct SrcPos {
    pub line: usize,
    pub column: usize,
}

/// Expression AST exposed to rule scripts. Serialized as tagged unions:
/// `{ "App": {...} }`, `{ "Lit": {...} }`, ... mirrored by daml-lint.d.ts.
#[derive(Debug, Clone, Serialize)]
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
        func: Box<Expr>,
        args: Vec<Expr>,
        span: SrcPos,
    },
    /// Binary operator with source-level operator text (`+`, `/`, `&&`,
    /// `` `div` `` for backtick application, `..` for ranges).
    BinOp {
        op: String,
        lhs: Box<Expr>,
        rhs: Box<Expr>,
        span: SrcPos,
    },
    Neg {
        expr: Box<Expr>,
        span: SrcPos,
    },
    Lambda {
        params: Vec<String>,
        body: Box<Expr>,
        span: SrcPos,
    },
    If {
        cond: Box<Expr>,
        then_branch: Box<Expr>,
        else_branch: Box<Expr>,
        span: SrcPos,
    },
    Case {
        scrutinee: Box<Expr>,
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
        body: Box<Expr>,
        span: SrcPos,
    },
    /// Record construction or update: `Foo with x = 1`, `this with owner`.
    Record {
        base: Box<Expr>,
        fields: Vec<RecordField>,
        span: SrcPos,
    },
    Tuple {
        items: Vec<Expr>,
        span: SrcPos,
    },
    List {
        items: Vec<Expr>,
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

#[derive(Debug, Clone, Serialize)]
pub struct CaseAlt {
    /// Pattern rendered to source text (`Some x`, `[]`, `_`).
    pub pattern: String,
    pub body: Expr,
}

#[derive(Debug, Clone, Serialize)]
pub struct LetBinding {
    pub name: String,
    pub value: Expr,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordField {
    pub name: String,
    /// None for punned fields (`Foo with owner`) and `..` spreads.
    pub value: Option<Expr>,
}

// ----- Expr analysis ------------------------------------------------------
//
// Detectors decide on the structured `Expr` tree, not on `raw_text`. Walking
// the tree makes a whole class of lexical bugs vanish: `not (...)`, `a || b`,
// operator direction, and `> 0` mentions hiding in comments / string literals
// all stop mattering, because the tree carries the real meaning.

impl Expr {
    /// Flatten a top-level `&&` conjunction into its leaf conditions. Only
    /// these are *guaranteed* to hold: anything under `||`, `not (...)`, or an
    /// `if` is returned as one opaque leaf, so it can never masquerade as a
    /// guarantee.
    pub(crate) fn conjuncts(&self) -> Vec<&Expr> {
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
            Expr::Var {
                name, qualifier, ..
            }
            | Expr::Con {
                name, qualifier, ..
            } => Some(match qualifier {
                Some(q) => format!("{}.{}", q, name),
                None => name.clone(),
            }),
            Expr::BinOp { op, lhs, rhs, .. } if op == "." => {
                Some(format!("{}.{}", lhs.ref_string()?, rhs.ref_string()?))
            }
            _ => None,
        }
    }

    /// True if this expression refers to `name`: a bare reference, or a
    /// `this.<name>` / `self.<name>` projection (the implicit-record forms a
    /// choice body may use for a template field).
    pub(crate) fn refers_to(&self, name: &str) -> bool {
        match self.ref_string() {
            Some(s) => s == name || strip_implicit_self(&s) == strip_implicit_self(name),
            None => false,
        }
    }

    /// True if this is a numeric literal equal to zero (`0`, `0.0`, `0.`).
    pub(crate) fn is_zero_lit(&self) -> bool {
        match self {
            Expr::Lit { kind, value, .. } if kind == "Int" || kind == "Decimal" => {
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
        matches!(self, Expr::Lit { kind, .. } if kind == "Int" || kind == "Decimal")
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
            Expr::Neg { expr, .. } => expr.is_nonzero_numeric_divisor(),
            _ => self.is_nonzero_numeric_lit(),
        }
    }

    /// True if this is a non-negative numeric literal (`0`, `0.01`, `100.0`).
    /// Negative literals are spelled `Neg(Lit)` and do not match.
    pub(crate) fn is_nonneg_numeric_lit(&self) -> bool {
        matches!(self, Expr::Lit { kind, .. } if kind == "Int" || kind == "Decimal")
    }

    /// Best-effort source-ish rendering, for evidence / messages only.
    pub(crate) fn render_text(&self) -> String {
        match self {
            Expr::Var { .. } | Expr::Con { .. } => self.ref_string().unwrap_or_default(),
            Expr::Lit { value, .. } => value.clone(),
            Expr::Neg { expr, .. } => format!("-{}", expr.render_text()),
            Expr::BinOp { op, lhs, rhs, .. } if op == "." => {
                format!("{}.{}", lhs.render_text(), rhs.render_text())
            }
            Expr::BinOp { op, lhs, rhs, .. } => {
                format!("{} {} {}", lhs.render_text(), op, rhs.render_text())
            }
            Expr::App { func, args, .. } => {
                let mut s = func.render_text();
                for a in args {
                    s.push(' ');
                    s.push_str(&a.render_text());
                }
                s
            }
            Expr::Tuple { items, .. } => format!("({})", render_join(items, ", ")),
            Expr::List { items, .. } => format!("[{}]", render_join(items, ", ")),
            Expr::Unknown { raw, .. } => raw.clone(),
            _ => "…".to_string(),
        }
    }

    /// The head of an application spine: `f a b` → `f`, `query @T x` → `query`.
    pub(crate) fn app_head(&self) -> &Expr {
        match self {
            Expr::App { func, .. } => func.app_head(),
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
        Statement::Exercise { cid, argument, .. } => match argument {
            Some(a) => vec![cid, a],
            None => vec![cid],
        },
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

#[derive(Debug, Clone, Serialize)]
pub struct Field {
    pub name: String,
    pub type_: DamlType,
    pub span: Span,
}

#[derive(Debug, Clone, Serialize)]
pub struct Template {
    pub name: String,
    pub fields: Vec<Field>,
    pub signatories: Vec<String>,
    pub observers: Vec<String>,
    /// Structured party expressions behind `signatories`/`observers`.
    pub signatory_exprs: Vec<Expr>,
    pub observer_exprs: Vec<Expr>,
    pub ensure_clause: Option<EnsureClause>,
    /// `key <expr> : <Type>` — expression and type text, if declared.
    pub key_expr: Option<Expr>,
    pub key_type: Option<String>,
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
    pub raw_text: String,
    /// Structured ensure condition.
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
    pub controllers: Vec<String>,
    /// Structured controller expressions behind `controllers`.
    pub controller_exprs: Vec<Expr>,
    /// Choice observers, if declared.
    pub observer_exprs: Vec<Expr>,
    pub parameters: Vec<Field>,
    pub return_type: DamlType,
    pub body: Vec<Statement>,
    pub body_raw: String,
    pub span: Span,
}

/// Do-statement classification. Raw-text fields (`expr`, `condition`,
/// `raw`) are kept for compatibility; the structured payloads (`value`,
/// `cid`, `argument`, ...) are the real parse tree.
#[derive(Debug, Clone, Serialize)]
pub enum Statement {
    Let {
        name: String,
        expr: String,
        value: Expr,
        span: SrcPos,
    },
    Assert {
        condition: String,
        condition_expr: Expr,
        span: SrcPos,
    },
    Fetch {
        cid_expr: String,
        cid: Expr,
        /// Pattern bound by `x <- fetch cid`, if any.
        binder: Option<String>,
        span: SrcPos,
    },
    Archive {
        cid_expr: String,
        cid: Expr,
        span: SrcPos,
    },
    Create {
        template_name: String,
        raw: String,
        /// The created payload (usually a Record expression).
        argument: Expr,
        binder: Option<String>,
        span: SrcPos,
    },
    Exercise {
        cid_expr: String,
        choice_name: String,
        raw: String,
        cid: Expr,
        /// The choice argument (usually a Record expression), if present.
        argument: Option<Expr>,
        binder: Option<String>,
        span: SrcPos,
    },
    TryCatch {
        try_body: Vec<Statement>,
        catch_body: Vec<Statement>,
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
#[derive(Debug, Clone, Serialize)]
pub struct BranchArm {
    pub pattern: Option<String>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Function {
    pub name: String,
    /// Declared type signature text, if present.
    pub type_signature: Option<String>,
    pub body: Vec<Statement>,
    pub body_raw: String,
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
    pub type_text: String,
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

    // Regression (audit F7): `Optional (ContractId Foo)` — the only valid way to
    // spell that type — must preserve the nested ContractId, not collapse the
    // parenthesized argument to Unknown. Rules that recurse for ContractIds
    // depend on it.
    #[test]
    fn parenthesized_type_argument_is_parsed() {
        match DamlType::from_str("Optional (ContractId Foo)") {
            DamlType::Optional(inner) => match *inner {
                DamlType::ContractId(c) => {
                    assert!(matches!(*c, DamlType::Named(ref n) if n == "Foo"))
                }
                other => panic!("expected ContractId inside Optional, got {:?}", other),
            },
            other => panic!("expected Optional, got {:?}", other),
        }
        // `[ContractId Foo]` and bare grouping also resolve.
        assert!(matches!(
            DamlType::from_str("(ContractId Foo)"),
            DamlType::ContractId(_)
        ));
        // The unit type must survive paren-stripping.
        assert!(matches!(DamlType::from_str("()"), DamlType::Unit));
    }

    // Regression (sweep F5/F6/F9/F27): `Numeric n` is the modern money type and
    // must be treated as Decimal so the monetary detectors see it.
    #[test]
    fn numeric_is_decimal() {
        assert!(DamlType::from_str("Numeric 10").is_decimal());
        assert!(DamlType::from_str("Numeric 6").is_decimal());
        assert!(DamlType::from_str("Numeric").is_decimal());
        assert!(DamlType::from_str("Decimal").is_decimal());
        // Not a false match on a Named type that merely starts with "Numeric".
        assert!(!DamlType::from_str("NumericThing").is_decimal());
    }

    // Regression (sweep F25): Map/Set/GenMap are unbounded collections.
    #[test]
    fn map_and_set_are_unbounded() {
        assert!(DamlType::from_str("Map Text Int").is_unbounded());
        assert!(DamlType::from_str("GenMap Party Int").is_unbounded());
        assert!(DamlType::from_str("Set Party").is_unbounded());
        assert!(DamlType::from_str("DA.Map.Map Text Int").is_unbounded());
        // A Named type starting with Map/Set is not a collection.
        assert!(!DamlType::from_str("MapView").is_unbounded());
    }

    // Regression (sweep F34): a tuple `(A, B)` is not a grouping paren and must
    // not be mis-parsed as Named("A, B").
    #[test]
    fn tuple_type_is_unknown_not_named() {
        assert!(matches!(
            DamlType::from_str("(Int, Text)"),
            DamlType::Unknown
        ));
        assert!(matches!(DamlType::from_str("(a, b, c)"), DamlType::Unknown));
    }

    // Whole-identifier matching is now structural: `Expr::refers_to` compares
    // variable names exactly, so `count` never matches inside `discount`. The
    // intent is exercised end-to-end by the ensure_decimal /
    // unbounded_fields substring-field regression tests.
}
