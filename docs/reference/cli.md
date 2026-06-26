# CLI reference

This page describes the command-line interfaces shipped by this workspace.

## `daml-fmt`

`daml-fmt` formats Daml source files. The published binary is defined in
[`crates/daml-fmt/src/bin/daml-fmt.rs`](../../crates/daml-fmt/src/bin/daml-fmt.rs).

```sh
daml-fmt [options] [file...]
```

With no file arguments, `daml-fmt` reads stdin and writes formatted source to
stdout.

### Options

| Option | Description |
|--------|-------------|
| `-w`, `--write` | Rewrite each file in place when the formatted output differs. Requires file arguments. |
| `--check` | Print each file that would change and exit `1` if any file is not formatted. Requires file arguments. |
| `--preserve-import-order` | Keep import declarations in source order instead of applying default import organization. |
| `-h`, `--help` | Show usage text and exit `0`. |
| `-v`, `--version` | Print the crate version and exit `0`. |

`--write` and `--check` are mutually exclusive.

### Input and output

| Invocation | Input | Output |
|------------|-------|--------|
| `daml-fmt` | stdin | formatted source on stdout |
| `daml-fmt file.daml` | file contents | formatted source on stdout |
| `daml-fmt -w file.daml` | file contents | rewrites the file only when changed |
| `daml-fmt --check file.daml` | file contents | prints the file path if formatting would change it |

Malformed input with lexical or parser diagnostics, such as an unterminated
string or incomplete expression, is reported on stderr. In write mode,
malformed input is not rewritten.

Import organization is enabled by default. It may change Daml package identity
because import declaration order contributes to the compiled package; pass
`--preserve-import-order` when package identity stability matters.

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | Formatting completed successfully, or `--check` found no changes. |
| `1` | `--check` found one or more files that would change. |
| `2` | CLI usage error, read/write error, or malformed input with lexical or parser diagnostics. |

## `daml-lint`

`daml-lint` scans Daml files with built-in and optional custom detectors. The
binary is defined in
[`crates/daml-lint/src/main.rs`](../../crates/daml-lint/src/main.rs).

```sh
daml-lint [options] <paths>...
```

Each path may be a `.daml` file or a directory. Directories are scanned
recursively for files with the `.daml` extension. Nonexistent paths produce a
warning. If no `.daml` files are found, the command exits `2`.

### Options

| Option | Description |
|--------|-------------|
| `<paths>...` | Required file or directory paths to scan. |
| `-f`, `--format <FORMAT>` | Output format. Accepted values: `markdown`, `md`, `json`, `sarif`. Default: `markdown`. |
| `-o`, `--output <FILE>` | Write the report to a file instead of stdout. |
| `--fail-on <SEVERITY>` | Minimum finding severity that causes exit `1`. Accepted values: `critical`, `high`, `medium`, `low`, `info`. Default: `high`. |
| `-c`, `--config <FILE>` | Load a YAML config file with `daml-tools.lint` settings. Default discovery: `./daml.yaml` then `./daml.yml` in the current directory. Requires the `custom-rules` feature. |
| `--rule <ID>` | Run a built-in or plugin rule by id. Repeatable. Replaces config rule/group selection. Requires the `custom-rules` feature. |
| `--group <GROUP>` | Run a built-in or plugin rule group (`recommended`, `all`, `off`, or `plugin/group`). Repeatable. Replaces config rule/group selection. Requires the `custom-rules` feature. |
| `--rules <FILE>` | Load a JavaScript custom rule file. Repeatable. Requires the `custom-rules` feature. |
| `-h`, `--help` | Show clap-generated help. |
| `-V`, `--version` | Show the crate version. |

### Config file

`daml.yaml` / `daml.yml` can configure linting under top-level `daml-tools.lint`.
`daml-lint` discovers `./daml.yaml` then `./daml.yml` in the current directory.
Legacy `.daml-lint.json` is not read.

```yaml
daml-tools:
  lint:
    plugins: [template]
    plugin-paths: [./local-plugins]
    groups: [recommended, template/recommended]
    rules:
      missing-ensure-decimal: off
      template/template-requires-ensure: [medium, { allowEmptyEnsure: false }]
```

Built-in groups:

| Group | Rules |
|-------|-------|
| `recommended` | All built-in detectors (default when no config or CLI selection). |
| `all` | Same as `recommended`. |
| `off` | No built-in detectors. |

Plugin groups use `plugin/group` ids and are declared in the plugin package
`package.json` under `damlLint.groups`.

When `--rule` or `--group` is passed on the CLI, that selection replaces config
`groups` and config rule enablement. Config severities and per-rule options still
apply to selected rules. `--rules <FILE>` continues to load ad hoc JavaScript
rule files and is separate from `--rule <ID>`.

Plugin names resolve to npm packages with the `daml-lint-plugin-` prefix, so
`template` resolves to `daml-lint-plugin-template`. Configured plugin rules are
reported as `plugin/rule`. Rule options are exposed to the JavaScript rule as
global `CONFIG`.

### Output formats

| Format | Description |
|--------|-------------|
| `markdown`, `md` | Human-readable report grouped by severity. |
| `json` | JSON object containing findings, parse errors, and summary counts. |
| `sarif` | SARIF 2.1.0 report for code-scanning integrations. Parse errors are reported as tool execution notifications. |

### Detectors

Built-in detectors are registered by
[`detectors::create_builtin_detectors`](../../crates/daml-lint/src/detectors/mod.rs).

| Detector | Severity | Description |
|----------|----------|-------------|
| `missing-ensure-decimal` | High | Template has a `Decimal` field without a positivity bound in its `ensure` clause. |
| `unguarded-division` | High | Division occurs without a prior guard on the denominator. |
| `missing-positive-amount` | High | Choice accepts amount, quantity, price, or input-list parameters without a positive-value or non-empty check. |
| `archive-before-execute` | High | Contract is archived before a `try/catch` block. |
| `head-of-list-query` | Medium | Query result is consumed with a head-of-list pattern or operation where result ordering is nondeterministic. |
| `unbounded-fields` | Medium | `Text`, `TextMap`, or list fields lack an `ensure` size bound. |

Custom rule names passed through `--rules` must not collide with built-in
detector names or with each other. Installed plugin rules are namespaced by the
plugin name.

### Parse diagnostics

`daml-lint` reports parser diagnostics on stderr and includes parse errors in
formatted output. Each diagnostic carries a stable category tag such as
`lexical-error`, `malformed`, `skipped-declaration`, `unsupported-syntax`, or
`recursion-limit` (JSON `parseErrors[].category`, SARIF notification
`properties.category`, markdown report headings). A scan with parse errors exits
`3`, even if no detector findings meet the `--fail-on` threshold.

Rule settings in `daml-tools.lint.rules` accept only canonical severities:
`off`, `critical`, `high`, `medium`, `low`, and `info`. Legacy aliases such as
`warn`/`error` and numeric shortcuts are rejected.

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | No findings at or above the `--fail-on` threshold, and no parse errors. |
| `1` | One or more findings are at or above the `--fail-on` threshold. |
| `2` | CLI usage error, invalid option value, no `.daml` files found, rule load/runtime error, detector error, or report write error. |
| `3` | One or more scanned files had parse errors; the scan is not authoritative. |
