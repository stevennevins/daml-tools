# First run

In this lesson, you will build the workspace, format a small Daml file, and
scan it with `daml-lint`.

You will work in a temporary directory so the repository checkout stays
unchanged.

## Prerequisites

You need:

- Rust 1.96 or newer
- a checkout of this repository
- a shell from the repository root

For project setup details, see the root [README](https://github.com/stevennevins/daml-tools/blob/main/README.md).

## Build the tools

From the repository root, build every crate in the workspace:

```sh
cargo build --workspace
```

This builds the shared `daml-parser` crate and the two CLI tools, `daml-fmt`
and `daml-lint`.

## Create a small Daml file

Create a temporary lesson directory:

```sh
mkdir -p /tmp/daml-tools-first-run
```

Create a Daml file:

```sh
cat > /tmp/daml-tools-first-run/Iou.daml <<'EOF'
module Tutorial.FirstRun where

template Iou
  with
    issuer : Party
    owner : Party
    amount : Decimal
  where
    signatory issuer
    observer owner
EOF
```

The sample is intentionally small and has one lint finding: a `Decimal` field
without an `ensure` clause.

## Preview formatted output

Run `daml-fmt` without changing the file:

```sh
cargo run -p daml-fmt --bin daml-fmt -- /tmp/daml-tools-first-run/Iou.daml
```

The formatted source is printed to stdout. The file on disk is unchanged.

## Format the file in place

Rewrite the file:

```sh
cargo run -p daml-fmt --bin daml-fmt -- -w /tmp/daml-tools-first-run/Iou.daml
```

Check that the file is now formatted:

```sh
cargo run -p daml-fmt --bin daml-fmt -- --check /tmp/daml-tools-first-run/Iou.daml
```

A formatted file exits with code `0`.

## Scan the file

Run `daml-lint` and keep the command successful by failing only on critical
findings:

```sh
cargo run -p daml-lint -- /tmp/daml-tools-first-run/Iou.daml --fail-on critical
```

The report is printed in Markdown. The sample template has a `Decimal` field
without an `ensure` clause, so the scanner reports a finding. Because this run
fails only on critical findings, the command exits successfully.

Now run the same scan as a stricter gate:

```sh
cargo run -p daml-lint -- /tmp/daml-tools-first-run/Iou.daml --fail-on high
```

This command is expected to exit with code `1`, because the sample has a
high-severity finding.

## Finish

You have built the workspace, formatted a Daml file, checked formatting, and
scanned the file with a lint threshold.

For task-focused usage, see:

- [Format Daml source](../how-to/format-daml.md)
- [Scan Daml source](../how-to/scan-daml.md)
