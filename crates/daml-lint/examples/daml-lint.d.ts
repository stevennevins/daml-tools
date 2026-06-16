// Type definitions for daml-lint custom rule scripts.
//
// Write rules in TypeScript against these types, compile to JavaScript
// (e.g. `npx esbuild my-rule.ts --outfile=my-rule.js`), and pass the .js
// file to daml-lint --rules. Node shapes mirror src/ir.rs.
//
// v2: nodes now carry a real expression AST (`Expr`). The raw-text fields
// of v1 (`body_raw`, `raw_text`, statement `expr`/`condition`/`raw`) are
// kept for compatibility but deprecated; prefer the structured payloads.

/** Span of a declaration-level node (template, choice, field, ...). */
interface Span {
  file: string;
  line: number;
  column: number;
}

/** Position of an expression-level node. 1-based; the file is the
 *  enclosing module's. */
interface SrcPos {
  line: number;
  column: number;
}

/** DAML types as parsed by daml-lint.
 *
 *  Builtin scalar types serialize as bare strings: "Party", "Text",
 *  "Decimal", "Int", "Bool", "Date", "Time", "Unit" (for `()`), and
 *  "Unknown" (anything carrying no money/collection meaning — tuples,
 *  function types `a -> b`, and bare type variables).
 *
 *  Parameterized types are single-key objects whose value is the inner
 *  type: { List: "Text" }, { Optional: "Party" }, { TextMap: "Int" },
 *  { ContractId: { Named: "Iou" } }.
 *
 *  User-defined / unrecognized capitalized types are { Named: "..." }
 *  where the payload is the (possibly qualified) constructor NAME — e.g.
 *  { Named: "Iou" } or { Named: "Lib.Mod.Asset" }. Application arguments
 *  are not part of the name: `Foo Bar` is { Named: "Foo" }. Recognized
 *  collections are classified, not Named, regardless of how they are
 *  qualified: `Map.Map Party Decimal` is { Map: [...] }, not Named. */
type DamlType =
  | "Party"
  | "Text"
  | "Decimal"
  | "Int"
  | "Bool"
  | "Date"
  | "Time"
  | "Unit"
  | "Unknown"
  | { ContractId: DamlType }
  | { List: DamlType }
  | { Optional: DamlType }
  | { TextMap: DamlType }
  | { Named: string };

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
type Expr =
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

interface CaseAlt {
  /** Pattern rendered to source text: "Some x", "[]", "_". */
  pattern: string;
  body: Expr;
}

interface LetBinding {
  /** Bound name; for function bindings includes parameters ("go x"). */
  name: string;
  value: Expr;
}

interface RecordField {
  name: string;
  /** null for punned fields (`Iou with owner`) and `..` spreads. */
  value: Expr | null;
}

interface Field {
  name: string;
  type_: DamlType;
  span: Span;
}

interface EnsureClause {
  /** @deprecated prefer `expr`. "ensure " + rendered condition. */
  raw_text: string;
  /** Structured ensure condition. */
  expr: Expr;
  span: Span;
}

/** Statements are single-key objects tagged by kind. Use the tag as a
 *  discriminant: `if ("Create" in stmt) { stmt.Create.template_name ... }`.
 *
 *  The per-field `@deprecated` raw-text payloads (`Let.expr`,
 *  `Assert.condition`, the `cid_expr`s, `Create`/`Exercise` `raw`) have
 *  structured replacements (`value`, `condition_expr`, `cid`, `argument`)
 *  that carry the parse tree. `Other.raw` is NOT deprecated — it is the
 *  deliberate raw-source form for statements with no structured encoding.
 *  `binder` is the pattern text bound by `x <- ...`, when
 *  present. Ledger actions under `$`/lambdas are surfaced as their own
 *  statements, in source order. An `if`/`case` is surfaced as a `Branch`
 *  whose `arms` are each their own statement scope (exactly one runs), so a
 *  rule must descend into `arms` to see effects inside a branch. */
type Statement =
  | {
      Let: {
        name: string;
        /** @deprecated prefer `value` (structured `Expr`). */
        expr: string;
        value: Expr;
        span: SrcPos;
      };
    }
  | {
      Assert: {
        /** @deprecated whole `assert`/`assertMsg` call text; prefer
         *  `condition_expr` (the structured condition only — drops the
         *  `assertMsg` message). */
        condition: string;
        condition_expr: Expr;
        span: SrcPos;
      };
    }
  | {
      Fetch: {
        /** @deprecated prefer `cid` (structured `Expr`). */
        cid_expr: string;
        cid: Expr;
        binder: string | null;
        span: SrcPos;
      };
    }
  | {
      Archive: {
        /** @deprecated prefer `cid` (structured `Expr`). */
        cid_expr: string;
        cid: Expr;
        span: SrcPos;
      };
    }
  | {
      Create: {
        template_name: string;
        /** @deprecated whole `create ...` call text; reconstruct from
         *  `template_name` + `argument` (which is only the payload record). */
        raw: string;
        /** The created payload, usually a Record expression. */
        argument: Expr;
        binder: string | null;
        span: SrcPos;
      };
    }
  | {
      Exercise: {
        /** @deprecated prefer `cid` (structured `Expr`). */
        cid_expr: string;
        choice_name: string;
        /** @deprecated whole `exercise ...` call text; reconstruct from
         *  `cid` + `choice_name` + `argument` (only the choice argument). */
        raw: string;
        cid: Expr;
        /** The choice argument (usually a Record expression), if present. */
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
interface BranchArm {
  pattern: string | null;
  body: Statement[];
}

interface Choice {
  name: string;
  consuming: boolean;
  /** Controller expressions rendered to text (list literals flattened). */
  controllers: string[];
  /** Structured controller expressions. */
  controller_exprs: Expr[];
  /** Choice observers, if declared. */
  observer_exprs: Expr[];
  parameters: Field[];
  return_type: DamlType;
  body: Statement[];
  /** @deprecated original source lines of the choice body; prefer `body`
   *  (structured statements). For verbatim line text/comments use `m.source`. */
  body_raw: string;
  span: Span;
}

interface InterfaceInstance {
  interface_name: string;
  /** Implemented method names, in declaration order. */
  methods: string[];
  span: Span;
}

interface Template {
  name: string;
  fields: Field[];
  /** Signatory expressions rendered to text (list literals flattened). */
  signatories: string[];
  observers: string[];
  /** Structured signatory/observer expressions. */
  signatory_exprs: Expr[];
  observer_exprs: Expr[];
  ensure_clause: EnsureClause | null;
  /** `key <expr> : <Type>`, if declared. */
  key_expr: Expr | null;
  key_type: string | null;
  maintainer_exprs: Expr[];
  choices: Choice[];
  /** Interfaces this template implements. */
  interface_instances: InterfaceInstance[];
  span: Span;
}

interface InterfaceMethod {
  name: string;
  type_text: string;
  span: Span;
}

/** A DAML interface declaration. Visited via on_interface. */
interface DamlInterface {
  name: string;
  /** Interfaces this interface requires (`requires Lockable.I`). */
  requires: string[];
  viewtype: string | null;
  methods: InterfaceMethod[];
  choices: Choice[];
  span: Span;
}

interface DamlFunction {
  name: string;
  /** Declared type signature text, if present. */
  type_signature: string | null;
  body: Statement[];
  /** @deprecated original source lines of the function; prefer `body`
   *  (structured statements). For verbatim line text/comments use `m.source`. */
  body_raw: string;
  span: Span;
}

interface Import {
  module_name: string;
  qualified: boolean;
  alias: string | null;
  span: Span;
}

interface DamlModule {
  name: string;
  file: string;
  imports: Import[];
  templates: Template[];
  interfaces: DamlInterface[];
  functions: DamlFunction[];
  source: string;
}

/** Report a finding at a node's span, or at an explicit 1-based line number. */
declare function report(
  node: { span: Span } | { span: SrcPos } | number,
  message: string
): void;
