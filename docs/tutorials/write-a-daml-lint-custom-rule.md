# Write a daml-lint custom rule

This tutorial starts from an empty directory and builds one custom rule that
reports templates without an `ensure` clause.

You need Node.js 18 or newer. Install `@daml-tools/lint-plugin` for typed rule
authoring — see the [custom rule contract](../reference/daml-lint-custom-rule-contract.md).

## Create the project

```sh
mkdir daml-lint-plugin-template-requires-ensure
cd daml-lint-plugin-template-requires-ensure
npm init -y
npm pkg set type=module
npm pkg set damlLint.rules.template-requires-ensure=dist/template-requires-ensure.js
npm install --save-dev @daml-tools/daml-lint @daml-tools/lint-plugin typescript esbuild
mkdir -p src fixtures dist
```

Create `tsconfig.json`:

```json
{
  "compilerOptions": {
    "target": "ES2020",
    "module": "ES2020",
    "moduleResolution": "bundler",
    "strict": true,
    "noEmit": true,
    "lib": ["ES2020"]
  },
  "include": ["src/**/*.ts"]
}
```

## Add the rule

Create `src/template-requires-ensure.ts`:

```typescript
import type { DamlLintRuleModule, Template } from "@daml-tools/lint-plugin";

const NAME = "template-requires-ensure";
const SEVERITY = "medium";
const DESCRIPTION = "Every template must declare an ensure clause";

function on_template(template: Template): void {
  if (template.ensure_clause === null) {
    report(template, `Template '${template.name}' has no ensure clause`);
  }
}

const rule: DamlLintRuleModule = { NAME, SEVERITY, DESCRIPTION, on_template };
globalThis.__daml_lint_rule = rule;
```

The top-level constants and `function on_template` are the runtime contract.
The `rule` object is the TypeScript-checked authoring shape.

## Add fixtures

Create `fixtures/missing-ensure.daml`:

```daml
module MissingEnsure where

template Iou
  with
    issuer : Party
    owner : Party
  where
    signatory issuer
    observer owner
```

Create `fixtures/with-ensure.daml`:

```daml
module WithEnsure where

template Iou
  with
    issuer : Party
    owner : Party
  where
    signatory issuer
    observer owner
    ensure True
```

## Add a local lint config

Create `daml.yaml`:

```yaml
daml-tools:
  lint:
    plugin-paths: [.]
    plugins: [template-requires-ensure]
    rules:
      template-requires-ensure/template-requires-ensure: warning
```

`plugin-paths: [.]` lets the package resolve itself before it is published.
After publishing and installing it in another project, consumers only need the
`plugins` and `rules` entries under `daml-tools.lint`.

## Type-check and bundle

```sh
npx tsc --noEmit
npx esbuild src/template-requires-ensure.ts --bundle --format=esm --target=es2020 --outfile=dist/template-requires-ensure.js
```

## Run the rule

The missing fixture should report one finding:

```sh
npx daml-lint fixtures/missing-ensure.daml --fail-on info
```

The ensured fixture should not report this custom finding:

```sh
npx daml-lint fixtures/with-ensure.daml --fail-on info
```

You now have a typed rule package that can be loaded through `./daml.yaml`.
The bundled JavaScript file also still works with direct `--rules` loading when
you want to test one script without a package manifest. See
[Scan Daml source](../how-to/scan-daml.md) for `--rules`, plugin packages, and
CI SARIF output.
