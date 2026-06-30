---
description: Command-line options, outputs, exit codes, and configuration details for daml-fmt and daml-lint.
---

# CLI reference

This page describes the command-line interfaces shipped by this workspace.

## `daml-fmt`

`daml-fmt` formats Daml source files. The published binary is defined in
[`crates/daml-fmt/src/bin/daml-fmt.rs`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-fmt/src/bin/daml-fmt.rs).

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
| `--config <FILE>` | Load formatter config from a YAML file. Default discovery: `./daml.yaml` when present. |
| `--ignore-path <FILE>` | Load formatter ignore patterns from a file. Repeatable. Patterns resolve relative to the ignore file's directory. |
| `--group <ID>` | Enable a formatter rule group. Repeatable. Currently `all`. |
| `--rule <ID>` | Enable a specific formatter rule. Repeatable. Accepted values: `imports`, `layout`, `spacing`, `syntax-normalization`. |
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

Formatter rule selection runs only the selected rules. CLI `--rule`/`--group`
selection overrides `daml.yaml`. With no explicit selection, all formatter
rules run unless config disables one.

Formatter config may set `daml-tools.fmt.import-order` to `organize` or
`preserve`. `--preserve-import-order` takes precedence over config. File
arguments matching `daml-tools.fmt.ignore` or any repeatable `--ignore-path`
file are skipped before reading, checking, printing, or writing. Stdin has no
file path, so ignore patterns do not apply.

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | Formatting completed successfully, or `--check` found no changes. |
| `1` | `--check` found one or more files that would change. |
| `2` | CLI usage error, read/write error, or malformed input with lexical or parser diagnostics. |

## `daml-lint`

`daml-lint` scans Daml files with built-in and optional custom detectors. The
binary is defined in
[`crates/daml-lint/src/main.rs`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-lint/src/main.rs).

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
| `-c`, `--config <FILE>` | Load config from a YAML file. Default discovery: `./daml.yaml` when present. |
| `--rule <ID>` | Run only the named lint rule. Repeatable. Built-ins use their detector id; plugin rules use `plugin/rule`. |
| `--group <ID>` | Run a lint rule group. Repeatable. Accepted values: `recommended`, `all`. |
| `--rules <FILE>` | Load a JavaScript custom rule file. Repeatable. Requires the `custom-rules` feature. |
| `-h`, `--help` | Show clap-generated help. |
| `-V`, `--version` | Show the crate version. |

### Config file

`./daml.yaml` can configure formatter and linter rule groups, rule toggles,
severity overrides, plugin packages, plugin search roots, and rule-specific
options:

```yaml
daml-tools:
  fmt:
    import-order: preserve
    ignore:
      - generated/**
      - vendor.daml
    groups: [all]
    rules:
      imports: off
      layout: on
      spacing: on
      syntax-normalization: on

  lint:
    groups: [recommended]
    plugin-paths: [./plugins]
    plugins: [template]
    rules:
      missing-ensure-decimal: off
      head-of-list-query: warning
      template/template-requires-ensure:
        - warning
        - allowEmptyEnsure: false
```

Default discovery checks only `./daml.yaml` in the current working directory for
both `daml-fmt` and `daml-lint`; it does not walk parent directories and does
not read `.daml-lint.json`.
`--config <FILE>` selects a specific YAML file instead.

| Field | Applies to | Description |
|-------|------------|-------------|
| `import-order` | `fmt` | Formatter import ordering strategy: `organize` (default) or `preserve`. Overridden by `daml-fmt --preserve-import-order`. |
| `ignore` | `fmt` | Formatter ignore patterns. Relative patterns resolve from the config file directory. |
| `groups` | `fmt`, `lint` | Rule groups to enable before per-rule overrides. Formatter accepts `all`; linter accepts `recommended` and `all`. |
| `rules` | `fmt`, `lint` | Map of rule ids to settings. Formatter settings are `on`/`off`; linter settings are `off`, `on`, a severity, or `[severity, options]`. |
| `plugins` | `lint` | Installed lint plugin package names. `template` resolves to `daml-lint-plugin-template`; scoped packages may use `@scope/name`. |
| `plugin-paths` | `lint` | Additional package search roots for plugin packages. Relative paths are resolved relative to the config file. |

Formatter ignore files passed with `--ignore-path <FILE>` support blank lines,
lines whose first non-whitespace character is `#`, exact paths, directory
prefixes ending in `/`, leading `/` anchors relative to the ignore source
directory, `*` within one path segment, and `**` across path separators. This
is a documented subset of gitignore semantics.

Configured plugin rules are reported as `plugin/rule`. Rule options are exposed
to the JavaScript rule as global `CONFIG`.

### Output formats

| Format | Description |
|--------|-------------|
| `markdown`, `md` | Human-readable report grouped by severity. |
| `json` | JSON object containing findings, parse errors, and summary counts. |
| `sarif` | SARIF 2.1.0 report for code-scanning integrations. Parse errors are reported as tool execution notifications. |

### Detectors

Built-in detectors are registered by
[`detectors::create_builtin_detectors`](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-lint/src/detectors/mod.rs).

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

Lint rule settings in `daml.yaml` accept `off`, `on`, `warning`, `error`,
and canonical severities `critical`, `high`, `medium`, `low`, and `info`.
`warning` maps to medium severity and `error` maps to high severity. Numeric
shortcuts and `warn` are rejected. Formatter rule settings accept `on`/`off`.

### Exit codes

| Code | Meaning |
|------|---------|
| `0` | No findings at or above the `--fail-on` threshold, and no parse errors. |
| `1` | One or more findings are at or above the `--fail-on` threshold. |
| `2` | CLI usage error, invalid option value, no `.daml` files found, rule load/runtime error, detector error, or report write error. |
| `3` | One or more scanned files had parse errors; the scan is not authoritative. |
