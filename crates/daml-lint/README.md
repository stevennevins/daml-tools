# daml-lint

[![CI](https://github.com/stevennevins/daml-tools/actions/workflows/ci.yml/badge.svg)](https://github.com/stevennevins/daml-tools/actions/workflows/ci.yml)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)

> [!WARNING]
> This software is experimental and not intended for production use. Use at your own risk.

Static analysis scanner for [Daml](https://www.digitalasset.com/developers) smart contracts. Catches security vulnerabilities and anti-patterns through AST pattern matching, similar to what [Slither](https://github.com/crytic/slither) does for Solidity.

Part of the [daml-tools](https://github.com/stevennevins/daml-tools) workspace.
Parsing is the shared [`daml-parser`](https://crates.io/crates/daml-parser) crate — lexer (comments,
strings, layout-aware spans) → Haskell offside-rule layout resolution →
recursive-descent parser producing a typed AST with positions on every node.
daml-lint lowers that AST to a rule-facing IR and runs detectors over it. Files
that fail to parse degrade to partial structure with a diagnostic on stderr
(`file:line:col`); a scan never aborts on bad input.

## Documentation

The workspace docs split task guides, reference, and design background:

- [Scan Daml source](https://github.com/stevennevins/daml-tools/blob/main/docs/how-to/scan-daml.md) for CLI usage patterns
- [Write a custom rule](https://github.com/stevennevins/daml-tools/blob/main/docs/tutorials/write-a-daml-lint-custom-rule.md)
  for a guided first external rule
- [Custom rule contract](https://github.com/stevennevins/daml-tools/blob/main/docs/reference/daml-lint-custom-rule-contract.md)
  for the JavaScript runtime contract and TypeScript types
- [CLI reference](https://github.com/stevennevins/daml-tools/blob/main/docs/reference/cli.md) for options, output formats, and
  exit codes
- [Crate reference](https://github.com/stevennevins/daml-tools/blob/main/docs/reference/crates.md) for features and public
  modules
- [Rule authoring model](https://github.com/stevennevins/daml-tools/blob/main/docs/explanation/daml-lint-rule-authoring.md)
  for why TypeScript authoring is bundled to JavaScript
- [Workspace architecture](https://github.com/stevennevins/daml-tools/blob/main/docs/explanation/workspace-architecture.md)
  for how `daml-lint` uses `daml-parser`

## Detectors

| Detector | Severity | Description |
|----------|----------|-------------|
| `missing-ensure-decimal` | HIGH | Template has Decimal fields without an `ensure` clause bounding them to > 0 |
| `unguarded-division` | HIGH | Division operation without a prior guard checking the denominator is non-zero |
| `missing-positive-amount` | HIGH | Choice accepts amount/quantity/price parameter without asserting it is positive |
| `archive-before-execute` | HIGH | Contract archived before a `try/catch` block — contract is lost if execution fails |
| `head-of-list-query` | MEDIUM | Pattern match on head of `queryFilter` result — non-deterministic ordering risk |
| `unbounded-fields` | MEDIUM | Text, List, or TextMap fields without size bounds in the `ensure` clause |

## Installation

For JavaScript/TypeScript projects that want `daml-lint` as a dev dependency:

```sh
npm install --save-dev @daml-tools/daml-lint
npx daml-lint ./daml
```

Cargo installs require [Rust](https://rustup.rs/) 1.87+ (the `rquickjs`
dependency needs rustc 1.87).

```sh
cargo install daml-lint
```

Or straight from the workspace repo:

```sh
cargo install --git https://github.com/stevennevins/daml-tools daml-lint
```

Or from a local checkout:

```sh
git clone https://github.com/stevennevins/daml-tools.git
cd daml-tools
cargo install --path crates/daml-lint
```

## Library features

The default features build the published CLI and custom-rule engine:

```toml
[dependencies]
daml-lint = "0.8"
```

Library consumers that only need parser lowering and the rule-facing IR can
avoid the CLI parser and QuickJS runtime:

```toml
[dependencies]
daml-lint = { version = "0.8", default-features = false }
```

Rust-facing finding locations, parser diagnostics, and IR spans use the
coordinate newtypes from `daml-syntax` (`LineNumber`, `CharColumn`,
`Utf16Offset`, and `ByteOffset`) so byte, UTF-16, line, and column coordinates
cannot be mixed accidentally. JSON, SARIF, and custom-rule JavaScript output
still serialize those coordinates as numbers.

The `js-runtime` feature enables the QuickJS-backed runtime used by shipped
built-ins. The `custom-rules` feature implies `js-runtime` and enables loading
user-provided rule files through `--rules` and configured plugin packages.
Shipped built-ins are authored in TypeScript and embedded as generated JavaScript; no TypeScript
toolchain is required at runtime. The shipped detectors are registered through
`create_builtin_detectors()` rather than exposed as individual Rust detector
modules. The `cli` feature enables the `daml-lint` binary and implies `js-runtime`.

## Usage

Scan a single file:

```sh
daml-lint src/MyContract.daml
```

Scan a directory recursively:

```sh
daml-lint ./daml/
```

Choose an output format:

```sh
daml-lint ./daml/ --format sarif    # SARIF JSON (GitHub / IDE integration)
daml-lint ./daml/ --format markdown # Human-readable (default)
daml-lint ./daml/ --format json     # Machine-readable JSON
daml-lint ./daml/ --rule missing-ensure-decimal # run one built-in rule
daml-lint ./daml/ --group recommended           # run a rule group
```

Write results to a file:

```sh
daml-lint ./daml/ --format sarif --output report.sarif
```

### Custom detectors

Define your own detectors as AST rule scripts and pass them with `--rules`
(repeatable), in the style of [solhint custom rules](https://github.com/protofire/solhint/blob/master/docs/writing-plugins.md):

```sh
daml-lint ./daml/ --rules my-rule.js --rules another-rule.js
```

Installed plugin packages can also be enabled from `./daml.yaml`:

```yaml
daml-tools:
  lint:
    plugins: [template]
    plugin-paths: [./plugins]
    rules:
      missing-ensure-decimal: off
      template/template-requires-ensure:
        - warning
        - allowEmptyEnsure: false
```

`template` resolves to `daml-lint-plugin-template` in `node_modules`; use
`plugin-paths` for local package roots during development. Rule options from the
array form are exposed to the rule as global `CONFIG`.

A rule is TypeScript/JavaScript (executed by an embedded QuickJS engine):
constants for metadata, plus visitor functions named after the node types you
care about — like solhint's `ContractDefinition(node)` callbacks. Write rules
in TypeScript against [examples/daml-lint.d.ts](examples/daml-lint.d.ts) for
type checking and autocomplete. The `globalThis.__daml_lint_rule` assignment is
the TypeScript-checked rule object; the current runtime still discovers
top-level metadata constants and visitor `function` declarations:

```typescript
import type { Template } from "./daml-lint";

const NAME = "template-requires-ensure";
const SEVERITY = "medium";
const DESCRIPTION = "Every template must declare an ensure clause";   // optional

function on_template(template: Template): void {
  if (template.ensure_clause === null) {
    report(template, `Template '${template.name}' has no ensure clause`);
  }
}

globalThis.__daml_lint_rule = { NAME, SEVERITY, DESCRIPTION, on_template };
```

then compile to the JavaScript file you pass to `--rules`:

```sh
npx esbuild my-rule.ts --bundle --outfile=dist/my-rule.js
```

Type-only imports are erased by the build. Runtime helper imports must be
bundled because the rule engine runs JavaScript without `import`, `require`,
filesystem, or network APIs. Plain JavaScript rules work directly — the compile
step is only for TypeScript.

Visitors (define any subset, at least one):

| Function | Called for | Node fields |
|---|---|---|
| `on_template(template)` | each template | `name`, `fields`, `signatory_exprs`, `observer_exprs`, `ensure_clause` (`null` if absent), `key_expr`, `key_type`, `maintainer_exprs`, `choices`, `interface_instances`, `span` |
| `on_choice(choice, template)` | each choice | `name`, `consuming`, `controller_exprs`, `observer_exprs`, `parameters`, `return_type`, `body`, `span` |
| `on_field(field, template)` | each template field | `name`, `type_`, `span` |
| `on_function(function)` | each top-level function | `name`, `type_signature`, `body`, `span` |
| `on_import(import)` | each import | `module_name`, `qualified`, `alias` |
| `on_interface(interface)` | each interface | `name`, `requires`, `viewtype`, `methods`, `choices`, `span` |
| `check(m)` | once per module | `ir_version`, `name`, `file`, `imports`, `templates`, `interfaces`, `functions`, `source` |

Report findings with `report(node, message)` (location taken from the node's
`span`) or `report(line, message)`. Pass `report(node, message, evidence)` when
the report should show structural evidence instead of the source line. The
rule's `SEVERITY` applies to all its findings. Node shapes are declared in
[examples/daml-lint.d.ts](examples/daml-lint.d.ts) and mirror the IR in
[src/ir.rs](src/ir.rs); statement nodes in `body` are objects keyed by kind,
e.g. `"Create" in stmt`.

Choice consumption and import forms are surfaced as string enums (`choice.consuming`
is `"consuming" | "non-consuming"`; `import.qualified` is `"qualified" | "unqualified"`)
to avoid boolean ambiguity.

Statements carry a typed expression AST: `stmt.Let.value`,
`stmt.Assert.condition_expr`, `stmt.Exercise.cid`/`.argument`, and
`stmt.Other.expr` are `Expr` nodes — tagged unions like
`{ BinOp: { op: "/", lhs, rhs, span } }` with a 1-based `span` on every
node (see the `Expr` type in the .d.ts). Type-bearing fields carry `TypeNode`
trees such as `{ Con: { name: "Party", qualifier: null, span } }`,
`{ App: { head, args, span } }`, and `{ Lit: { kind: "Text", value: "cid", span } }`
for type-level literals (for example `HasField "cid"`); type spans include `line`/`column`,
JavaScript string offsets (`start`/`end`, suitable for
`m.source.slice(start, end)`), and parser byte offsets
(`byte_start`/`byte_end`). Compatibility-only raw-text fields and rendered
party-name lists were removed in the breaking custom-rule surface, so rules
should match on structure, not substrings.
[examples/unguarded-division-ast.ts](examples/unguarded-division-ast.ts)
shows a denominator-guard check written entirely on typed nodes.

Removed v1/v2 compatibility fields and their structured replacements:

| Removed field | Use instead | Notes |
|---|---|---|
| `choice.body_raw`, `function.body_raw` | `body` (`Statement[]`) | Match statements structurally; only `stmt.Other.raw` / `Expr.Unknown.raw` preserve unsupported source text. |
| `template.ensure_clause.raw_text` | `ensure_clause.expr` (`Expr`) | Match the condition structurally. |
| `stmt.Let.expr` | `stmt.Let.value` (`Expr`) | The bound expression. |
| `stmt.Assert.condition` | `stmt.Assert.condition_expr` (`Expr`) | The condition expression only. |
| `stmt.Fetch.cid_expr`, `stmt.Archive.cid_expr`, `stmt.Exercise.cid_expr` | `.cid` (`Expr`) | The contract-id expression. |
| `stmt.Create.raw` | `template_name` + `argument` (`Expr`) | `argument` is the created payload. |
| `stmt.Exercise.raw` | `cid` + `choice_name` + `argument` (`Expr`) | `argument` is the choice argument, if present. |
| `choice.controllers` | `controller_exprs` (`Expr[]`) | Flatten list expressions in the rule if you want list-literal party semantics. |
| `choice.authorities` | `authority_exprs` (`Expr[]`) | Source-level `authority` metadata clauses on choices. |
| `template.signatories`, `template.observers` | `signatory_exprs`, `observer_exprs` (`Expr[]`) | Structured party expressions only. |

`stmt.Other.raw` and the `Unknown` expression's `raw` are deliberate raw-source
escape hatches for constructs with no structured form (e.g.
[examples/no-trace.ts](examples/no-trace.ts) matches source text).

Heads up: visitors must be `function` declarations — arrow functions assigned
to `const` are not discovered. If a script fails at runtime, the CLI exits 2;
library callers can use `Detector::try_detect` to receive the rule error
without terminating the host process. `DetectError` preserves the underlying
`ScriptLoadError` through `std::error::Error::source()` when one is available,
so library callers can inspect the typed failure chain instead of parsing
strings. Rule errors are never swallowed. A runaway loop is interrupted so a
broken rule can't hang CI. The engine runs JavaScript
(ES2023) — no Node APIs, no `require`/`import`, no filesystem or network.
Each rule's script is evaluated once and its visitors are then called for
every module — visitors should be stateless; don't accumulate findings in
top-level mutable state across files.

`SEVERITY` is one of `critical`, `high`, `medium`, `low`, `info`. Config can
also use `error` (high) and `warning` (medium). Custom rules
run alongside the built-in detectors, appear in all output formats, and count
toward `--fail-on`. Direct `--rules` names must not collide with built-in
detector names or each other. Installed plugin rules are reported under their
configured `plugin/rule` ID.

Examples:

- [examples/template-requires-ensure.ts](examples/template-requires-ensure.ts) — structural check on a single node
- [examples/consuming-choice-signatory-controller.ts](examples/consuming-choice-signatory-controller.ts) — cross-references choice controllers against template signatories
- [examples/no-create-in-nonconsuming.ts](examples/no-create-in-nonconsuming.ts) — walks choice body statements, recursing into try/catch
- [examples/no-trace.ts](examples/no-trace.ts) — banned-token check over raw source lines
- [examples/unguarded-division-ast.ts](examples/unguarded-division-ast.ts) — expression-level analysis on the typed AST (division denominators vs prior assertions)

Each example is authored in TypeScript and ships with its compiled `.js` under
[examples/dist/](examples/dist/) - that's the file `--rules` takes. Run
`npm run build:examples` from this crate to refresh those generated files.

To check that a rule script parses without running a scan, point the tool at a nonexistent path — rule errors are reported before file discovery. (A valid script then prints `No .daml files found.`, which also exits 2 — go by the message, not the exit code.)

Library callers can load custom rules without writing temporary files:
`detectors::script::load_script_source(label, source)` accepts in-memory
JavaScript, and `load_script_reader_with_options(label, reader, options)`
accepts any `std::io::Read` source plus JSON rule `CONFIG`.

### CI gating

Use `--fail-on` to control when the tool returns a non-zero exit code:

```sh
daml-lint ./daml/ --fail-on medium   # fail on medium or above
daml-lint ./daml/ --fail-on critical # fail only on critical
```

## Output Formats

- **SARIF** — Standard format for static analysis tools. Integrates with GitHub Code Scanning and IDEs.
- **Markdown** — Human-readable report grouped by severity. Good for pull request comments.
- **JSON** — Flat findings array with summary counts. Good for dashboards and aggregation.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | No findings at or above the `--fail-on` threshold |
| 1 | One or more findings at or above the threshold |
| 2 | CLI error (invalid format, no files found, etc.) |
| 3 | A scanned file had parse errors (scan is not authoritative) |

## Development

```sh
cargo test
```

Tests run entirely offline: parser and layout integration tests use a vendored
copy of the [daml-finance](https://github.com/digital-asset/daml-finance)
sources under [corpus/daml-finance/](https://github.com/stevennevins/daml-tools/tree/main/corpus/daml-finance) (634 real
`.daml` files) — shared at the workspace root with `daml-parser` — as a
ground-truth corpus; see
[corpus/daml-finance/README.md](https://github.com/stevennevins/daml-tools/blob/main/corpus/daml-finance/README.md) for
provenance and licensing.

## Public API Stability

`daml-lint` is pre-1.0. The CLI exit codes and documented feature flags are the
stable user contract for the current 0.8 line. The rule-facing IR is
intentionally public for custom rules and library users, but it may gain
structure in 0.x minor releases;
custom rules should check `ir_version` and match typed nodes rather than raw
source substrings. Detector result types such as `Finding`, `Severity`, and
`DetectError` are non-exhaustive; use their documented fields/accessors and keep
wildcard arms when matching enums. Patch releases should remain compatible.

Breaking updates introduced in this branch:

- `Severity` no longer implements `Ord`/`PartialOrd`; use `rank()` or
  `meets_or_exceeds()` for risk-based ordering and thresholds.
- `Severity::from_str` now returns `SeverityParseError` instead of `()`.
- `parse_severity` was removed; use `value.parse::<Severity>()` so invalid
  input preserves `SeverityParseError`.
- Public IR/report DTO structs are `#[non_exhaustive]`; construct through
  parser lowering or documented constructors such as `Finding::new`.
- `parse_daml_with_diagnostics` now returns a named `ParseResult` with fields
  (`module`, `diagnostics`) instead of a tuple.
- Rule setting values are now canonical only: `off`, `critical`, `high`,
  `medium`, `low`, `info` (legacy `warn`/`error` and numeric shortcuts
  `0`/`1`/`2` are intentionally rejected).

## License

AGPL-3.0-only. See [LICENSE](LICENSE).
