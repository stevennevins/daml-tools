# daml-lint custom rule contract

This page describes the custom rule interface loaded by `daml-lint --rules`
and by installed plugin packages configured in `daml.yaml` under
`daml-tools.lint`.

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
`crates/daml-lint/examples/daml-lint.d.ts`.

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

## Project config

`daml-lint` reads `daml-tools.lint` from `./daml.yaml` in the
current directory by default. Use `--config <FILE>` to load a different YAML
file. Legacy `.daml-lint.json` is not discovered.

```yaml
daml-tools:
  lint:
    plugins: [template]
    plugin-paths: [./local-plugins]
    groups: [recommended]
    rules:
      missing-ensure-decimal: off
      template/template-requires-ensure: [medium, { allowEmptyEnsure: false }]
```

Fields:

| Field | Shape | Meaning |
|-------|-------|---------|
| `plugins` | string array | Plugin package names or short names. `template` resolves to `daml-lint-plugin-template`. |
| `plugin-paths` | string array | Additional package search roots, resolved relative to the config file. |
| `groups` | string array | Built-in groups (`recommended`, `all`, `off`) or plugin groups (`plugin/group`). |
| `rules` | object | Built-in rule IDs or plugin-qualified rule IDs mapped to settings. |

Rule IDs for plugin packages use `plugin/rule`, following the same namespace
shape as ESLint and Solhint. The namespace is the package name without the
`daml-lint-plugin-` prefix, so `daml-lint-plugin-template` exposes
`template/<rule>`.

Rule settings accept:

| Setting | Meaning |
|---------|---------|
| `"off"` | Disable the rule. |
| `"critical"`, `"high"`, `"medium"`, `"low"`, `"info"` | Enable and set a `daml-lint` severity. |
| `[severity, options]` | Enable with options exposed to the rule as global `CONFIG`. |

`CONFIG` defaults to `{}`. If more than one option value is provided after the
severity, `CONFIG` is an array of those values.

## Node shapes

The rule-facing IR is versioned by `DamlModule.ir_version`. Current rules see
`ir_version: 4`.

Important node families:

| Type | Purpose |
|------|---------|
| `DamlModule` | File-level imports, templates, interfaces, functions, and source text. |
| `Template` | Fields, signatories, observers, ensure clause, key, choices, interface instances. |
| `Choice` | Controllers, observers, parameters, return type, body statements, and `consuming` (`"consuming"` or `"non-consuming"`). |
| `Statement` | Tagged union for `Let`, `Assert`, `Fetch`, `Archive`, `Create`, `Exercise`, `TryCatch`, `Branch`, and `Other`. |
| `Import` | Module imports and `qualified` style (`"qualified"` or `"unqualified"`). |
| `Expr` | Tagged union for variables, constructors, literals, applications, binary operations, conditionals, records, tuples, lists, and unknown expressions. |
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

A runaway rule is interrupted. Rule load or runtime errors are reported and
cause the CLI to exit `2`.

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
    },
    "groups": {
      "recommended": ["template-requires-ensure"]
    }
  }
}
```

The manifest rule key is the unqualified rule name. The bundled script must
define the same `const NAME`; users enable it as
`template/template-requires-ensure`.

The `@daml-tools/lint-plugin` package publishes only the type contract and
starter templates. It does not publish runtime helpers.
