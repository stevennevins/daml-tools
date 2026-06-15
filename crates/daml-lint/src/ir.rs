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
        self.is_text() || self.is_textmap() || self.is_list() || self.is_map()
    }
}

fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_' || b == b'\''
}

/// True if `pattern` occurs in `text` with a non-identifier char (or string
/// start) immediately before it. Use when `pattern` BEGINS with the identifier
/// you care about, so `"count > 0"` is not matched inside `"discount > 0"`.
pub(crate) fn contains_left_anchored(text: &str, pattern: &str) -> bool {
    let b = text.as_bytes();
    let mut from = 0;
    while let Some(rel) = text[from..].find(pattern) {
        let i = from + rel;
        if i == 0 || !is_ident_byte(b[i - 1]) {
            return true;
        }
        from = i + 1;
    }
    false
}

/// True if `pattern` occurs in `text` with a non-identifier char (or string
/// end) immediately after it. Use when `pattern` ENDS with the identifier you
/// care about, so `"0 < amount"` is not matched inside `"0 < amountDue"`.
pub(crate) fn contains_right_anchored(text: &str, pattern: &str) -> bool {
    let b = text.as_bytes();
    let mut from = 0;
    while let Some(rel) = text[from..].find(pattern) {
        let i = from + rel;
        let j = i + pattern.len();
        if j == text.len() || !is_ident_byte(b[j]) {
            return true;
        }
        from = i + 1;
    }
    false
}

/// True if `ident` appears in `text` as a whole identifier token (bounded on
/// both sides), so `"q"` does not match inside `"quantity"`.
pub(crate) fn mentions_ident(text: &str, ident: &str) -> bool {
    if ident.is_empty() {
        return false;
    }
    let b = text.as_bytes();
    let mut from = 0;
    while let Some(rel) = text[from..].find(ident) {
        let i = from + rel;
        let j = i + ident.len();
        let left = i == 0 || !is_ident_byte(b[i - 1]);
        let right = j == text.len() || !is_ident_byte(b[j]);
        if left && right {
            return true;
        }
        from = i + 1;
    }
    false
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

/// Blank out line comments (`-- … EOL`), block comments (`{- … -}`), and the
/// contents of double-quoted string literals, so a substring scan doesn't match
/// inside lexical noise (a `> 0` in a comment is not a real guard). Lengths and
/// newlines are preserved so line indexing stays valid.
pub(crate) fn code_only(src: &str) -> String {
    let b = src.as_bytes();
    let n = b.len();
    let mut out = String::with_capacity(n);
    let mut i = 0;
    while i < n {
        // line comment
        if b[i] == b'-' && b.get(i + 1) == Some(&b'-') {
            while i < n && b[i] != b'\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        // block comment
        if b[i] == b'{' && b.get(i + 1) == Some(&b'-') {
            out.push_str("  ");
            i += 2;
            while i < n && !(b[i] == b'-' && b.get(i + 1) == Some(&b'}')) {
                out.push(if b[i] == b'\n' { '\n' } else { ' ' });
                i += 1;
            }
            if i < n {
                out.push_str("  ");
                i += 2;
            }
            continue;
        }
        // string literal
        if b[i] == b'"' {
            out.push('"');
            i += 1;
            while i < n && b[i] != b'"' {
                if b[i] == b'\\' && i + 1 < n {
                    out.push_str("  ");
                    i += 2;
                    continue;
                }
                out.push(' ');
                i += 1;
            }
            if i < n {
                out.push('"');
                i += 1;
            }
            continue;
        }
        // ordinary char (copy whole UTF-8 char; i is at a boundary here)
        let ch = src[i..].chars().next().unwrap();
        out.push(ch);
        i += ch.len_utf8();
    }
    out
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
    pub fn references_field_with_bound(&self, field_name: &str, bound: &str) -> bool {
        let text = &self.raw_text;
        text.contains(field_name) && text.contains(bound)
    }

    pub fn references_field(&self, field_name: &str) -> bool {
        self.raw_text.contains(field_name)
    }

    /// Whether the ensure clause bounds `field_name` to be non-negative. The
    /// field sits at the identifier end of each pattern, so anchor on it: this
    /// stops `count` from matching inside `discount > 0`. (Non-strict `>= 0` is
    /// accepted on purpose — a field may legitimately allow a zero balance.)
    pub fn has_positive_bound(&self, field_name: &str) -> bool {
        let text = &self.raw_text;
        contains_left_anchored(text, &format!("{} > 0", field_name))
            || contains_left_anchored(text, &format!("{} >= 0", field_name))
            || contains_right_anchored(text, &format!("0 < {}", field_name))
            || contains_right_anchored(text, &format!("0 <= {}", field_name))
            || contains_right_anchored(text, &format!("0.0 < {}", field_name))
            || contains_right_anchored(text, &format!("0.0 <= {}", field_name))
    }

    pub fn has_size_bound(&self, field_name: &str) -> bool {
        let text = &self.raw_text;
        // The field is the argument at the end of each pattern, so right-anchor
        // it so `reason` is not matched inside `reasons`.
        contains_right_anchored(text, &format!("T.length {}", field_name))
            || contains_right_anchored(text, &format!("Text.length {}", field_name))
            || contains_right_anchored(text, &format!("DA.Text.length {}", field_name))
            || contains_right_anchored(text, &format!("length {}", field_name))
            || contains_right_anchored(text, &format!("Map.size {}", field_name))
            || contains_right_anchored(text, &format!("size {}", field_name))
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
    Other {
        raw: String,
        /// Structured form of the statement expression.
        expr: Expr,
        binder: Option<String>,
        span: SrcPos,
    },
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

    // Regression (sweep F1/F7/F12/F29): whole-identifier matching, so `q` is not
    // found inside `quantity` and `count` not inside `discount`.
    #[test]
    fn identifier_matchers_respect_token_boundaries() {
        assert!(mentions_ident("(q > 0)", "q"));
        assert!(!mentions_ident("(quantity > 0)", "q"));
        assert!(mentions_ident("r.amount > 0", "r.amount"));

        assert!(contains_left_anchored("count > 0", "count > 0"));
        assert!(!contains_left_anchored("discount > 0", "count > 0"));

        assert!(contains_right_anchored("length reason", "length reason"));
        assert!(!contains_right_anchored("length reasons", "length reason"));
    }
}
