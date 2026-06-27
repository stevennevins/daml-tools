//! Rule-facing IR for `daml-lint`.
//!
//! This module intentionally prefers forward-compatible matching for public enum
//! nodes. The IR mirrors the parsed Daml AST at a lossy-compatibility boundary:
//! `TypeNode`, `LiteralKind`, `Expr`, `Consuming`, `Statement`, and
//! `ImportStyle` are all `#[non_exhaustive]` so downstream crates should add
//! wildcard arms when matching instead of exhaustiveness assumptions.

use daml_parser::ast::Type;
use daml_syntax::{ByteOffset, CharColumn, LineNumber, SourceFile, TextRange, Utf16Offset};
use serde::{Serialize, Serializer};
use std::fmt;
use std::path::{Path, PathBuf};

fn serialize_line_number<S>(line: &LineNumber, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_u64(line.get() as u64)
}

fn serialize_char_column<S>(column: &CharColumn, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_u64(column.get() as u64)
}

fn serialize_utf16_offset<S>(offset: &Utf16Offset, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_u64(offset.get() as u64)
}

fn serialize_byte_offset<S>(offset: &ByteOffset, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    serializer.serialize_u64(offset.get() as u64)
}

/// File, line, and column location for declaration-level IR nodes.
///
/// `line` and `column` are 1-based Unicode-scalar coordinates in `file`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Span {
    /// Source file that owns the located syntax.
    pub file: PathBuf,
    /// 1-based source line.
    #[serde(serialize_with = "serialize_line_number")]
    pub line: LineNumber,
    /// 1-based Unicode-scalar source column.
    #[serde(serialize_with = "serialize_char_column")]
    pub column: CharColumn,
}

/// Source range for type nodes and other source-slice-aware IR.
///
/// `line`/`column` identify the range start using 1-based Unicode-scalar
/// coordinates. `start`/`end` are UTF-16 code-unit offsets into
/// [`DamlModule::source`] for JavaScript string slicing. `byte_start`/`byte_end`
/// are byte offsets into the UTF-8 source for Rust callers.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct SourceSpan {
    /// Source file that owns the range.
    pub file: PathBuf,
    /// 1-based source line at the start of the range.
    #[serde(serialize_with = "serialize_line_number")]
    pub line: LineNumber,
    /// 1-based Unicode-scalar column at the start of the range.
    #[serde(serialize_with = "serialize_char_column")]
    pub column: CharColumn,
    /// UTF-16 code-unit offset into `DamlModule.source`, suitable for
    /// JavaScript's `module.source.slice(start, end)`.
    #[serde(serialize_with = "serialize_utf16_offset")]
    pub start: Utf16Offset,
    /// Exclusive UTF-16 code-unit offset into `DamlModule.source`.
    #[serde(serialize_with = "serialize_utf16_offset")]
    pub end: Utf16Offset,
    /// Inclusive start byte offset into the UTF-8 source.
    #[serde(serialize_with = "serialize_byte_offset")]
    pub byte_start: ByteOffset,
    /// Exclusive end byte offset into the UTF-8 source.
    #[serde(serialize_with = "serialize_byte_offset")]
    pub byte_end: ByteOffset,
}

impl SourceSpan {
    fn from_text_range(file: &Path, source_file: &SourceFile, range: TextRange) -> Self {
        let line_col = source_file.line_index().char_line_col(range.start());
        let utf16_range = source_file.line_index().utf16_range(range);
        Self {
            file: file.to_path_buf(),
            line: line_col.line,
            column: line_col.column,
            start: utf16_range.start(),
            end: utf16_range.end(),
            byte_start: range.start().into(),
            byte_end: range.end().into(),
        }
    }
}

/// Structured Daml type syntax exposed to Rust callers and custom rules.
///
/// Serialized to JavaScript as a serde externally tagged union such as
/// `{ "Con": { ... } }`. Variants are `#[non_exhaustive]`; include wildcard
/// arms when matching so new Daml type syntax can be added without forcing
/// downstream rewrites.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum TypeNode {
    /// Named type constructor such as `Party`, `ContractId`, or `DA.Date.Date`.
    Con {
        /// Module qualifier when the source used a qualified constructor.
        qualifier: Option<String>,
        /// Unqualified constructor name.
        name: String,
        /// Source range of the constructor type expression.
        span: SourceSpan,
    },
    /// Type application such as `ContractId Asset`.
    App {
        /// Applied type constructor or function.
        head: Box<Self>,
        /// Application arguments in source order.
        args: Vec<Self>,
        /// Source range of the full application.
        span: SourceSpan,
    },
    /// List type `[a]`.
    List {
        /// Element type.
        inner: Box<Self>,
        /// Source range of the list type.
        span: SourceSpan,
    },
    /// Tuple type `(a, b, ...)`.
    Tuple {
        /// Tuple item types in source order.
        items: Vec<Self>,
        /// Source range of the tuple type.
        span: SourceSpan,
    },
    /// Function type `a -> b`.
    Fun {
        /// Parameter/input type.
        param: Box<Self>,
        /// Result/output type.
        result: Box<Self>,
        /// Source range of the function type.
        span: SourceSpan,
    },
    /// Type variable such as `a`.
    Var {
        /// Variable name.
        name: String,
        /// Source range of the variable type.
        span: SourceSpan,
    },
    /// Unit type `()`.
    Unit {
        /// Source range of the unit type.
        span: SourceSpan,
    },
    /// Constrained type body; constraint details are currently lossily omitted.
    Constrained {
        /// Type body after constraints are stripped.
        body: Box<Self>,
        /// Source range of the constrained type.
        span: SourceSpan,
    },
    /// Type-level literal such as a numeric, text, or char literal.
    Lit {
        /// Literal kind.
        kind: LiteralKind,
        /// Literal text as it appeared in the parsed type.
        value: String,
        /// Source range of the literal type.
        span: SourceSpan,
    },
}

impl TypeNode {
    pub(crate) fn from_type(t: &Type, file: &Path, source_file: &SourceFile) -> Self {
        let source_span = || {
            SourceSpan::from_text_range(
                file,
                source_file,
                source_file.parser_span_to_text_range(t.span()),
            )
        };
        match t {
            Type::Con {
                qualifier, name, ..
            } => Self::Con {
                qualifier: qualifier.to_owned().map(String::from),
                name: name.to_string(),
                span: source_span(),
            },
            Type::App(head, args, _) => Self::App {
                head: Box::new(Self::from_type(head, file, source_file)),
                args: args
                    .iter()
                    .map(|arg| Self::from_type(arg, file, source_file))
                    .collect(),
                span: source_span(),
            },
            Type::List(inner, _) => Self::List {
                inner: Box::new(Self::from_type(inner, file, source_file)),
                span: source_span(),
            },
            Type::Tuple(items, _) => Self::Tuple {
                items: items
                    .iter()
                    .map(|item| Self::from_type(item, file, source_file))
                    .collect(),
                span: source_span(),
            },
            Type::Fun(param, result, _) => Self::Fun {
                param: Box::new(Self::from_type(param, file, source_file)),
                result: Box::new(Self::from_type(result, file, source_file)),
                span: source_span(),
            },
            Type::Var(name, _) => Self::Var {
                name: name.to_string(),
                span: source_span(),
            },
            Type::Unit(_) => Self::Unit {
                span: source_span(),
            },
            Type::Constrained(body, _) => Self::Constrained {
                body: Box::new(Self::from_type(body, file, source_file)),
                span: source_span(),
            },
            Type::Lit { kind, text, .. } => Self::Lit {
                kind: match kind {
                    daml_parser::ast::LitKind::Char => LiteralKind::Char,
                    daml_parser::ast::LitKind::Int => LiteralKind::Int,
                    daml_parser::ast::LitKind::Decimal => LiteralKind::Decimal,
                    _ => LiteralKind::Text,
                },
                value: text.clone(),
                span: source_span(),
            },
            _ => Self::Con {
                qualifier: None,
                name: "<unknown>".to_string(),
                span: source_span(),
            },
        }
    }
}

/// Lightweight source position for expression-level nodes (1-based). The
/// enclosing module fixes the file; repeating the path on every node would
/// bloat the JSON handed to rule scripts.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct SrcPos {
    /// 1-based source line.
    #[serde(serialize_with = "serialize_line_number")]
    pub line: LineNumber,
    /// 1-based Unicode-scalar source column.
    #[serde(serialize_with = "serialize_char_column")]
    pub column: CharColumn,
}

/// Literal token category used by [`Expr::Lit`] and [`TypeNode::Lit`].
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum LiteralKind {
    /// Integer literal.
    Int,
    /// Decimal literal.
    Decimal,
    /// Text/string literal.
    Text,
    /// Character literal.
    Char,
}

/// Expression AST exposed to rule scripts. Serialized as tagged unions:
/// `{ "App": {...} }`, `{ "Lit": {...} }`, ... mirrored by daml-lint.d.ts.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum Expr {
    /// Variable reference: `amount`, `Map.lookup` (qualifier "Map").
    Var {
        /// Unqualified variable name.
        name: String,
        /// Module or record qualifier, when present in source.
        qualifier: Option<String>,
        /// 1-based source position of the variable.
        span: SrcPos,
    },
    /// Constructor or type reference in expression position: `Some`, `Iou`.
    Con {
        /// Unqualified constructor name.
        name: String,
        /// Module qualifier, when present in source.
        qualifier: Option<String>,
        /// 1-based source position of the constructor.
        span: SrcPos,
    },
    /// Literal; kind is "Int" | "Decimal" | "Text" | "Char".
    Lit {
        /// Literal category.
        kind: LiteralKind,
        /// Literal text/value captured from the parser.
        value: String,
        /// 1-based source position of the literal.
        span: SrcPos,
    },
    /// Application, flattened: `f a b` has two args.
    App {
        /// Function expression being applied.
        func: Box<Self>,
        /// Application arguments in source order.
        args: Vec<Self>,
        /// 1-based source position of the application.
        span: SrcPos,
    },
    /// Binary operator with source-level operator text (`+`, `/`, `&&`,
    /// `` `div` `` for backtick application, `..` for ranges).
    BinOp {
        /// Source-level operator token or backtick operator text.
        op: String,
        /// Left-hand operand.
        lhs: Box<Self>,
        /// Right-hand operand.
        rhs: Box<Self>,
        /// 1-based source position of the operator expression.
        span: SrcPos,
    },
    /// Unary negation.
    Neg {
        /// Negated expression.
        expr: Box<Self>,
        /// 1-based source position of the negation.
        span: SrcPos,
    },
    /// Lambda expression.
    Lambda {
        /// Parameter patterns rendered as source text.
        params: Vec<String>,
        /// Lambda body.
        body: Box<Self>,
        /// 1-based source position of the lambda.
        span: SrcPos,
    },
    /// `if` expression.
    If {
        /// Condition expression.
        cond: Box<Self>,
        /// Then branch expression.
        then_branch: Box<Self>,
        /// Else branch expression.
        else_branch: Box<Self>,
        /// 1-based source position of the `if`.
        span: SrcPos,
    },
    /// `case` expression.
    Case {
        /// Scrutinee expression after `case`.
        scrutinee: Box<Self>,
        /// Alternatives in source order.
        alts: Vec<CaseAlt>,
        /// 1-based source position of the `case`.
        span: SrcPos,
    },
    /// Nested do block, lowered to statements like a choice body.
    DoBlock {
        /// Lowered statements in source order.
        statements: Vec<Statement>,
        /// 1-based source position of the `do`.
        span: SrcPos,
    },
    /// `let ... in ...` expression.
    LetIn {
        /// Let bindings in source order.
        bindings: Vec<LetBinding>,
        /// Expression after `in`.
        body: Box<Self>,
        /// 1-based source position of the `let`.
        span: SrcPos,
    },
    /// Record construction or update: `Foo with x = 1`, `this with owner`.
    Record {
        /// Record constructor or update base expression.
        base: Box<Self>,
        /// Record fields in source order.
        fields: Vec<RecordField>,
        /// 1-based source position of the record expression.
        span: SrcPos,
    },
    /// Tuple expression.
    Tuple {
        /// Tuple items in source order.
        items: Vec<Self>,
        /// 1-based source position of the tuple.
        span: SrcPos,
    },
    /// List expression.
    List {
        /// List items in source order.
        items: Vec<Self>,
        /// 1-based source position of the list.
        span: SrcPos,
    },
    /// Anything without a structured encoding (operator sections,
    /// comprehension qualifiers, recovered parse errors). `raw` preserves
    /// the source text.
    Unknown {
        /// Raw source text for syntax without a structured IR encoding.
        raw: String,
        /// 1-based source position of the recovered expression.
        span: SrcPos,
    },
}

/// A single case alternative.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct CaseAlt {
    /// Pattern rendered to source text (`Some x`, `[]`, `_`).
    pub pattern: String,
    /// Alternative body expression.
    pub body: Expr,
}

/// A `let` binding inside an expression.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct LetBinding {
    /// Bound name or rendered pattern.
    pub name: String,
    /// Bound value expression.
    pub value: Expr,
}

/// Record field assignment or pun in a record expression.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct RecordField {
    /// Field name, or `..` for a spread/wildcard field.
    pub name: String,
    /// None for punned fields (`Foo with owner`) and `..` spreads.
    pub value: Option<Expr>,
}

/// Template, choice, or interface method field.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Field {
    /// Field or parameter name.
    pub name: String,
    /// Declared type, if present and successfully lowered.
    pub type_: Option<TypeNode>,
    /// Declaration location.
    pub span: Span,
}

/// Daml template declaration.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Template {
    /// Template name.
    pub name: String,
    /// Template fields in declaration order.
    pub fields: Vec<Field>,
    /// Signatory expressions in source order.
    pub signatory_exprs: Vec<Expr>,
    /// Observer expressions in source order.
    pub observer_exprs: Vec<Expr>,
    /// Optional `ensure` clause.
    pub ensure_clause: Option<EnsureClause>,
    /// `key <expr> : <Type>` — expression and structured type, if declared.
    pub key_expr: Option<Expr>,
    /// Template key type, when declared.
    pub key_type: Option<TypeNode>,
    /// Maintainer expressions in source order.
    pub maintainer_exprs: Vec<Expr>,
    /// Choices declared directly on the template.
    pub choices: Vec<Choice>,
    /// Interfaces this template implements (`interface instance I for T`).
    pub interface_instances: Vec<InterfaceInstance>,
    /// Template declaration location.
    pub span: Span,
}

/// `interface instance` declared for a template.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct InterfaceInstance {
    /// Implemented interface name.
    pub interface_name: String,
    /// Implemented method names, in declaration order.
    pub methods: Vec<String>,
    /// Interface instance declaration location.
    pub span: Span,
}

/// Template `ensure` clause.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct EnsureClause {
    /// Ensure predicate expression.
    pub expr: Expr,
    /// Ensure clause location.
    pub span: Span,
}

/// Template or interface choice declaration.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Choice {
    /// Choice name.
    pub name: String,
    /// Whether exercising the choice consumes the contract.
    pub consuming: Consuming,
    /// Controller expressions in source order.
    pub controller_exprs: Vec<Expr>,
    /// Choice observers, if declared.
    pub observer_exprs: Vec<Expr>,
    /// Choice authority expressions from `authority` metadata clauses.
    pub authority_exprs: Vec<Expr>,
    /// Choice parameters in declaration order.
    pub parameters: Vec<Field>,
    /// Return type, if declared and successfully lowered.
    pub return_type: Option<TypeNode>,
    /// Lowered choice body statements in source order.
    pub body: Vec<Statement>,
    /// Choice declaration location.
    pub span: Span,
}

/// Choice consuming mode.
///
/// Serialized with kebab-case tags (`consuming` or `non-consuming`) for custom
/// JavaScript rules.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Consuming {
    /// Exercising the choice consumes the contract.
    Consuming,
    /// Exercising the choice leaves the contract active.
    NonConsuming,
}

impl Consuming {
    /// Returns `true` for [`Consuming::Consuming`].
    #[must_use]
    pub const fn is_consuming(&self) -> bool {
        matches!(self, Self::Consuming)
    }
}

impl fmt::Display for Consuming {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Consuming => "consuming",
            Self::NonConsuming => "non-consuming",
        })
    }
}

/// Do-statement classification.
///
/// Structured payloads (`value`, `condition_expr`, `cid`, `argument`) are the
/// rule-facing parse tree.
/// `Other.raw` is the deliberate raw-source form for statements with no
/// structured encoding.
#[non_exhaustive]
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub enum Statement {
    /// `let` binding statement inside a `do` block.
    Let {
        /// Bound name or rendered pattern.
        name: String,
        /// Bound value.
        value: Expr,
        /// 1-based source position of the statement.
        span: SrcPos,
    },
    /// Assertion statement.
    Assert {
        /// Asserted condition expression.
        condition_expr: Expr,
        /// 1-based source position of the assertion.
        span: SrcPos,
    },
    /// `fetch` update statement.
    Fetch {
        /// Contract id expression.
        cid: Expr,
        /// Pattern bound by `x <- fetch cid`, if any.
        binder: Option<String>,
        /// 1-based source position of the fetch.
        span: SrcPos,
    },
    /// `archive` update statement.
    Archive {
        /// Contract id expression.
        cid: Expr,
        /// 1-based source position of the archive.
        span: SrcPos,
    },
    /// `create` update statement.
    Create {
        /// Created template name.
        template_name: String,
        /// The created payload (usually a Record expression).
        argument: Expr,
        /// Pattern bound by `cid <- create ...`, if any.
        binder: Option<String>,
        /// 1-based source position of the create.
        span: SrcPos,
    },
    /// `exercise` update statement.
    Exercise {
        /// Exercised choice name.
        choice_name: String,
        /// Contract id expression.
        cid: Expr,
        /// The choice argument (usually a Record expression), if present.
        argument: Option<Expr>,
        /// Pattern bound by `result <- exercise ...`, if any.
        binder: Option<String>,
        /// 1-based source position of the exercise.
        span: SrcPos,
    },
    /// `try`/`catch` statement with separate statement scopes.
    TryCatch {
        /// Statements in the try body.
        try_body: Vec<Self>,
        /// Statements in the catch body.
        catch_body: Vec<Self>,
        /// 1-based source position of the try/catch.
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
        /// `case` scrutinee expression, or `None` for `if` branches.
        scrutinee: Option<Expr>,
        /// Branch arms in source order.
        arms: Vec<BranchArm>,
        /// 1-based source position of the branch expression.
        span: SrcPos,
    },
    /// Statement with no more-specific structured encoding.
    Other {
        /// Raw source text for the statement.
        raw: String,
        /// Structured form of the statement expression.
        expr: Expr,
        /// Pattern bound by the statement, if any.
        binder: Option<String>,
        /// 1-based source position of the statement.
        span: SrcPos,
    },
}

/// One arm of a `Statement::Branch`. `pattern` is the rendered case alt pattern
/// (`x :: _`, `[a]`, `_`); None for the then/else arms of an `if`.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct BranchArm {
    /// Rendered case alternative pattern, or `None` for `if` then/else arms.
    pub pattern: Option<String>,
    /// Statements that run when this arm is selected.
    pub body: Vec<Statement>,
}

/// Top-level Daml function lowered for rule analysis.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Function {
    /// Function name.
    pub name: String,
    /// Declared type signature, if present.
    pub type_signature: Option<TypeNode>,
    /// Lowered function body statements in source order.
    pub body: Vec<Statement>,
    /// Function declaration location.
    pub span: Span,
}

/// Daml import declaration.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Import {
    /// Imported module name.
    pub module_name: String,
    /// Import qualification style.
    pub qualified: ImportStyle,
    /// Import alias from `as`, if present.
    pub alias: Option<String>,
    /// Import declaration location.
    pub span: Span,
}

/// Import qualification style.
///
/// Serialized with kebab-case tags (`qualified` or `unqualified`) for custom
/// JavaScript rules.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImportStyle {
    /// Qualified import.
    Qualified,
    /// Unqualified import.
    Unqualified,
}

impl ImportStyle {
    /// Returns `true` for [`ImportStyle::Qualified`].
    #[must_use]
    pub const fn is_qualified(&self) -> bool {
        matches!(self, Self::Qualified)
    }
}

impl fmt::Display for ImportStyle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Qualified => "qualified",
            Self::Unqualified => "unqualified",
        })
    }
}

/// Method signature declared by an interface.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct InterfaceMethod {
    /// Method name.
    pub name: String,
    /// Method type, if declared and successfully lowered.
    pub type_: Option<TypeNode>,
    /// Method declaration location.
    pub span: Span,
}

/// Daml interface declaration.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct Interface {
    /// Interface name.
    pub name: String,
    /// Interfaces this interface requires.
    pub requires: Vec<String>,
    /// View type name, if declared.
    pub viewtype: Option<String>,
    /// Interface methods in declaration order.
    pub methods: Vec<InterfaceMethod>,
    /// Choices declared on the interface.
    pub choices: Vec<Choice>,
    /// Interface declaration location.
    pub span: Span,
}

/// Lowered Daml module passed to detectors and custom JavaScript rules.
///
/// `ir_version` identifies this serialized rule-facing contract. `source`
/// preserves the original source text so UTF-16 spans can be used with
/// JavaScript string APIs.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[non_exhaustive]
pub struct DamlModule {
    /// Rule-facing IR schema version.
    pub ir_version: u32,
    /// Module name.
    pub name: String,
    /// Source file path used while parsing.
    pub file: PathBuf,
    /// Original source text.
    pub source: String,
    /// Imports in source order.
    pub imports: Vec<Import>,
    /// Templates in source order.
    pub templates: Vec<Template>,
    /// Interfaces in source order.
    pub interfaces: Vec<Interface>,
    /// Function definitions with bodies, in source order.
    pub functions: Vec<Function>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    // Pins private SourceSpan::from_text_range line/column mapping.
    #[test]
    fn source_span_line_calculation_tracks_its_source_file() {
        let source_a = SourceFile::parse("module A where\nabcde\n");
        let source_b = SourceFile::parse("module A where\nx\ncde\n");
        let range = TextRange::new(17.into(), 18.into());

        let span_a = SourceSpan::from_text_range(Path::new("A.daml"), &source_a, range);
        let span_b = SourceSpan::from_text_range(Path::new("B.daml"), &source_b, range);

        assert_eq!(span_a.line, LineNumber::new(2));
        assert_eq!(span_b.line, LineNumber::new(3));
        assert_ne!(span_a.column, span_b.column);
        assert_eq!(span_a.column, CharColumn::new(3));
        assert_eq!(span_b.column, CharColumn::new(1));
    }
}
