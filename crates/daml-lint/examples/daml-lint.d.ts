// TypeScript contract for daml-lint custom rule authoring.
//
// Examples and built-in rules use these types while authoring TypeScript.
// Bundle the rule to JavaScript before passing it to daml-lint --rules.
// Node shapes mirror src/ir.rs.
//
// v4: nodes carry structured expression and type ASTs. Compatibility-only raw
// fields and rendered party-name lists from v1/v2 have been removed.

/** Span of a declaration-level node (template, choice, field, ...). */
export interface Span {
  file: string;
  line: number;
  column: number;
}

/** Source range for typed parser nodes.
 *
 *  `start`/`end` are UTF-16 code-unit offsets into `module.source`, so
 *  `module.source.slice(span.start, span.end)` returns the exact source text.
 *  `byte_start`/`byte_end` preserve the parser's UTF-8 byte-span basis. */
export interface SourceSpan extends Span {
  start: number;
  end: number;
  byte_start: number;
  byte_end: number;
}

/** Position of an expression-level node. 1-based; the file is the
 *  enclosing module's. */
export interface SrcPos {
  line: number;
  column: number;
}

/** Choice consumption mode in the contract IR. */
export type ChoiceConsumption = "consuming" | "non-consuming";

/** Import style in the contract IR. */
export type ImportStyle = "qualified" | "unqualified";

/** Structured DAML type AST. Source spans support diagnostics and exact
 *  `module.source` slicing. Unknown/unparseable types are represented as null
 *  at the field that carries the type.
 *
 *  - Lit: type-level literals such as `HasField "cid"` field names; kind and
 *    value mirror expression literals. */
export type TypeNode =
  | { Con: { qualifier: string | null; name: string; span: SourceSpan } }
  | { App: { head: TypeNode; args: TypeNode[]; span: SourceSpan } }
  | { List: { inner: TypeNode; span: SourceSpan } }
  | { Tuple: { items: TypeNode[]; span: SourceSpan } }
  | { Fun: { param: TypeNode; result: TypeNode; span: SourceSpan } }
  | { Var: { name: string; span: SourceSpan } }
  | { Unit: { span: SourceSpan } }
  | { Constrained: { body: TypeNode; span: SourceSpan } }
  | { Lit: { kind: "Int" | "Decimal" | "Text" | "Char"; value: string; span: SourceSpan } };

/** Expression AST. Tagged unions: use the key as discriminant, e.g.
 *  `if ("BinOp" in e && e.BinOp.op === "/") { ... }`.
 *
 *  - Var: variable reference; qualifier is the module alias for qualified
 *    names (`Map.lookup` → { name: "lookup", qualifier: "Map" }).
 *  - Con: constructor / type name in expression position (`Some`, `Iou`).
 *  - Lit: kind is "Int" | "Decimal" | "Text" | "Char"; value is source text.
 *  - App: application, flattened (`f a b` has two args).
 *  - BinOp: op is source-level text (`+`, `/`, `&&`, "`div`" for backtick
 *    application, ".." for ranges).
 *  - DoBlock: nested do, lowered to statements like a choice body.
 *  - Record: construction (`Iou with amount = 1.0`) when base is Con,
 *    update (`this with owner = p`) otherwise. Punned fields and `..`
 *    spreads have value: null.
 *  - Unknown: no structured encoding (operator sections, comprehension
 *    qualifiers, recovered parse errors); raw preserves source text. */
export type Expr =
  | { Var: { name: string; qualifier: string | null; span: SrcPos } }
  | { Con: { name: string; qualifier: string | null; span: SrcPos } }
  | { Lit: { kind: "Int" | "Decimal" | "Text" | "Char"; value: string; span: SrcPos } }
  | { App: { func: Expr; args: Expr[]; span: SrcPos } }
  | { BinOp: { op: string; lhs: Expr; rhs: Expr; span: SrcPos } }
  | { Neg: { expr: Expr; span: SrcPos } }
  | { Lambda: { params: string[]; body: Expr; span: SrcPos } }
  | { If: { cond: Expr; then_branch: Expr; else_branch: Expr; span: SrcPos } }
  | { Case: { scrutinee: Expr; alts: CaseAlt[]; span: SrcPos } }
  | { DoBlock: { statements: Statement[]; span: SrcPos } }
  | { LetIn: { bindings: LetBinding[]; body: Expr; span: SrcPos } }
  | { Record: { base: Expr; fields: RecordField[]; span: SrcPos } }
  | { Tuple: { items: Expr[]; span: SrcPos } }
  | { List: { items: Expr[]; span: SrcPos } }
  | { Unknown: { raw: string; span: SrcPos } };

export interface CaseAlt {
  /** Pattern rendered to source text: "Some x", "[]", "_". */
  pattern: string;
  body: Expr;
}

export interface LetBinding {
  /** Bound name; for function bindings includes parameters ("go x"). */
  name: string;
  value: Expr;
}

export interface RecordField {
  name: string;
  /** null for punned fields (`Iou with owner`) and `..` spreads. */
  value: Expr | null;
}

export interface Field {
  name: string;
  type_: TypeNode | null;
  span: Span;
}

export interface EnsureClause {
  expr: Expr;
  span: Span;
}

/** Statements are single-key objects tagged by kind. Use the tag as a
 *  discriminant: `if ("Create" in stmt) { stmt.Create.template_name ... }`.
 *
 *  Structured payloads (`value`, `condition_expr`, `cid`, `argument`) carry
 *  the parse tree. `Other.raw` is the deliberate raw-source form for statements
 *  with no structured encoding.
 *  `binder` is the pattern text bound by `x <- ...`, when
 *  present. Ledger actions under `$`/lambdas are surfaced as their own
 *  statements, in source order. An `if`/`case` is surfaced as a `Branch`
 *  whose `arms` are each their own statement scope (exactly one runs), so a
 *  rule must descend into `arms` to see effects inside a branch. */
export type Statement =
  | {
      Let: {
        name: string;
        value: Expr;
        span: SrcPos;
      };
    }
  | {
      Assert: {
        condition_expr: Expr;
        span: SrcPos;
      };
    }
  | {
      Fetch: {
        cid: Expr;
        binder: string | null;
        span: SrcPos;
      };
    }
  | {
      Archive: {
        cid: Expr;
        span: SrcPos;
      };
    }
  | {
      Create: {
        template_name: string;
        argument: Expr;
        binder: string | null;
        span: SrcPos;
      };
    }
  | {
      Exercise: {
        choice_name: string;
        cid: Expr;
        argument: Expr | null;
        binder: string | null;
        span: SrcPos;
      };
    }
  | { TryCatch: { try_body: Statement[]; catch_body: Statement[]; span: SrcPos } }
  /** `if`/`case`: each arm is an independent statement scope (exactly one
   *  arm runs at runtime). `scrutinee` is the `case <e> of` expression (null
   *  for `if`); each arm carries the source pattern it matched (null for the
   *  `if` then/else arms). */
  | { Branch: { scrutinee: Expr | null; arms: BranchArm[]; span: SrcPos } }
  | { Other: { raw: string; expr: Expr; binder: string | null; span: SrcPos } };

/** One arm of a `Branch`: the matched case pattern (null for `if` arms) and the
 *  arm's own statement scope. */
export interface BranchArm {
  pattern: string | null;
  body: Statement[];
}

export interface Choice {
  name: string;
  consuming: ChoiceConsumption;
  controller_exprs: Expr[];
  /** Choice observers, if declared. */
  observer_exprs: Expr[];
  /** Choice authority expressions from `authority` metadata clauses. */
  authority_exprs: Expr[];
  parameters: Field[];
  return_type: TypeNode | null;
  body: Statement[];
  span: Span;
}

export interface InterfaceInstance {
  interface_name: string;
  /** Implemented method names, in declaration order. */
  methods: string[];
  span: Span;
}

export interface Template {
  name: string;
  fields: Field[];
  signatory_exprs: Expr[];
  observer_exprs: Expr[];
  ensure_clause: EnsureClause | null;
  /** `key <expr> : <Type>`, if declared. */
  key_expr: Expr | null;
  key_type: TypeNode | null;
  maintainer_exprs: Expr[];
  choices: Choice[];
  /** Interfaces this template implements. */
  interface_instances: InterfaceInstance[];
  span: Span;
}

export interface InterfaceMethod {
  name: string;
  type_: TypeNode | null;
  span: Span;
}

/** A DAML interface declaration. Visited via on_interface. */
export interface DamlInterface {
  name: string;
  /** Interfaces this interface requires (`requires Lockable.I`). */
  requires: string[];
  viewtype: string | null;
  methods: InterfaceMethod[];
  choices: Choice[];
  span: Span;
}

export interface DamlFunction {
  name: string;
  /** Declared type signature, if present. */
  type_signature: TypeNode | null;
  body: Statement[];
  span: Span;
}

export interface Import {
  module_name: string;
  qualified: ImportStyle;
  alias: string | null;
  span: Span;
}

export interface DamlModule {
  ir_version: 5;
  name: string;
  file: string;
  imports: Import[];
  templates: Template[];
  interfaces: DamlInterface[];
  functions: DamlFunction[];
  source: string;
}

export type RuleSeverity = "critical" | "high" | "medium" | "low" | "info";

export type RuleVisitorModule = {
  on_template: (template: Template) => void;
  on_choice: (choice: Choice, template: Template) => void;
  on_field: (field: Field, template: Template) => void;
  on_function: (fn: DamlFunction) => void;
  on_import: (imp: Import) => void;
  on_interface: (iface: DamlInterface) => void;
  check: (module: DamlModule) => void;
};

export type RuleVisitor = {
  [Name in keyof RuleVisitorModule]: Pick<RuleVisitorModule, Name> &
    Partial<Omit<RuleVisitorModule, Name>>;
}[keyof RuleVisitorModule];

export type RuleModule = {
  NAME: string;
  SEVERITY: RuleSeverity;
  DESCRIPTION?: string;
} & RuleVisitor;

export type ReportTarget = { span: Span } | { span: SrcPos } | number;

declare global {
  /** Per-rule options from `daml.yaml`. Defaults to `{}`. */
  var CONFIG: unknown;

  var __daml_lint_rule: RuleModule | undefined;

  /** Report a finding at a node's span, or at an explicit 1-based line number.
   *  `evidence`, when supplied, is used in reports instead of the source line. */
  function report(node: ReportTarget, message: string, evidence?: string): void;
}
