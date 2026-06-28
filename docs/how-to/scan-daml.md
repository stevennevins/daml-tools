# Scan Daml source

Use `daml-lint` to scan Daml files and directories, choose an output format,
write reports, apply custom rules, and set CI failure thresholds.

## Install `daml-lint`

As a project dev dependency:

```sh
npm install --save-dev @daml-tools/daml-lint
npx daml-lint ./daml
```

From crates.io:

```sh
cargo install daml-lint
```

From a local checkout:

```sh
cargo install --path crates/daml-lint
```

## Scan one file

```sh
daml-lint src/MyContract.daml
```

## Scan a directory

```sh
daml-lint ./daml/
```

`daml-lint` scans `.daml` files under the provided directory.

## Choose an output format

Markdown is the default:

```sh
daml-lint ./daml/ --format markdown
```

Use SARIF for code-scanning integrations:

```sh
daml-lint ./daml/ --format sarif
```

Use JSON for machine-readable output:

```sh
daml-lint ./daml/ --format json
```

## Write the report to a file

```sh
daml-lint ./daml/ --format sarif --output report.sarif
```

## Set the failure threshold

Fail on medium findings or higher:

```sh
daml-lint ./daml/ --fail-on medium
```

Fail only on critical findings:

```sh
daml-lint ./daml/ --fail-on critical
```

Supported thresholds are `critical`, `high`, `medium`, `low`, and `info`.


## Run selected lint rules

Run one rule by id:

```sh
daml-lint ./daml/ --rule missing-ensure-decimal
```

Run a rule group:

```sh
daml-lint ./daml/ --group recommended
```

Available built-in rule ids are listed in the CLI reference. CLI `--rule` and
`--group` selection overrides config selection from `daml.yaml`.

## Configure rules in daml.yaml

`daml-lint` reads `./daml.yaml` by default when it exists. Use `--config` to
load a different YAML file.

```yaml
daml-tools:
  lint:
    groups: [recommended]
    rules:
      missing-ensure-decimal: off
      head-of-list-query: warning
      unguarded-division: error
```

Lint severities are `critical`, `high`, `medium`, `low`, `info`, plus
ESLint-style aliases `error` (high) and `warning` (medium). Use `off` to disable
a rule.

## Run custom rule scripts

Pass JavaScript rule files with repeatable `--rules` options:

```sh
daml-lint ./daml/ --rules my-rule.js --rules another-rule.js
```

For a TypeScript rule, type-check and bundle it before scanning:

```sh
npm pkg set type=module
npm install --save-dev @daml-tools/daml-lint @daml-tools/lint-plugin typescript esbuild
npx tsc --noEmit
npx esbuild src/template-requires-ensure.ts --bundle --format=esm --target=es2020 --outfile=dist/template-requires-ensure.js
npx daml-lint ./daml/ --rules dist/template-requires-ensure.js --fail-on medium
```

The bundled JavaScript must expose top-level `const NAME`, `const SEVERITY`,
an optional `const DESCRIPTION`, and at least one top-level visitor `function`.
Assigning `globalThis.__daml_lint_rule` gives TypeScript a rule object to
validate, but it does not replace the current runtime discovery contract.

## Run installed plugin rules

Install a plugin package in the project and enable its rules from
`./daml.yaml`:

```sh
npm install --save-dev daml-lint-plugin-template
cat > daml.yaml <<'YAML'
daml-tools:
  lint:
    plugins: [template]
    rules:
      template/template-requires-ensure: warning
YAML
daml-lint ./daml/ --fail-on medium
```

Use `[severity, options]` when a rule accepts configuration. The options value
is available to the rule as global `CONFIG`.

## Use in CI

A typical CI scan writes SARIF and fails on high findings or higher:

```sh
daml-lint ./daml/ --format sarif --output daml-lint.sarif --fail-on high
```

Exit codes:

| Code | Meaning |
|------|---------|
| 0 | No findings at or above the `--fail-on` threshold. |
| 1 | One or more findings at or above the threshold. |
| 2 | CLI error. |
| 3 | A scanned file had parse errors, so the scan is not authoritative. |

Treat exit code `3` as a failed quality gate. Fix parse errors, then run the
scan again.

## Related

- [`daml-lint` crate README](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-lint/README.md)
- [`daml-lint` API on docs.rs](https://docs.rs/daml-lint)
- [Write a custom rule](../tutorials/write-a-daml-lint-custom-rule.md)
- [Custom rule contract](../reference/daml-lint-custom-rule-contract.md)
- [CLI reference](../reference/cli.md)
