# daml-tools documentation

Published site: [https://stevennevins.github.io/daml-tools/](https://stevennevins.github.io/daml-tools/)

This directory is the cross-workspace documentation for users and contributors
moving between `daml-parser`, `daml-lint`, and `daml-fmt`. Content follows the
[Diataxis](https://diataxis.fr/) model: tutorials, how-to guides, reference,
and explanation.

## Site source

The VitePress site lives here:

- [`index.md`](index.md) — site home
- [`.vitepress/config.ts`](.vitepress/config.ts) — navigation, search, and GitHub Pages base path
- [`package.json`](package.json) — `npm run dev`, `npm run build`, `npm run preview`

Local preview from the repository root:

```sh
npm ci --prefix docs
npm run dev --prefix docs
```

Rust API documentation remains on [docs.rs](https://docs.rs/releases/search?query=daml-).
Crate READMEs stay the crates.io and GitHub package entry points.

## Start here

Use the tutorial when you want a guided first pass through the tools:

- [Run the tools on a small Daml module](tutorials/first-run.md)
- [Build a tiny parser tool](tutorials/build-a-parser-tool.md)
- [Write a daml-lint custom rule](tutorials/write-a-daml-lint-custom-rule.md)

## How-to guides

Use these when you already know the result you want and need the commands:

- [Format Daml source](how-to/format-daml.md)
- [Scan Daml source](how-to/scan-daml.md)
- [Run CI locally with mise, act, and gh-signoff](how-to/local-ci.md)
- [Release the workspace](how-to/release.md)
- [Verify a formatter change](how-to/verify-formatter-change.md)

## Reference

Use reference pages when you need neutral facts about commands or crate
surfaces:

- [CLI reference](reference/cli.md)
- [Crate and package reference](reference/crates.md)
- [daml-lint custom rule contract](reference/daml-lint-custom-rule-contract.md)

## Explanation

Use explanation pages when you want the reasoning behind the repo structure:

- [Workspace architecture](explanation/workspace-architecture.md)
- [Formatter verification model](explanation/formatter-verification.md)
- [daml-lint rule authoring model](explanation/daml-lint-rule-authoring.md)
