# Scan Daml source

Use `daml-lint` to scan Daml files and directories, choose an output format,
write reports, apply custom rules, and set CI failure thresholds.

## Install `daml-lint`

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

## Run custom rule scripts

Pass JavaScript rule files with repeatable `--rules` options:

```sh
daml-lint ./daml/ --rules my-rule.js --rules another-rule.js
```

For a TypeScript rule, type-check and bundle it before scanning:

```sh
npm pkg set type=module
npm install --save-dev @daml-tools/lint-plugin typescript esbuild
npx tsc --noEmit
npx esbuild src/template-requires-ensure.ts --bundle --format=esm --target=es2020 --outfile=dist/template-requires-ensure.js
daml-lint ./daml/ --rules dist/template-requires-ensure.js --fail-on medium
```

The bundled JavaScript must expose top-level `const NAME`, `const SEVERITY`,
an optional `const DESCRIPTION`, and at least one top-level visitor `function`.
Assigning `globalThis.__daml_lint_rule` gives TypeScript a rule object to
validate, but it does not replace the current runtime discovery contract.

## Run installed plugin rules

Install a plugin package in the project and enable its rules from
`.daml-lint.json`:

```sh
npm install --save-dev daml-lint-plugin-template
cat > .daml-lint.json <<'JSON'
{
  "plugins": ["template"],
  "rules": {
    "template/template-requires-ensure": "medium"
  }
}
JSON
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

- [`daml-lint` crate README](../../crates/daml-lint/README.md)
- [Write a custom rule](../tutorials/write-a-daml-lint-custom-rule.md)
- [Custom rule contract](../reference/daml-lint-custom-rule-contract.md)
- [CLI reference](../reference/cli.md)
