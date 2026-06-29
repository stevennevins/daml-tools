# Developer documentation

This directory contains repository-only documentation for people changing,
validating, and releasing `daml-tools` itself. It is intentionally outside the
VitePress `docs/` tree so these maintainer runbooks are not published on the
consumer documentation site.

Published consumer documentation remains in [`docs/`](../docs/) and on
[GitHub Pages](https://stevennevins.github.io/daml-tools/).

## Repository runbooks

Use these when you are working on the repo rather than consuming the published
packages:

- [Run CI locally with mise, act, and gh-signoff](how-to/local-ci.md)
- [Release the workspace](how-to/release.md)
- [Verify a formatter change](how-to/verify-formatter-change.md)

## Published docs source

The VitePress site lives in [`docs/`](../docs/):

- [`docs/index.md`](../docs/index.md) — published site home
- [`docs/.vitepress/config.ts`](../docs/.vitepress/config.ts) — navigation, search, and GitHub Pages base path
- [`docs/package.json`](../docs/package.json) — `npm run dev`, `npm run build`, `npm run preview`

Local preview from the repository root:

```sh
npm ci --prefix docs
npm run dev --prefix docs
```

Rust API documentation remains on [docs.rs](https://docs.rs/releases/search?query=daml-).
Crate READMEs stay the crates.io, docs.rs, and GitHub package entry points.
