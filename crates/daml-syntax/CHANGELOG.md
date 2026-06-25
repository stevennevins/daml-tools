# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.8.1](https://github.com/stevennevins/daml-tools/compare/daml-syntax-v0.8.0...daml-syntax-v0.8.1) - 2026-06-25

### Other

- updated the following local packages: daml-parser

## [0.8.0](https://github.com/stevennevins/daml-tools/compare/daml-syntax-v0.7.0...daml-syntax-v0.8.0) - 2026-06-25

### Added

- *(api)* [**breaking**] add typed coordinate error reporting with `CoordinateRangeError`, `CoordinateRangeErrorKind`, and `InvalidOneBasedCoordinate`.
- *(api)* add `Utf16Range` so JavaScript-style UTF-16 ranges are not represented as ambiguous offset pairs.
- *(api)* add explicit `From`/`TryFrom` conversions and inherent `get` methods for coordinate newtypes.

### Changed

- *(api)* [**breaking**] remove the shared `Coordinate` trait; use each coordinate type's inherent `get` method or `usize::from(...)`.
- *(api)* [**breaking**] `LineIndex::utf16_col` now returns `Result<Utf16Offset, CoordinateRangeError>` instead of clamping out-of-range line/column inputs.
- *(api)* [**breaking**] `LineIndex::utf16_range` now returns `Utf16Range` instead of an unnamed offset tuple.
- *(api)* [**breaking**] diagnostics now report end-column state with typed variants instead of a nullable end column.

## [0.7.0](https://github.com/stevennevins/daml-tools/compare/daml-syntax-v0.6.0...daml-syntax-v0.7.0) - 2026-06-24

### Added

- *(api)* [**breaking**] improve Rust crate type APIs

### Changed

- Align crate package metadata with workspace inheritance: `homepage`, docs.rs
  `documentation` URL, and AGPL `LICENSE` in the published tarball.

## [0.4.2](https://github.com/stevennevins/daml-tools/compare/daml-syntax-v0.4.1...daml-syntax-v0.4.2) - 2026-06-24

### Fixed

- *(rust)* tighten parser spans and lint input errors

## [0.4.1](https://github.com/stevennevins/daml-tools/compare/daml-syntax-v0.4.0...daml-syntax-v0.4.1) - 2026-06-23

### Added

- *(rust)* improve diagnostic and lint APIs

## [0.1.5](https://github.com/stevennevins/daml-tools/compare/daml-syntax-v0.1.4...daml-syntax-v0.1.5) - 2026-06-23

### Fixed

- *(syntax)* avoid utf8 boundary panic ([#80](https://github.com/stevennevins/daml-tools/pull/80))

## [0.1.4](https://github.com/stevennevins/daml-tools/compare/daml-syntax-v0.1.3...daml-syntax-v0.1.4) - 2026-06-23

### Other

- *(parser)* improve public API quality ([#68](https://github.com/stevennevins/daml-tools/pull/68))

## [0.1.3](https://github.com/stevennevins/daml-tools/compare/daml-syntax-v0.1.2...daml-syntax-v0.1.3) - 2026-06-23

### Other

- updated the following local packages: daml-parser

## [0.1.2](https://github.com/stevennevins/daml-tools/compare/daml-syntax-v0.1.1...daml-syntax-v0.1.2) - 2026-06-23

### Other

- updated the following local packages: daml-parser

## [0.1.1](https://github.com/stevennevins/daml-tools/compare/daml-syntax-v0.1.0...daml-syntax-v0.1.1) - 2026-06-22

### Other

- updated the following local packages: daml-parser
