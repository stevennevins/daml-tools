# Format Daml source

Use `daml-fmt` to print formatted Daml source, rewrite files, check formatting,
or format stdin.

## Install `daml-fmt`

As a project dev dependency:

```sh
npm install --save-dev @daml-tools/daml-fmt
npx daml-fmt --check ./daml
```

From crates.io:

```sh
cargo install daml-fmt
```

## Print formatted output

Print one file to stdout without changing it:

```sh
daml-fmt Foo.daml
```

Format several files and print the formatted output:

```sh
daml-fmt src/Contracts.daml src/Choices.daml
```

## Rewrite files in place

Rewrite files with `-w` or `--write`:

```sh
find src -name '*.daml' -exec daml-fmt -w {} +
```

```sh
daml-fmt --write src/Contracts.daml
```

## Check formatting

Use `--check` in CI or hooks:

```sh
find src -name '*.daml' -exec daml-fmt --check {} +
```

Exit codes:

| Code | Meaning |
|------|---------|
| 0 | All files are formatted. |
| 1 | At least one file would change. |
| 2 | Formatter error. |

## Format stdin

Pipe source through `daml-fmt`:

```sh
cat Foo.daml | daml-fmt
```

With no file arguments, `daml-fmt` reads stdin and writes formatted source to
stdout.


## Run selected formatter rules

Run only one formatter rule with repeatable `--rule` options:

```sh
daml-fmt --rule imports Foo.daml
daml-fmt --rule spacing Foo.daml
```

Formatter rule ids are:

| Rule | Effect |
|------|--------|
| `imports` | Organize import declarations. |
| `layout` | Apply AST-guided structural indentation. |
| `spacing` | Normalize whitespace gaps and type-annotation colon spacing. |
| `syntax-normalization` | Rewrite supported inline/layout forms into canonical multiline shapes. |

Configure defaults in `./daml.yaml`:

```yaml
daml-tools:
  fmt:
    import-order: organize
    groups: [all]
    rules:
      imports: off
      layout: on
      spacing: on
      syntax-normalization: on
```

CLI `--rule` and `--group` selection overrides `daml.yaml`.

## Preserve import order

By default, `daml-fmt` organizes import declarations. Import reordering can
change Daml package identity, so preserve the original order when package
identity stability matters:

```sh
daml-fmt --preserve-import-order Foo.daml
```

You can also preserve import order by default in `./daml.yaml`:

```yaml
daml-tools:
  fmt:
    import-order: preserve
```

`daml-fmt` discovers config only at `./daml.yaml` in the current working
directory. It does not search parent directories. Pass `--config <FILE>` to use
a different YAML file. `--preserve-import-order` takes precedence over
`import-order` from config.

## Ignore generated or vendored files

Skip files with formatter config:

```yaml
daml-tools:
  fmt:
    ignore:
      - generated/**
      - vendor.daml
```

Config ignore patterns resolve relative to the directory containing the YAML
file. Ignored file arguments are skipped before read/check/print/write work, so
they do not produce unformatted output, diagnostics, or writes. Stdin has no
file path to ignore.

You can also load ignore patterns from one or more files:

```sh
daml-fmt --ignore-path .damlfmtignore --check ./**/*.daml
```

Ignore files support blank lines, `#` comments, exact paths, directory prefixes
ending in `/`, leading `/` relative to the ignore file directory, `*` within one
path segment, and `**` across path separators. This is a small gitignore-like
subset rather than full gitignore semantics.

## Avoid invalid option combinations

Do not combine `--write` and `--check`:

```sh
daml-fmt --write --check src/Foo.daml
```

That command exits with code `2`.

## Use the library API

When embedding the formatter in Rust code, prefer the public functions documented on
[docs.rs/daml-fmt](https://docs.rs/daml-fmt): `format_source`, `try_format_source`,
`FormatOptions`, and `source_diagnostics`.

## Related

- [`daml-fmt` crate README](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-fmt/README.md)
- [`daml-fmt` API on docs.rs](https://docs.rs/daml-fmt)
- [CLI reference](../reference/cli.md)
