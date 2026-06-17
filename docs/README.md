# daml-tools documentation

This documentation is organized by the kind of help you need.

The crate READMEs remain the package entry points for crates.io and GitHub. This
directory is the cross-workspace documentation map for users and contributors
who need to move between `daml-parser`, `daml-lint`, and `daml-fmt`.

## Start here

Use the tutorial when you want a guided first pass through the tools:

- [Run the tools on a small Daml module](tutorials/first-run.md)
- [Write a daml-lint custom rule](tutorials/write-a-daml-lint-custom-rule.md)

## How-to guides

Use these when you already know the result you want and need the commands:

- [Format Daml source](how-to/format-daml.md)
- [Scan Daml source](how-to/scan-daml.md)
- [Release the workspace](how-to/release.md)
- [Verify a formatter change](how-to/verify-formatter-change.md)

## Reference

Use reference pages when you need neutral facts about commands or crate
surfaces:

- [CLI reference](reference/cli.md)
- [Crate reference](reference/crates.md)
- [daml-lint custom rule contract](reference/daml-lint-custom-rule-contract.md)

## Explanation

Use explanation pages when you want the reasoning behind the repo structure:

- [Workspace architecture](explanation/workspace-architecture.md)
- [Formatter verification model](explanation/formatter-verification.md)
- [daml-lint rule authoring model](explanation/daml-lint-rule-authoring.md)
