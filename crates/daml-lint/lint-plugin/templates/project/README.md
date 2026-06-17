# daml-lint plugin starter

This project shows the minimal TypeScript flow for a `daml-lint --rules`
custom rule plugin.

```sh
npm install
npm run check
npm run build
daml-lint fixtures/missing-ensure.daml --rules dist/template-requires-ensure.js --fail-on info
daml-lint fixtures/with-ensure.daml --rules dist/template-requires-ensure.js --fail-on info
```

The first scan reports one `template-requires-ensure` finding. The second scan
has no finding from the custom rule.

The runtime still discovers top-level metadata constants and visitor
`function` declarations. The `globalThis.__daml_lint_rule` assignment gives
TypeScript a single rule object to validate, but it is not the only runtime
discovery mechanism.
