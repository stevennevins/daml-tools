# daml-lint-rule-authoring

TypeScript types and starter templates for external `daml-lint --rules`
custom rule authors.

Install it with TypeScript and esbuild in your rule project:

```sh
npm pkg set type=module
npm install --save-dev daml-lint-rule-authoring typescript esbuild
```

Author rules in TypeScript, keep top-level `const NAME`, `const SEVERITY`, an
optional `const DESCRIPTION`, and top-level visitor `function` declarations,
then assign the same values to `globalThis.__daml_lint_rule` so TypeScript can
validate the rule object. Bundle the rule to one JavaScript file before passing
it to `daml-lint --rules`.

Runtime helper functions are intentionally not exported. The package is only
the public rule-facing IR contract and starter templates.
