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

With file arguments, `daml-fmt` rewrites files in place by default. To print
formatted output to stdout instead, pipe through stdin or use a shell redirect
after formatting.

Format stdin:

```sh
cat Foo.daml | daml-fmt
```

## Rewrite files in place

File arguments rewrite in place by default:

```sh
daml-fmt src/Contracts.daml
```

`-w` and `--write` are explicit aliases for the same behavior:

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

## Preserve import order

By default, `daml-fmt` organizes import declarations. Import reordering can
change Daml package identity, so preserve the original order when package
identity stability matters:

```sh
daml-fmt --preserve-import-order Foo.daml
```

`--preserve-import-order` removes `import-order` from the default formatter
rule set. It conflicts with an explicit `--rule import-order`.

## Select formatter rules

Configure formatter rules in `daml.yaml` / `daml.yml`:

```yaml
daml-tools:
  fmt:
    rules: [structural-layout, import-order, layout-rewrites, gap-normalization]
```

Or pass repeatable `--rule <ID>` flags. CLI `--rule` replaces config `fmt.rules`.

```sh
daml-fmt --check --rule import-order src/Foo.daml
daml-fmt --rule gap-normalization src/Foo.daml
```

Rule ids (applied in this order when selected):

- `structural-layout`
- `import-order`
- `layout-rewrites`
- `gap-normalization`

Load an explicit config file with `--config <FILE>`. Without `--config`,
`daml-fmt` checks only `./daml.yaml` then `./daml.yml`.

## Avoid invalid option combinations

Do not combine `--write` and `--check`:

```sh
daml-fmt --write --check src/Foo.daml
```

That command exits with code `2`.

## Related

- [`daml-fmt` crate README](../../crates/daml-fmt/README.md)
- [CLI reference](../reference/cli.md)
