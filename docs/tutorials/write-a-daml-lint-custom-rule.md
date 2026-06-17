# Write a daml-lint custom rule

This tutorial starts from an empty directory and builds one custom rule that
reports templates without an `ensure` clause.

You need Node.js 18 or newer and `daml-lint` on your `PATH`.

## Create the project

```sh
mkdir daml-lint-template-rule
cd daml-lint-template-rule
npm init -y
npm pkg set type=module
npm install --save-dev daml-lint-rule-authoring typescript esbuild
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
import type { DamlLintRuleModule, Template } from "daml-lint-rule-authoring";

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

## Type-check and bundle

```sh
npx tsc --noEmit
npx esbuild src/template-requires-ensure.ts --bundle --format=esm --target=es2020 --outfile=dist/template-requires-ensure.js
```

## Run the rule

The missing fixture should report one finding:

```sh
daml-lint fixtures/missing-ensure.daml --rules dist/template-requires-ensure.js --fail-on info
```

The ensured fixture should not report this custom finding:

```sh
daml-lint fixtures/with-ensure.daml --rules dist/template-requires-ensure.js --fail-on info
```

You now have a typed rule project that produces the single JavaScript file
expected by `daml-lint --rules`.
