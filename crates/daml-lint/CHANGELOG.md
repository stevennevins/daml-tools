# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.9.4](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.9.3...daml-lint-v0.9.4) - 2026-06-27

### Fixed

- *(release)* address pre-public blockers

## [0.9.3](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.9.2...daml-lint-v0.9.3) - 2026-06-27

### Added

- *(parser)* implement source parser audit gaps

### Other

- *(p0)* add corpus and fixture safety coverage ([#118](https://github.com/stevennevins/daml-tools/pull/118))

### Added

- *(ir)* [**breaking**] `Choice.authority_exprs` (`ir_version: 5`).
- *(ir)* [**breaking**] `InterfaceInstance.view_expr` distinct from `methods` (`ir_version: 6`).
- *(ir)* [**breaking**] `CaseAlt.branches`, `CaseBranch.guards`, `CaseGuard`, and `where_bindings` (`ir_version: 7`).
- *(ir)* [**breaking**] `Import.package_label` for package-qualified imports (`ir_version: 8`).

## [0.9.2](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.9.1...daml-lint-v0.9.2) - 2026-06-26

### Added

- *(config)* add rule selection yaml

## [0.9.1](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.9.0...daml-lint-v0.9.1) - 2026-06-25

### Other

- updated the following local packages: daml-parser, daml-syntax

## [0.9.0](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.8.1...daml-lint-v0.9.0) - 2026-06-25

### Added

- *(api)* [**breaking**] expose lint locations with `LineNumber` and `CharColumn` newtypes instead of raw `usize` coordinates.
- *(api)* add typed error chaining for recoverable configuration and detector failures via `ConfigError::source` and `DetectError::with_source`.
- *(api)* add typed parser/linting contracts for custom-rule runtime behavior, detector wrappers, parser lowering, and reporter output.

### Changed

- *(api)* [**breaking**] `Finding` and `FindingLocation` now use typed source coordinates; construct locations with `FindingLocation::new(file, LineNumber, CharColumn)`.
- *(api)* [**breaking**] `DetectError` now preserves an optional source error and no longer implements `Clone`, `PartialEq`, or `Eq`.
- *(api)* [**breaking**] `ConfiguredDetector::new` is no longer part of the public construction API; configure detectors through the documented lint configuration path.
- *(api)* [**breaking**] detector wrapper failures now retain the wrapped detector error as the source instead of flattening it to a string.
- *(api)* update dependencies on the breaking `daml-parser 0.9.0` and `daml-syntax 0.8.0` APIs.

## [0.8.1](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.8.0...daml-lint-v0.8.1) - 2026-06-24

### Other

- updated the following local packages: daml-parser, daml-syntax

### Changed

- *(daml-lint)* `parse_daml_with_diagnostics` returns named `ParseResult` with stable `ParseDiagnosticCategory` tags instead of a tuple and parser-internal categories.
- *(daml-lint)* `.daml-lint.json` rule settings accept only canonical severities (`off`, `critical`, `high`, `medium`, `low`, `info`); legacy `warn`/`error` aliases and numeric shortcuts are rejected.
- *(daml-lint)* severity thresholds and report ordering use explicit `Severity::rank` / `meets_or_exceeds` semantics.
- *(daml-lint)* tighten crates.io package contents: exclude lint-plugin npm metadata, rule TypeScript sources, and release sync scripts while keeping embedded `rules/*.js` and custom-rule `examples/`. Add shared `homepage` and docs.rs `documentation` metadata.

## [0.6.2](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.6.1...daml-lint-v0.6.2) - 2026-06-24

### Fixed

- *(rust)* tighten parser spans and lint input errors

## [0.6.1](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.6.0...daml-lint-v0.6.1) - 2026-06-23

### Added

- *(rust)* improve diagnostic and lint APIs

## [0.3.18](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.3.17...daml-lint-v0.3.18) - 2026-06-23

### Other

- updated the following local packages: daml-syntax

## [0.3.17](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.3.16...daml-lint-v0.3.17) - 2026-06-23

### Other

- *(parser)* improve public API quality ([#68](https://github.com/stevennevins/daml-tools/pull/68))

## [0.3.16](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.3.15...daml-lint-v0.3.16) - 2026-06-23

### Other

- updated the following local packages: daml-parser, daml-syntax

## [0.3.15](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.3.14...daml-lint-v0.3.15) - 2026-06-23

### Other

- updated the following local packages: daml-parser, daml-syntax

## [0.3.14](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.3.13...daml-lint-v0.3.14) - 2026-06-22

### Other

- updated the following local packages: daml-parser, daml-syntax

## [0.3.12](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.3.11...daml-lint-v0.3.12) - 2026-06-22

### Added

- *(syntax)* add shared source surface ([#56](https://github.com/stevennevins/daml-tools/pull/56))

## [0.3.9](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.3.8...daml-lint-v0.3.9) - 2026-06-19

### Other

- release daml-lint 0.3.9

## [0.3.8](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.3.7...daml-lint-v0.3.8) - 2026-06-18

### Fixed

- *(npm)* migrate CLI publishing to cargo-npm

## [0.3.7](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.3.6...daml-lint-v0.3.7) - 2026-06-18

### Fixed

- support npm CLI installs on Linux arm64
- install daml-lint in the custom rule template

## [0.3.6](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.3.5...daml-lint-v0.3.6) - 2026-06-18

### Fixed

- harden npm registry verification workflow

## [0.3.5](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.3.4...daml-lint-v0.3.5) - 2026-06-18

### Fixed

- harden npm CLI release flow

## [0.3.4](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.3.3...daml-lint-v0.3.4) - 2026-06-18

### Added

- distribute daml CLIs through npm

## [0.3.3](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.3.2...daml-lint-v0.3.3) - 2026-06-17

### Added

- *(daml-lint)* load plugin manifests from config

## [0.3.2](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.3.1...daml-lint-v0.3.2) - 2026-06-17

### Fixed

- *(daml-lint)* sync release metadata versions

### Other

- harden release-plz release pr flow
- document release process

## [0.3.1](https://github.com/stevennevins/daml-tools/compare/daml-lint-v0.3.0...daml-lint-v0.3.1) - 2026-06-17

### Other

- harden release publishing
- [codex] Add daml-lint custom rule plugin support ([#20](https://github.com/stevennevins/daml-tools/pull/20))
- *(daml-lint)* import custom rule types explicitly
