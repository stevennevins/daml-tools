# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- *(daml-lint)* `parse_daml_with_diagnostics` returns named `ParseResult` with stable `ParseDiagnosticCategory` tags instead of a tuple and parser-internal categories.
- *(daml-lint)* `.daml-lint.json` rule settings accept only canonical severities (`off`, `critical`, `high`, `medium`, `low`, `info`); legacy `warn`/`error` aliases and numeric shortcuts are rejected.
- *(daml-lint)* severity thresholds and report ordering use explicit `Severity::rank` / `meets_or_exceeds` semantics.

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
