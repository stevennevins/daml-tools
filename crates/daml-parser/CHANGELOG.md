# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- *(api)* [**breaking**] model top-level fixity declarations as `Decl::Fixity`, operator signatures/equations as `Decl::Function`, and pattern synonyms as `Decl::UnsupportedSyntax` with `UnsupportedSyntaxKind::PatternSynonym`.

## [0.9.0](https://github.com/stevennevins/daml-tools/compare/daml-parser-v0.8.0...daml-parser-v0.9.0) - 2026-06-25

### Added

- *(api)* [**breaking**] add typed byte-offset surfaces for parser spans: `ByteOffset`, `ByteSpan`, `Span::from_usize`, `Span::start_usize`, and `Span::end_usize`.
- *(api)* [**breaking**] add typed parser diagnostics with `ParseDiagnosticKind`, `ExpectedToken`, `TypeAnnotationContext`, `MalformedSyntaxKind`, `SkippedDeclarationReason`, `UnsupportedSyntaxKind`, and strict parsing via `parse_module_strict`.
- *(api)* add interop trait impls for parser domain types including identifiers, operators, module names, positions, and spans.

### Changed

- *(api)* [**breaking**] `Span::new` now accepts typed `ByteOffset` values instead of raw `usize` offsets.
- *(api)* [**breaking**] record updates now model explicit assignments, puns, and wildcards with `FieldAssign::{Assign, Pun, Wildcard}` instead of a struct with nullable value fields.
- *(api)* [**breaking**] parser diagnostics now expose typed diagnostic kinds for logic and keep the human message for presentation.

## [0.8.0](https://github.com/stevennevins/daml-tools/compare/daml-parser-v0.7.0...daml-parser-v0.8.0) - 2026-06-24

### Added

- *(api)* [**breaking**] improve Rust crate type APIs

### Changed

- Add shared `homepage` and docs.rs `documentation` metadata.

## [0.6.2](https://github.com/stevennevins/daml-tools/compare/daml-parser-v0.6.1...daml-parser-v0.6.2) - 2026-06-24

### Fixed

- *(rust)* tighten parser spans and lint input errors

## [0.6.1](https://github.com/stevennevins/daml-tools/compare/daml-parser-v0.6.0...daml-parser-v0.6.1) - 2026-06-23

### Added

- *(rust)* improve diagnostic and lint APIs
- *(rust)* improve crate API ergonomics

### Other

- *(rust)* complete deferred API cleanup
- *(rust)* strengthen parser and lint types

## [0.3.2](https://github.com/stevennevins/daml-tools/compare/daml-parser-v0.3.1...daml-parser-v0.3.2) - 2026-06-23

### Other

- *(parser)* apply audit-backed quality fixes ([#66](https://github.com/stevennevins/daml-tools/pull/66))

## [0.3.1](https://github.com/stevennevins/daml-tools/compare/daml-parser-v0.3.0...daml-parser-v0.3.1) - 2026-06-23

### Fixed

- *(daml-parser)* harden parser diagnostics ([#62](https://github.com/stevennevins/daml-tools/pull/62))

### Other

- *(parser)* apply audit-backed quality fixes ([#64](https://github.com/stevennevins/daml-tools/pull/64))

## [0.3.0](https://github.com/stevennevins/daml-tools/compare/daml-parser-v0.2.4...daml-parser-v0.3.0) - 2026-06-22

### Changed

- **Breaking:** Document the parser public API and strengthen audit-flagged tests ([#60](https://github.com/stevennevins/daml-tools/pull/60))

## [0.2.4](https://github.com/stevennevins/daml-tools/compare/daml-parser-v0.2.3...daml-parser-v0.2.4) - 2026-06-22

### Changed

- Tighten workspace Clippy lint coverage ([#59](https://github.com/stevennevins/daml-tools/pull/59))

## [0.2.3](https://github.com/stevennevins/daml-tools/compare/daml-parser-v0.2.2...daml-parser-v0.2.3) - 2026-06-18

### Changed

- Release daml-parser 0.2.3.

## [0.2.2](https://github.com/stevennevins/daml-tools/compare/daml-parser-v0.2.1...daml-parser-v0.2.2) - 2026-06-19

### Changed

- Release daml-parser 0.2.2.

## [0.2.1](https://github.com/stevennevins/daml-tools/compare/daml-parser-v0.2.0...daml-parser-v0.2.1) - 2026-06-17

### Changed

- improve crate readmes ([#12](https://github.com/stevennevins/daml-tools/pull/12))
