# daml-finance test corpus

Vendored copy of the `.daml` sources from
[digital-asset/daml-finance](https://github.com/digital-asset/daml-finance)
(snapshot taken June 2026, SDK 3.4.11 era; exact upstream commit not
recorded), used as a real-world ground-truth corpus for parser and detector
integration tests in `crates/daml-parser/src/{layout.rs,span_tests.rs}` and
`crates/daml-lint/src/corpus_tests.rs`.

Only `*.daml` files are included (634 files, including the upstream
`docs/` example sources); build metadata and non-Daml artifacts are
stripped.

These files are Copyright (c) Digital Asset (Switzerland) GmbH and/or its
affiliates, licensed under Apache-2.0 (most files carry an SPDX header;
the docs example files without one are from the same upstream repository).
They are test data only. Living at the workspace root (outside any crate
directory), they are never part of a published crate's tarball.
