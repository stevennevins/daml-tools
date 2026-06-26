# daml-lint plugin starter

This project shows the minimal TypeScript flow for a `daml-lint` custom rule
plugin loaded through `./daml.yaml`.

```sh
npm install
npm run check
npm run build
npm run lint:missing
npm run lint:clean
```

The first scan reports one `template-requires-ensure` finding. The second scan
has no finding from the custom rule.

The package manifest exposes bundled rule files under `damlLint.rules`.
`daml.yaml` uses `plugin-paths: [.]` so the project can resolve itself before
it is published. After publishing, consumers install the package and enable
rules by `plugin/rule` ID.

The runtime still discovers top-level metadata constants and visitor
`function` declarations. The `globalThis.__daml_lint_rule` assignment gives
TypeScript a single rule object to validate, but it is not the only runtime
discovery mechanism.

For a one-off script test, the bundled JavaScript can still be passed directly:

```sh
daml-lint fixtures/missing-ensure.daml --rules dist/template-requires-ensure.js --fail-on info
```
