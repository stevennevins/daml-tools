---
description: Scaffold a daml-lint plugin package, add a custom TypeScript rule, and run it with daml-lint.
---

# Write a daml-lint custom rule

This tutorial shows how to create a multi-rule plugin package with
`create-daml-lint-plugin`, then extend it with additional rules.

You need Node.js 18 or newer. Install `@daml-tools/lint-plugin` for typed rule
authoring — see the [custom rule contract](../reference/daml-lint-custom-rule-contract.md).

## Scaffold a plugin package

```sh
npx -y -p @daml-tools/lint-plugin create-daml-lint-plugin ledger-style
cd daml-lint-plugin-ledger-style
npm install
npm run check
npm run build
npm run test:rules
```

The scaffold creates a package with two example rules under `src/rules/`:

- `template-requires-ensure` uses `on_template`
- `unqualified-da-import` uses `on_import`

Both rules are bundled to `dist/rules/`, registered in `package.json`
`damlLint.rules`, and enabled from `./daml.yaml` under the `ledger-style`
plugin namespace.

`npm run test:rules` verifies that both rules report findings on
`fixtures/violations.daml` and that neither reports on `fixtures/clean.daml`.

## Add another rule

Create `src/rules/my-rule.ts`:

```typescript
import type { DamlLintRuleModule, Template } from "@daml-tools/lint-plugin";

const NAME = "my-rule";
const SEVERITY = "medium";
const DESCRIPTION = "Example additional rule";

function on_template(template: Template): void {
  if (template.name.endsWith("Draft")) {
    report(template, `Template '${template.name}' looks like a draft`);
  }
}

const rule: DamlLintRuleModule = { NAME, SEVERITY, DESCRIPTION, on_template };
globalThis.__daml_lint_rule = rule;
```

Register the bundled output in `package.json`:

```json
{
  "damlLint": {
    "rules": {
      "template-requires-ensure": "dist/rules/template-requires-ensure.js",
      "unqualified-da-import": "dist/rules/unqualified-da-import.js",
      "my-rule": "dist/rules/my-rule.js"
    }
  }
}
```

Enable it from `daml.yaml`:

```yaml
daml-tools:
  lint:
    plugin-paths: [.]
    plugins: [ledger-style]
    rules:
      ledger-style/my-rule: warning
```

Rebuild and extend `scripts/smoke-test.mjs` if you want CI-style coverage for
the new rule.

## Debug one bundled rule directly

For one-off debugging without `./daml.yaml`, pass a bundled rule file directly:

```sh
npx daml-lint fixtures/violations.daml --rules dist/rules/template-requires-ensure.js --fail-on info
```

See [Scan Daml source](../how-to/scan-daml.md) for plugin packages, `--rules`, and
CI SARIF output.
