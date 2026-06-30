---
layout: home

hero:
  name: daml-tools
  text: Pure-Rust tooling for Daml
  tagline: Consumer docs for the parser, linter, formatter, and custom-rule packages.
  actions:
    - theme: brand
      text: First run tutorial
      link: /tutorials/first-run
    - theme: alt
      text: Crate reference
      link: /reference/crates

features:
  - title: Tutorials
    details: Guided first passes — run the CLIs, build a parser tool, or author a custom lint rule from published packages.
    link: /tutorials/first-run
  - title: How-to guides
    details: Task-focused commands for formatting and scanning Daml projects.
    link: /how-to/format-daml
  - title: Reference
    details: CLI options, workspace packages, and the daml-lint custom-rule contract. Rust APIs live on docs.rs.
    link: /reference/cli
  - title: Explanation
    details: Why the packages are split into parser, syntax, lint, and formatter crates.
    link: /explanation/workspace-architecture
---

## Documentation map

| Kind | Start here |
|------|------------|
| Tutorials | [First run](tutorials/first-run.md) |
| How-to | [Format Daml source](how-to/format-daml.md) · [Scan Daml source](how-to/scan-daml.md) |
| Reference | [CLI](reference/cli.md) · [Crates and npm packages](reference/crates.md) |
| Explanation | [Workspace architecture](explanation/workspace-architecture.md) |

Rust API documentation is published on [docs.rs](https://docs.rs/releases/search?query=daml-).
Maintainer setup, local CI, and release runbooks live only in the
[GitHub repository](https://github.com/stevennevins/daml-tools/tree/main/developer-docs).
