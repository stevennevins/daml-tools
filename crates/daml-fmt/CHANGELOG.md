# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.7.2](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.7.1...daml-fmt-v0.7.2) - 2026-06-26

### Added

- *(config)* add rule selection yaml

## [0.7.1](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.7.0...daml-fmt-v0.7.1) - 2026-06-25

### Other

- updated the following local packages: daml-parser, daml-syntax

## [0.7.0](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.6.0...daml-fmt-v0.7.0) - 2026-06-25

### Added

- *(api)* add formatter behavior tests for library calls, layout fixtures, coverage failures, and corpus span preservation.

### Changed

- *(api)* [**breaking**] update formatter internals and public behavior to consume the typed `daml-parser 0.9.0` span and field-assignment APIs.
- *(api)* [**breaking**] update formatter diagnostics and source mapping to consume the typed `daml-syntax 0.8.0` coordinate APIs.
- *(api)* keep formatter output stable while replacing ambiguous raw byte/line counters in layout code with typed internal coordinates.

## [0.6.0](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.5.0...daml-fmt-v0.6.0) - 2026-06-24

### Added

- *(api)* [**breaking**] improve Rust crate type APIs

### Added

- *(daml-fmt)* typed [`FormatDiagnostic`] / [`FormatError`] API and fallible `try_format_source*` entry points
- *(daml-fmt)* document `FormatOptions` construction style and exhaustive-struct API posture

### Changed

- *(daml-fmt)* `lex_diagnostics` and `source_diagnostics` now return typed diagnostics instead of formatted strings
- *(daml-fmt)* `coverage` dev tool exits non-zero on unreadable inputs and when no `.daml` files are found
- *(daml-fmt)* add shared `homepage` and docs.rs `documentation` metadata.

## [0.4.2](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.4.1...daml-fmt-v0.4.2) - 2026-06-24

### Fixed

- *(rust)* tighten parser spans and lint input errors

## [0.4.1](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.4.0...daml-fmt-v0.4.1) - 2026-06-23

### Added

- *(rust)* improve crate API ergonomics

### Other

- *(rust)* complete deferred API cleanup
- *(rust)* strengthen parser and lint types

## [0.2.15](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.2.14...daml-fmt-v0.2.15) - 2026-06-23

### Other

- updated the following local packages: daml-syntax

## [0.2.14](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.2.13...daml-fmt-v0.2.14) - 2026-06-23

### Other

- remove formatter scoreboard references ([#75](https://github.com/stevennevins/daml-tools/pull/75))
- *(parser)* improve public API quality ([#68](https://github.com/stevennevins/daml-tools/pull/68))

## [0.2.13](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.2.12...daml-fmt-v0.2.13) - 2026-06-23

### Other

- updated the following local packages: daml-parser, daml-syntax

## [0.2.12](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.2.11...daml-fmt-v0.2.12) - 2026-06-23

### Other

- updated the following local packages: daml-parser, daml-syntax

## [0.2.11](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.2.10...daml-fmt-v0.2.11) - 2026-06-22

### Other

- updated the following local packages: daml-parser, daml-syntax

## [0.2.9](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.2.8...daml-fmt-v0.2.9) - 2026-06-22

### Added

- *(syntax)* add shared source surface ([#56](https://github.com/stevennevins/daml-tools/pull/56))

### Other

- [codex] expand daml formatter layouts ([#54](https://github.com/stevennevins/daml-tools/pull/54))

## [0.2.7](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.2.6...daml-fmt-v0.2.7) - 2026-06-19

### Other

- release daml-fmt 0.2.7

## [0.2.6](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.2.5...daml-fmt-v0.2.6) - 2026-06-18

### Fixed

- *(npm)* migrate CLI publishing to cargo-npm

## [0.2.5](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.2.4...daml-fmt-v0.2.5) - 2026-06-18

### Fixed

- support npm CLI installs on Linux arm64

## [0.2.4](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.2.3...daml-fmt-v0.2.4) - 2026-06-18

### Fixed

- harden npm registry verification workflow

## [0.2.3](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.2.2...daml-fmt-v0.2.3) - 2026-06-18

### Fixed

- harden npm CLI release flow

## [0.2.2](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.2.1...daml-fmt-v0.2.2) - 2026-06-18

### Added

- distribute daml CLIs through npm

## [0.2.1](https://github.com/stevennevins/daml-tools/compare/daml-fmt-v0.2.0...daml-fmt-v0.2.1) - 2026-06-17

### Added

- *(daml-fmt)* expand structural layout coverage ([#8](https://github.com/stevennevins/daml-tools/pull/8))
- *(daml-fmt)* add audit workflow

### Other

- improve crate readmes ([#12](https://github.com/stevennevins/daml-tools/pull/12))
