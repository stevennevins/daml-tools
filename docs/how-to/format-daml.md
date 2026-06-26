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

From a local checkout:

```sh
cargo install --path crates/daml-fmt
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

## Avoid invalid option combinations

Do not combine `--write` and `--check`:

```sh
daml-fmt --write --check src/Foo.daml
```

That command exits with code `2`.

## Related

- [`daml-fmt` crate README](../../crates/daml-fmt/README.md)
- [CLI reference](../reference/cli.md)
