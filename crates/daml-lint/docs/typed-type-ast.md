# Design: AST-based types (replace the `type_text` reparse)

Status: **PLANNED — design only.** No parser code changed yet. This is the
written plan for the one deferred item from the branch review; the three other
follow-ups (workspace lints, `rust_2018_idioms`, daml-lint feature-gating) have
landed.

## Problem

A Daml type is currently parsed **twice**:

1. `daml-parser` parses the source and stores each type as
   `FieldDecl.type_text: String` — a source-ish *rendering* of the type
   (`crates/daml-parser/src/ast.rs:259`).
2. `daml-lint` re-parses that string back into structure with
   `DamlType::from_str` (`crates/daml-lint/src/ir.rs:34`), a hand-written string
   matcher (`strip_prefix("Optional ")`, `strip_prefix("ContractId ")`,
   "starts-uppercase ⇒ `Named(whole string)`", else `Unknown`).

Every "harden the money-type / substring / token" fix has been patching step 2.
That is the wrong layer: step 2 is a second parser working from a lossy string,
and a string matcher **structurally cannot** tell type application from a
function arrow from an atomic constructor.

### Evidence (measured over the 924-file corpus)

A throwaway probe ran `DamlType::from_str` over every field / choice-param /
choice-return / interface-method / function-signature type string in the corpus:

| bucket            | count | distinct | notes |
|-------------------|-------|----------|-------|
| total type strings| 4719  | —        | |
| concrete builtin  | ~2753 | —        | `Party`/`Text`/`Decimal`/`Int`/`List`/`Optional`/`ContractId …` — classified correctly |
| `Named(_)`        | 1567  | 586      | **polluted** (see below) |
| `Unknown`         | 399   | 218      | mostly function signatures + tuples |

The `Named` bucket is where the string heuristic visibly breaks. Real corpus
strings that `from_str` collapses into one opaque `Named`:

- `"Script ()"` ×147 — a type **application** (`Script` applied to `()`),
  flattened to `Named("Script ()")`.
- `"Int -> Int"` ×15, `"Party -> Script ()"` ×9, `"Decimal -> Decimal"` ×7,
  `"NumericScale n => Numeric 37 -> Numeric n"` ×9 — **function** types, also
  swallowed into `Named`.

The `Unknown` bucket also exposes the *upstream* render being lossy, not just
step 2: a handful of strings are `"(Int, Text) = (1, \"a\")"` — `type_text`
captured the binding's `= value` RHS, so even a perfect step-2 parser gets junk
input.

### Why the detectors mostly survive today

The shipped detectors (`ensure_decimal`, `positive_amount`, `unbounded_fields`,
`head_of_list`, `archive_before_execute`) only deeply classify **field** and
**choice-param** types, which skew heavily to the concrete-builtin bucket
(`Decimal`, `[T]`, `ContractId T`, `Optional T`). The mis-bucketed strings are
overwhelmingly function signatures and `Script ()`/`Test` return types, which no
detector reasons about by type. So this is **latent**, not actively producing
false findings — but any new detector that must reason about an *applied* or
*function* type (e.g. "this choice returns `ContractId X`", "this field is a
function") cannot trust the current model.

There is also a structural coverage gap beyond `from_str`: `data` / record
declarations are stored as `Decl::TypeDef { keyword, name, .. }`
(`crates/daml-parser/src/ast.rs:406`) — **the record fields are not modeled at
all.** A `data Asset = Asset with amount : Decimal` is invisible to every
detector. (Out of scope for the first cut; see below.)

## Hard constraint

`daml-parser`'s AST is **shared with `daml-fmt`**, which is **924/924
desugar-equivalent and idempotent**. The formatter slices node content from
byte spans and gates every edit on token-stream equality
(`crates/daml-fmt/CLAUDE.md`). So we must **not** remove or repurpose
`type_text`, and we must not perturb existing spans.

Safe shape: **add** a structured `ty: Option<Type>` next to `type_text`. Keep
`type_text` exactly as the source slice the formatter and the lossless oracle
rely on; the new `Type` becomes the analysis truth for `daml-lint`. Net effect
for the formatter: one new ignored field, zero span change.

## Proposed `Type` AST (in `daml-parser`)

Scoped to the forms the corpus actually contains:

```rust
pub enum Type {
    Con { qualifier: Option<String>, name: String }, // IouCid, DA.Map.Map
    App(Box<Type>, Vec<Type>),                        // ContractId Foo, Script (), Numeric 10
    List(Box<Type>),                                  // [T]
    Tuple(Vec<Type>),                                 // (a, b, ...)
    Fun(Box<Type>, Box<Type>),                        // a -> b
    Var(String),                                       // lowercase type variable
    Unit,                                              // ()
    Constrained(Box<Type>),                            // C a => T  — keep body, drop context
}
```

`Numeric 10` is `App(Con "Numeric", [literal-or-var])`; the existing
`type_text` already carries the literal, so the type parser only needs to reach
the head `Con`. Constraints (`=> T`) keep the body and discard the context — no
detector reasons about constraints.

## Map `Type` → `DamlType`, then delete `from_str`

`DamlType` (`ir.rs:11`) stays as the coarse rule-facing classification; only its
*source* changes from a string to the structured `Type`:

- `App(Con "ContractId", [t])` → `ContractId(map t)`
- `App(Con "Optional", [t])` → `Optional(map t)`
- `List(t)` → `List(map t)`
- `App(Con "Numeric", _)` / `Con "Decimal"` → `Decimal`
- `App(Con("Map"|"GenMap"|"TextMap"|…), …)` / `App(Con "Set", _)` → the
  unbounded-collection mapping the money/`unbounded_fields` detectors use
- `Con { name, .. }` with no args → `Named(name)`
- `Fun`/`Tuple`/`Var` → the existing coarse fallbacks (detectors already ignore
  these) — but now they are *known to be* arrows/tuples/vars, not mistaken for
  `Named`.

`DamlType::from_str` and its string helpers (`strip_grouping_parens`,
`split_top_level_ws`, `has_top_level_comma`) are then **deleted**, along with the
re-parse call sites in `crates/daml-lint/src/parser.rs` (lines ~226, ~341,
~374). The `from_str` unit tests become `Type`-construction tests.

## Migration plan (each step TDD, each gated)

1. **Land the `Type` parser additively.** Add `ty: Option<Type>` to the
   type-bearing nodes (`FieldDecl`, `ChoiceDecl` return, `Key`); parse types
   into it. `type_text` unchanged. Gate: full `cargo test` + clippy +
   `node test/diff.js` **924/924** (proves the formatter is untouched). `ty`
   not yet consumed.
2. **Prove one detector.** Switch `ensure_decimal` to read `ty` (fall back to
   `from_str` while both exist). Add a corpus test asserting the `ty` path gives
   identical findings to the string path on real templates. Gate: full gauntlet.
3. **Migrate the rest** (`unbounded_fields`, `positive_amount`,
   `head_of_list`, choice-return uses) one at a time, each its own TDD step +
   gauntlet.
4. **Delete `from_str`** and the string helpers once nothing calls them. Run the
   adversarial-accuracy audit workflow (`.claude/workflows/`) to confirm no
   regression / no new false positives, since structuring previously-`Unknown`
   types can surface new findings.

## Out of scope for the first cut

- **Modelling `data`/record fields** (`Decl::TypeDef` → structured fields). This
  is the larger "AST-based from the start" follow-up: it changes how `data`
  parses and touches `daml-fmt` span coverage. Worth doing, but separately and
  after the type-AST swap is proven.
- **Function-type / tuple internals.** Detectors don't need them decomposed; the
  `Type` AST models them (so they stop being mistaken for `Named`) but nothing
  consumes the structure yet.
- **Richer `ParseDiagnostic`** and **formatter construct coverage** — unrelated
  to the type model; tracked elsewhere.

## Risks

- Structuring types that previously fell to `Unknown` may make a detector that
  silently ignored `Unknown` start firing. Mitigation: step 4's audit pass + the
  per-detector parity tests in steps 2–3.
- A bug in the new type parser could perturb a span and break the 924
  differential. Mitigation: `ty` is additive and span-free in step 1; the
  differential is a hard gate on every step.
- `type_text` already captures junk in a few cases (`= value` RHS). The new
  parser should parse from the real token stream, not from `type_text`, so it
  does **not** inherit that lossiness — and may motivate fixing the upstream
  render separately.

## Verification (every step)

`cargo fmt --all --check` · `cargo clippy --workspace --all-targets
--all-features --locked` · `cargo test --workspace --all-features --locked` ·
`cd crates/daml-fmt && node test/diff.js` (must stay **924/924**).
