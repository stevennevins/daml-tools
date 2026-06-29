# @daml-tools/lint-plugin

TypeScript types and starter templates for external `daml-lint` custom rule
plugin authors.

## Scaffold a multi-rule plugin package

```sh
npx -y -p @daml-tools/lint-plugin create-daml-lint-plugin ledger-style
cd daml-lint-plugin-ledger-style
npm install
npm run check
npm run build
npm run test:rules
```

The scaffold creates a package with multiple rules under `src/rules/`, bundles
them to `dist/rules/`, and enables them from `./daml.yaml` under one plugin
namespace.

## Author rules in TypeScript

Install the contract package with TypeScript and esbuild in an existing rule
project:

```sh
npm pkg set type=module
npm install --save-dev @daml-tools/daml-lint @daml-tools/lint-plugin typescript esbuild
```

Author rules in TypeScript, keep top-level `const NAME`, `const SEVERITY`, an
optional `const DESCRIPTION`, and top-level visitor `function` declarations,
then assign the same values to `globalThis.__daml_lint_rule` so TypeScript can
validate the rule object. Bundle each rule to one JavaScript file before
exposing it from a plugin package.

Installed plugin packages expose bundled rules from `package.json`:

```json
{
  "name": "daml-lint-plugin-ledger-style",
  "damlLint": {
    "rules": {
      "template-requires-ensure": "dist/rules/template-requires-ensure.js",
      "unqualified-da-import": "dist/rules/unqualified-da-import.js"
    }
  }
}
```

Consumers enable installed rules from `daml.yaml` under
`daml-tools.lint.rules` with `plugin/rule` IDs. Rule options from
`[severity, options]` settings are available as global `CONFIG`.

For one-off debugging, bundled rule files can still be passed directly to
`daml-lint --rules`.

Runtime helper functions are intentionally not exported. The package is the
public rule-facing IR contract and starter templates for plugin projects.
