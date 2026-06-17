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

pub(crate) struct SourceTextMap<'a> {
    file: &'a Path,
    source: &'a str,
    line_start_bytes: Vec<usize>,
    utf16_offset_by_byte: Vec<usize>,
}

impl<'a> SourceTextMap<'a> {
    pub(crate) fn new(file: &'a Path, source: &'a str) -> Self {
        let mut line_start_bytes = vec![0];
        for (idx, byte) in source.bytes().enumerate() {
            if byte == b'\n' {
                line_start_bytes.push(idx + 1);
            }
        }
        let mut utf16_offset_by_byte = vec![0; source.len() + 1];
        let mut utf16 = 0usize;
        let mut prev = 0usize;
        for (idx, ch) in source.char_indices() {
            for slot in utf16_offset_by_byte.iter_mut().take(idx).skip(prev) {
                *slot = utf16;
            }
            let char_end = idx + ch.len_utf8();
            for slot in utf16_offset_by_byte.iter_mut().take(char_end).skip(idx) {
                *slot = utf16;
            }
            utf16 += ch.len_utf16();
            prev = char_end;
        }
        for slot in utf16_offset_by_byte
            .iter_mut()
            .take(source.len() + 1)
            .skip(prev)
        {
            *slot = utf16;
        }
        Self {
            file,
            source,
            line_start_bytes,
            utf16_offset_by_byte,
        }
    }

    fn line_column_for_byte(&self, byte: usize) -> (usize, usize) {
        let byte = byte.min(self.source.len());
        let line_idx = match self.line_start_bytes.binary_search(&byte) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };
        let line_start = self.line_start_bytes[line_idx];
        (
            line_idx + 1,
            self.source[line_start..byte].chars().count() + 1,
        )
    }

    fn source_span_for_parser_span(&self, span: ParserSpan) -> SourceSpan {
        let byte_start = span.start.min(self.source.len());
        let byte_end = span.end.min(self.source.len()).max(byte_start);
        let (line, column) = self.line_column_for_byte(byte_start);
        SourceSpan {
            file: self.file.to_path_buf(),
            line,
            column,
            start: self.utf16_offset_by_byte[byte_start],
            end: self.utf16_offset_by_byte[byte_end],
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
    pub(crate) fn from_type(t: &Type, source_map: &SourceTextMap<'_>) -> Self {
        let source_span = || source_map.source_span_for_parser_span(t.span());
        match t {
            Type::Con {
                qualifier, name, ..
            } => Self::Con {
                qualifier: qualifier.clone(),
                name: name.clone(),
                span: source_span(),
            },
            Type::App(head, args, _) => Self::App {
                head: Box::new(Self::from_type(head, source_map)),
                args: args
                    .iter()
                    .map(|arg| Self::from_type(arg, source_map))
                    .collect(),
                span: source_span(),
            },
            Type::List(inner, _) => Self::List {
                inner: Box::new(Self::from_type(inner, source_map)),
                span: source_span(),
            },
            Type::Tuple(items, _) => Self::Tuple {
                items: items
                    .iter()
                    .map(|item| Self::from_type(item, source_map))
                    .collect(),
                span: source_span(),
            },
            Type::Fun(param, result, _) => Self::Fun {
                param: Box::new(Self::from_type(param, source_map)),
                result: Box::new(Self::from_type(result, source_map)),
                span: source_span(),
            },
            Type::Var(name, _) => Self::Var {
                name: name.clone(),
                span: source_span(),
            },
            Type::Unit(_) => Self::Unit {
                span: source_span(),
            },
            Type::Constrained(body, _) => Self::Constrained {
                body: Box::new(Self::from_type(body, source_map)),
                span: source_span(),
            },
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

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Field {
    pub name: String,
    pub type_: Option<TypeNode>,
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
