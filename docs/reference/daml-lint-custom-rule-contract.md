---
description: Stable metadata, visitor, configuration, reporting, and packaging contract for daml-lint custom rules.
---

# daml-lint custom rule contract

This page describes the custom rule interface loaded by `daml-lint --rules`
and by installed plugin packages configured in `./daml.yaml`.

## Runtime file

A custom rule file is JavaScript executed by the embedded QuickJS runtime. The
file must define:

| Item | Required | Shape |
|------|----------|-------|
| `const NAME` | Yes | Unqualified string rule name. For plugin packages, it must match the rule key in `package.json`. |
| `const SEVERITY` | Yes | `critical`, `high`, `medium`, `low`, or `info`. |
| `const DESCRIPTION` | No | String shown in rule metadata. |
| Visitor function | Yes | At least one supported top-level `function` declaration. |

Visitor functions are discovered by name on the evaluated script global object.
Arrow functions assigned to `const` are not discovered by the current runtime.

## Visitor hooks

| Function | Called for |
|----------|------------|
| `on_template(template)` | Each template. |
| `on_choice(choice, template)` | Each choice with its enclosing template. |
| `on_field(field, template)` | Each template field with its enclosing template. |
| `on_function(function)` | Each top-level function. |
| `on_import(import)` | Each import. |
| `on_interface(interface)` | Each interface. |
| `check(module)` | Once per module. |

Visitors may define any subset, but at least one must be present.

## TypeScript contract

Rule authors can import types from `@daml-tools/lint-plugin`:

```typescript
import type { DamlLintRuleModule, Template } from "@daml-tools/lint-plugin";
```

The package exports the rule-facing IR types, `DamlLintRuleSeverity`,
`DamlLintRuleModule`, `DamlLintReportTarget`, global `CONFIG`, global
`report`, and global `__daml_lint_rule`.

The crate-local examples import equivalent types from
[`crates/daml-lint/examples/daml-lint.d.ts`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-lint/examples/daml-lint.d.ts).

## `@daml-tools/lint-plugin` package contract

Install the npm package as a dev dependency alongside TypeScript and esbuild:

```sh
npm install --save-dev @daml-tools/daml-lint @daml-tools/lint-plugin typescript esbuild
```

The npm package ships [`dist/index.d.ts`](https://github.com/stevennevins/daml-tools/tree/main/crates/daml-lint/lint-plugin/dist),
[`templates/`](https://github.com/stevennevins/daml-tools/tree/main/crates/daml-lint/lint-plugin/templates),
and `README.md`. Only `dist/index.d.ts` is the runtime/type entry point: it
exports rule-facing IR types, `DamlLintRuleModule`, `DamlLintRuleSeverity`,
`DamlLintReportTarget`, and globals `CONFIG`, `report`, and
`__daml_lint_rule`. The templates are copy-paste scaffolding, not bundled
runtime helpers — bundle rule logic to one JavaScript file before scanning.

Starter templates are available under `@daml-tools/lint-plugin/templates/*` and
via the package `exports` map for `minimal-rule` and `project/*` files.

Scaffold a multi-rule plugin package with:

```sh
npx -y -p @daml-tools/lint-plugin create-daml-lint-plugin ledger-style
```

The command copies the `templates/project` starter, substitutes the plugin and
package names, and creates `src/rules/`, `dist/rules/`, `package.json`
`damlLint.rules`, and a local `daml.yaml` that enables every bundled rule under
one plugin namespace.

## Rule IDs

Built-in rules use unqualified ids such as `missing-ensure-decimal`. Plugin rules
use `plugin/rule` ids where `plugin` is the package name without the
`daml-lint-plugin-` prefix. The manifest rule key in `package.json` must match
the bundled script's `const NAME`; users enable the rule as
`template/template-requires-ensure` when the package is
`daml-lint-plugin-template`.

Custom `--rules` file names must not collide with built-in detector names or
each other. Installed plugin rules are always namespaced.

## Authoring object

`globalThis.__daml_lint_rule` is a TypeScript authoring object:

```typescript
const rule: DamlLintRuleModule = { NAME, SEVERITY, DESCRIPTION, on_template };
globalThis.__daml_lint_rule = rule;
```

This validates that metadata and visitors have the expected shape. The runtime
still reads top-level constants and visitor `function` declarations directly.

## Report function

Use `report` to emit findings:

```typescript
report(template, "Template has no ensure clause");
report(12, "Line-specific finding");
report(field, "Field is unbounded", "field : Text");
```

The first argument is a node with `span`, an expression node with `span`, or a
1-based line number. Explicit evidence replaces the source line shown in
reports.

### Span and reporting caveats

IR nodes expose byte-oriented `span` values from the parser. The runtime maps
those spans to line/column for Markdown and SARIF output. When you pass a line
number instead of a node, reports anchor to that line only. When evidence text
is omitted, the reporter shows the source line covered by the target span.
Expression and type nodes may span multiple lines; prefer the narrowest node
that still explains the finding.

Visitor hooks receive lowered IR nodes, not raw parser AST values. Fields removed
in older IR versions (`body_raw`, stringly-typed controller lists, and similar)
are not available — consult `DamlModule.ir_version` when upgrading rules.

## Project config

`daml-lint` reads `./daml.yaml` from the current directory by default. Use
`--config <FILE>` to load a different YAML file.

```yaml
daml-tools:
  lint:
    groups: [recommended]
    plugins: [template]
    plugin-paths: [./local-plugins]
    rules:
      missing-ensure-decimal: off
      template/template-requires-ensure:
        - warning
        - allowEmptyEnsure: false
```

Fields under `daml-tools.lint`:

| Field | Shape | Meaning |
|-------|-------|---------|
| `groups` | string array | Built-in rule groups to enable. `recommended` and `all` are supported. |
| `plugins` | string array | Plugin package names or short names. `template` resolves to `daml-lint-plugin-template`. |
| `plugin-paths` | string array | Additional package search roots, resolved relative to the config file. |
| `rules` | object | Built-in rule IDs or plugin-qualified rule IDs mapped to settings. |

Rule IDs for plugin packages use `plugin/rule`, following the same namespace
shape as ESLint and Solhint. The namespace is the package name without the
`daml-lint-plugin-` prefix, so `daml-lint-plugin-template` exposes
`template/<rule>`.

Rule settings accept:

| Setting | Meaning |
|---------|---------|
| `"off"` | Disable the rule. |
| `"critical"`, `"high"`, `"medium"`, `"low"`, `"info"`, `"error"`, `"warning"` | Enable and set a `daml-lint` severity. `error` maps to high and `warning` maps to medium. |
| `[severity, options]` | Enable with options exposed to the rule as global `CONFIG`. |

`CONFIG` defaults to `{}`. If more than one option value is provided after the
severity, `CONFIG` is an array of those values.

## Node shapes

The rule-facing IR is versioned by `DamlModule.ir_version`. Current rules see
`ir_version: 8`.

Important node families:

| Type | Purpose |
|------|---------|
| `DamlModule` | File-level imports, templates, interfaces, functions, and source text. |
| `Template` | Fields, signatories, observers, ensure clause, key, choices, interface instances (with optional `view_expr`). |
| `Choice` | Controllers, observers, authority expressions, parameters, return type, body statements, and `consuming` (`"consuming"` or `"non-consuming"`). |
| `Statement` | Tagged union for `Let`, `Assert`, `Fetch`, `Archive`, `Create`, `Exercise`, `TryCatch`, `Branch`, and `Other`. |
| `Import` | Module imports, `qualified` style (`"qualified"` or `"unqualified"`), and optional `package_label` source string from `import "pkg" Module`. |
| `Expr` | Tagged union for variables, constructors, literals, applications, binary operations, conditionals, case alternatives (with guarded branches and alternative-local `where` bindings), records, tuples, lists, and unknown expressions. |
| `TypeNode` | Structured Daml type tree with source spans. |

Removed v1/v2 raw fields such as `body_raw`, `raw_text`, `controllers`,
`signatories`, and string type fields are not part of the v4 contract.

## Runtime limits

The runtime executes JavaScript without Node APIs. Rules cannot use `require`,
`import`, filesystem access, or network access at runtime. Bundle TypeScript
and any helper imports into one JavaScript file before scanning.

Each rule script is evaluated once. The same visitors are reused across scanned
modules, so visitors should not accumulate module-specific state in top-level
mutable variables.

A runaway rule is interrupted after a bounded execution budget. Rule load or
runtime errors are reported and cause the CLI to exit `2`. Rules cannot spawn
workers, import Node built-ins, or access the filesystem at runtime.

## Packaging

Publish external rule packages as TypeScript source plus a bundled JavaScript
artifact, or publish only the bundled rule file if users do not need to edit
it. The JavaScript file passed to `--rules` must be self-contained.

Name published rule packages after the plugin they provide, following the same
pattern as Solhint plugins: `daml-lint-plugin-<name>` for unscoped packages or
`@scope/daml-lint-plugin-<name>` for scoped packages.

Installed plugin packages expose their rules through `package.json`:

```json
{
  "name": "daml-lint-plugin-template",
  "damlLint": {
    "rules": {
      "template-requires-ensure": "dist/template-requires-ensure.js"
    }
  }
}
```

The manifest rule key is the unqualified rule name. The bundled script must
define the same `const NAME`; users enable it as
`template/template-requires-ensure`.

The `@daml-tools/lint-plugin` package publishes the type contract entry point
(`dist/index.d.ts`), starter templates, and package README. It does not publish
bundled runtime helpers — authors compile rule logic themselves.
