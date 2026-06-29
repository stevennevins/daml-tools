# __PACKAGE_NAME__

This starter is a multi-rule `daml-lint` plugin package. One package exposes
multiple bundled rules through `package.json` `damlLint.rules`, and `./daml.yaml`
enables them under the `__PLUGIN_NAME__` plugin namespace.

```sh
npm install
npm run check
npm run build
npm run test:rules
```

`npm run test:rules` expects both example rules to report findings on
`fixtures/violations.daml` and no custom-rule findings on `fixtures/clean.daml`.

Add more rules by creating `src/rules/<rule-name>.ts`, bundling to
`dist/rules/<rule-name>.js`, and registering the rule in `package.json` and
`daml.yaml`.

For one-off debugging, a bundled rule file can still be passed directly:

```sh
daml-lint fixtures/violations.daml --rules dist/rules/template-requires-ensure.js --fail-on info
```
