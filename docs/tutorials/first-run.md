---
description: Install the published daml-fmt and daml-lint CLIs, format a sample Daml file, and run a first lint scan.
---

# First run

In this lesson, you will install the published CLI packages, format a small
Daml file, and scan it with `daml-lint`.

You will work in a temporary directory so no existing project files change.

## Prerequisites

You need Node.js 18 or newer and a shell. This tutorial uses the npm packages
because they install the CLIs without requiring a Rust toolchain or repository
checkout.

## Create a temporary project

Create a lesson directory and initialize npm metadata:

```sh
mkdir -p /tmp/daml-tools-first-run
cd /tmp/daml-tools-first-run
npm init -y
```

Install the formatter and linter as dev dependencies:

```sh
npm install --save-dev @daml-tools/daml-fmt @daml-tools/daml-lint
```

## Create a small Daml file

Create a Daml file:

```sh
cat > Iou.daml <<'EOF_DAML'
module Tutorial.FirstRun where

template Iou
  with
    issuer : Party
    owner : Party
    amount : Decimal
  where
    signatory issuer
    observer owner
EOF_DAML
```

The sample is intentionally small and has one lint finding: a `Decimal` field
without an `ensure` clause.

## Preview formatted output

Run `daml-fmt` without changing the file:

```sh
npx daml-fmt Iou.daml
```

The formatted source is printed to stdout. The file on disk is unchanged.

## Format the file in place

Rewrite the file:

```sh
npx daml-fmt --write Iou.daml
```

Check that the file is now formatted:

```sh
npx daml-fmt --check Iou.daml
```

A formatted file exits with code `0`.

## Scan the file

Run `daml-lint` and keep the command successful by failing only on critical
findings:

```sh
npx daml-lint Iou.daml --fail-on critical
```

The report is printed in Markdown. The sample template has a `Decimal` field
without an `ensure` clause, so the scanner reports a finding. Because this run
fails only on critical findings, the command exits successfully.

Now run the same scan as a stricter gate:

```sh
npx daml-lint Iou.daml --fail-on high
```

This command is expected to exit with code `1`, because the sample has a
high-severity finding.

## Finish

You have installed the published CLI packages, formatted a Daml file, checked
formatting, and scanned the file with a lint threshold.

For task-focused usage, see:

- [Format Daml source](../how-to/format-daml.md)
- [Scan Daml source](../how-to/scan-daml.md)
