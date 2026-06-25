# Rust Idiom Package Audit

Generated from persisted Smithers audit outputs for run `4b61f365-4f31-4f62-a465-99726fd362e3`.

## Summary

- Package/category tasks: 16
- Findings: 66
- Actionable findings: 64
- High-priority findings: 1
- Blocked/partial slices: 0

## Actionable implementation slices

### daml-parser / type-safety

- daml-parser-type-safety-001 (medium, high): Replace the sentinel/Option encoding with a domain enum such as RecordField::Assign { name, value }, RecordField::Pun { name }, and RecordField::Wildcard, or add a FieldAssignKind enum. This would make wildcard versus pun explicit and remove the need for Identifier('..').
- daml-parser-type-safety-002 (medium, high): Model the section shape directly, for example OperatorReference { op }, LeftSection { op, operand }, and RightSection { op, operand }, or introduce a SectionOperand/SectionKind enum that cannot represent None+Left.
- daml-parser-type-safety-003 (medium, high): Introduce parser-owned coordinate newtypes or private-field wrappers, such as ByteOffset, ByteSpan, LineNumber, and CharColumn, and expose Token::span()/Trivia::span() returning a byte-span type. Keep raw usize extraction explicit for slicing. This can be planned as a breaking 0.x API cleanup because the README already says public AST shape changes are SemVer-relevant.
- daml-parser-type-safety-004 (low, medium): For public AST fields where absence and malformed input are distinct downstream states, consider a small enum such as TypeAnnotation::Present(Type), TypeAnnotation::Absent, TypeAnnotation::Malformed { span } or TypeParseState. Keep diagnostics as the detailed error channel, but avoid collapsing semantically different states into None.
- daml-parser-type-safety-005 (low, medium): Decide and document whether these are purely tagged strings or validated lexical domain types. If invariants are intended, replace or supplement From<&str>/From<String> with TryFrom plus meaningful validation errors, and reserve unchecked constructors for parser-internal or test-only construction. If unchecked construction is intentional, document the relaxed invariant in rustdoc to avoid misleading downstream callers.

### daml-parser / error-handling

- daml-parser-error-handling-001 (medium, high): Introduce a typed diagnostic detail/kind alongside the human message, such as ParseDiagnosticKind variants for lexical errors, expected-token failures, malformed type annotations, unsupported syntax, and recursion limits. Preserve LexErrorKind or LexError-derived structured data for lexical diagnostics, and consider Display/Error impls on ParseDiagnostic if it is intended to be nested under ParseModuleError. Keep message() as presentation, not the machine-readable contract.
- daml-parser-error-handling-002 (low, high): Rewrite fallible public examples to use hidden `fn main() -> Result<(), Box<dyn std::error::Error>>` plus `?` for success paths. For expected-error examples, prefer explicit `match`, `assert!(result.is_err())`, or a small helper that avoids suggesting unwrap as normal handling.

### daml-parser / interoperability

- daml-parser-interoperability-001 (low, high): Add impl std::fmt::Display for DiagnosticCategory that delegates to as_str(). Keep as_str() for stable machine-readable tags, but let user-facing formatting use the standard Display path.
- daml-parser-interoperability-002 (info, medium): Consider adding Display for TokenKind only if downstream users need normalized token spelling in diagnostics/logs. Document that it is not a lossless source renderer and keep render_lossless as the source-exact API. If no public need exists, leave this as-is.
- daml-parser-interoperability-003 (info, high): Derive PartialEq and Eq for ParseModuleResult to match the lexer output wrappers and make public API contract tests/downstream assertions more ergonomic.

### daml-parser / documentation

- daml-parser-doc-001 (medium, high): Update the README dependency snippet to the current published package line, or use a release-maintained placeholder/process so the install example changes with the crate version.
- daml-parser-doc-002 (medium, high): Narrow the wording to distinguish AST families: declarations/expressions/patterns generally carry `Pos` plus `Span`, while `Type` nodes carry `Span` only. Add this contract near `Type` and in the README source-position section.
- daml-parser-doc-003 (medium, high): Add targeted rustdoc to public AST DTOs and fields explaining ordering, optionality (`None` meanings), raw-preservation variants, parser-created construction expectations, and `pos`/`span` semantics. Prioritize shapes consumed by daml-lint and daml-fmt.
- daml-parser-doc-005 (medium, high): Document lexer DTO fields and variants with the observable contracts from tests: decoded literal values versus source spans, trivia inclusion/exclusion, ordering, tab/position semantics, and what each recoverable lexical error means.
- daml-parser-doc-004 (low, high): Document `ParseModuleResult` and its fields directly: `module` is always present and may be partial; `diagnostics` are source-ordered recoverable parse/lex issues; use `has_errors`, `into_parts`, or `into_result` depending on tolerance.
- daml-parser-doc-006 (low, high): Add one-line rustdoc to each public error variant explaining the invariant violation and whether byte fields are source offsets, interval endpoints, or diagnostic payloads. Keep the function-level `# Errors` sections as the overview.
- daml-parser-doc-007 (low, medium): Where examples call fallible APIs, wrap them in a hidden `fn main() -> Result<(), Box<dyn std::error::Error>>` and use `?`, or make the text explicit that the unwrap/expect is only asserting the example's fixed input. Keep `unwrap_err` only where demonstrating expected failure is the point.

### daml-syntax / type-safety

- daml-syntax-type-safety-001 (medium, high): Replace or supplement Option<CharColumn> with a small public enum or struct that names the cases, for example SameLineEnd(CharColumn), Multiline, and EmptySpan, or document and test a deliberately narrower invariant if the distinction is intentionally not exposed.
- daml-syntax-type-safety-002 (low, high): Introduce a named public Utf16Range or Utf16Span struct with start and end fields/accessors. That keeps the UTF-16 coordinate-space newtype while also encoding the range endpoints by name, mirroring the clarity already provided by TextRange for byte ranges.
- daml-syntax-type-safety-003 (low, medium): Expose the checked source length as a typed byte-space value, such as source_len_bytes() -> ByteOffset for the EOF byte offset or a new ByteLen newtype if length-vs-offset should remain distinct. Keep raw usize extraction explicit only at final interop boundaries.

### daml-syntax / error-handling

- daml-syntax-error-handling-001 (medium, high): Make the out-of-range contract explicit. Prefer a fallible API such as `try_utf16_col(...) -> Result<Utf16Offset, CoordinateRangeError>` with typed variants for invalid line/column, or change the existing API in a clean break if that is the intended public contract. If silent clamping is intentional, document it precisely and add tests for line-past-EOF and column-past-line/source behavior.
- daml-syntax-error-handling-002 (low, high): Keep the typed `kind()` accessor, but make Display messages kind-specific, for example by saying that the span is out of bounds, inverted, or not on a UTF-8 boundary. Update the tests to lock in those more actionable messages.
- daml-syntax-error-handling-003 (low, medium): Consider adding standard `TryFrom<usize>` implementations for the one-based coordinate newtypes with a small typed error such as `InvalidOneBasedCoordinate`, while retaining or documenting the Option-returning constructor as a NonZero-style convenience. If the project intentionally prefers Option for the single zero case, document that rationale and leave the API unchanged.

### daml-syntax / interoperability

- daml-syntax-interoperability-001 (medium, high): Add standard conversion impls where semantically valid: TryFrom<usize> for one-based coordinates with a small meaningful error type for zero, From<LineNumber/ByteColumn/CharColumn/ByteOffset/Utf16Offset> for usize if explicit raw extraction remains intended, and possibly From<usize> for zero-based coordinates. Keep new()/try_new() only if they remain useful constructors, but make standard conversions the interoperable path used in examples/tests.
- daml-syntax-interoperability-002 (medium, high): Implement Display for ParserSpanToTextRangeErrorKind and include the specific kind text in ParserSpanToTextRangeError's Display output, while keeping the typed kind() accessor. Update tests to assert distinct messages for out-of-bounds, inverted, non-UTF-8-boundary, and TextSize overflow failures.
- daml-syntax-interoperability-003 (low, medium): Consider implementing Clone, PartialEq, and Eq for SourceTokens and SourceFile if equality can intentionally ignore or normalize lazy OnceLock caches. Avoid a naive derive if initialized vs uninitialized cache state would make semantically identical values compare unequal; use manual impls if needed.
- daml-syntax-interoperability-004 (low, high): Prefer typed accessors plus standard conversions, e.g. usize::from(err.span_start()) after adding From<ByteOffset> for usize. If keeping span_start_usize/span_end_usize for ergonomics, document them as convenience wrappers rather than the primary conversion path and update tests to cover the standard conversion path.

### daml-syntax / documentation

- daml-syntax-doc-001 (medium, high): Clarify the public contract: document that `end_column` is an exclusive 1-based Unicode-scalar end column, present only for non-empty single-line diagnostic spans, and absent for multi-line or zero-width spans. Add or update a public API test that pins at least one `Some(end_column)` case and one zero-width or multi-line `None` case if this behavior is intentional.
- daml-syntax-doc-002 (low, medium): Document the out-of-range contract for `utf16_col`: either state that callers must pass coordinates known to belong to this `LineIndex`, or explicitly describe the current EOF/source-length clamping behavior. If clamping is intended API behavior, add a small unit test that pins it.
- daml-syntax-doc-003 (low, high): Either include the README in rustdoc so the README code blocks themselves are tested, or rephrase the README note to say the README examples are mirrored in `src/lib.rs` doctests. This keeps the documentation process contract accurate and avoids stale README snippets being assumed covered automatically.

### daml-lint / type-safety

- daml-lint-type-safety-001 (medium, high): Introduce typed Rust-side coordinate fields or accessors for SourceSpan, reusing daml_syntax LineNumber, CharColumn, ByteOffset, Utf16Offset, or a typed SourceRange wrapper. If the JavaScript/JSON contract must remain numeric, keep serde serialization as numbers while making the Rust API prevent cross-space assignment.
- daml-lint-type-safety-002 (medium, high): Add a public SourceLocation or ReportLocation type with LineNumber and CharColumn fields, and constructors that validate non-zero coordinates. Keep existing numeric JSON/SARIF output as a serialization detail. If immediate breaking changes are too large, add typed constructors/accessors first and steer new public code away from raw usize pairs.
- daml-lint-type-safety-003 (low, medium): Make the general constructor private or replace it with a DetectorOverrides struct/builder whose methods are named by intent. Keep with_name and with_severity as the public simple paths, and validate or strongly type override names if external callers can supply them.
- daml-lint-type-safety-004 (low, high): Deprecate or change parse_severity to return Result<Severity, SeverityParseError>, or remove it from the public surface in favor of Severity::from_str/str::parse. Update script loading to use the same meaningful error path instead of losing detail through Option.

### daml-lint / error-handling

- DL-LINT-EH-001 (medium, high): Make DetectError preserve structured causes instead of flattening them into message strings. For example, add a non-exhaustive cause enum or source field such as Box<dyn Error + Send + Sync + 'static> where feasible, implement Error::source(), and keep detector/path as typed context accessors. Update ConfiguredDetector to preserve an inner DetectError as the source rather than copying only its message.
- DL-LINT-EH-002 (medium, high): Implement Error::source() for variants that already carry std::io::Error or rquickjs::Error, and store typed sources for serde_json::Error and SeverityParseError where available. For JavaScript exception text that cannot be represented as an Error source, rename the field to message/detail to avoid pretending it is a source chain. Then have DetectError preserve ScriptLoadError as its source when custom rule detection fails.
- DL-LINT-EH-004 (medium, high): Either make the panic contract explicit with a # Panics section saying detect is only for detectors whose try_detect cannot fail, or de-emphasize/deprecate detect for public library use and update docs/examples toward try_detect. If detect remains, document that custom ScriptDetector runtime errors will panic through this adapter.
- DL-LINT-EH-003 (low, high): Implement std::error::Error::source() for ConfigError by matching variants with source fields and returning Some(source). For RuleLoadFailed, return the boxed ScriptLoadError. This is a surgical internal change because ConfigError is currently binary-private.
- DL-LINT-EH-005 (low, medium): Prefer the FromStr API directly or change parse_severity to return Result<Severity, SeverityParseError> in the next breaking window. Internally, parse custom-rule severities with severity_str.parse::<Severity>() and carry SeverityParseError as a typed source in ScriptLoadError::UnknownSeverity.

### daml-lint / interoperability

- daml-lint-interoperability-001 (medium, high): Implement Display for ParseDiagnosticCategory using the same kebab-case strings as as_str(), replace from_parser_category() with or supplement it by impl From<daml_parser::ast::DiagnosticCategory>, and consider FromStr/TryFrom<&str> with a meaningful error if the documented tags are intended to be accepted from users or external data.
- daml-lint-interoperability-004 (medium, medium): Derive PartialEq consistently for the public IR DTO graph, and derive Eq where all contained fields support Eq. Consider Hash only for stable identifier-like or immutable value types where hashing the full value is semantically useful; do not add traits where equality/hash semantics would be misleading.
- daml-lint-interoperability-005 (medium, medium): Expose an in-memory or reader-based public loader, for example load_script_source(label: impl Into<String>, source: &str) and/or load_script_reader<R: std::io::Read>(label: impl Into<String>, reader: R, options: &serde_json::Value). Also consider changing path parameters to impl AsRef<Path> for call-site flexibility while keeping the existing path-based functions as convenience wrappers if compatibility is required.
- daml-lint-interoperability-002 (low, high): Prefer severity_str.parse::<Severity>() at call sites and either remove parse_severity before the next breaking release or deprecate it in favor of FromStr. If a helper remains, return Result<Severity, SeverityParseError> rather than Option.
- daml-lint-interoperability-003 (low, high): Implement Display for OutputFormat returning canonical lowercase values such as sarif, markdown, and json. This lets CLI/help/config/docs code use standard formatting instead of duplicating strings.
- daml-lint-interoperability-006 (low, medium): Implement Display for Consuming and ImportStyle using their serialized kebab-case tags. Add FromStr/TryFrom<&str> with meaningful parse errors only if Rust callers are expected to accept these tags from external rule/config data.

### daml-lint / documentation

- daml-lint-doc-001 (high, high): Add rustdoc to every public IR struct, enum, variant, and field that defines the rule-facing contract. Include Option/null semantics, 1-based position semantics, UTF-16 versus byte offsets, serde/tagged-union shape, raw/Unknown fallback behavior, and non_exhaustive matching guidance. Consider enabling at least `warn(missing_docs)` for the library once the backlog is cleared.
- daml-lint-doc-002 (medium, high): Document `ScriptLoadError` itself and each public variant/field with the user-visible condition it represents. Distinguish script load/validation failures from invocation-time visitor failures and runaway-loop interruption. If callers should not match variants exhaustively, mark the enum `#[non_exhaustive]` and document Display/accessor expectations.
- daml-lint-doc-003 (medium, high): Add a `# Panics` section to `Detector::detect` that states it panics when `try_detect` returns `Err`, and add or link an example showing library callers using `try_detect` to handle custom rule/runtime failures without panicking.
- daml-lint-doc-004 (medium, high): Document parser and reporter DTO fields with exact coordinate basis, absence semantics for `end_column`, category stability guarantees, and relationship between parser diagnostics and formatted parse errors. Add docs for `OutputFormat` variants and `ParseResult` fields so rustdoc users do not need README/tests to understand the API contract.
- daml-lint-doc-005 (low, high): Update version-specific docs to the current 0.8 line or remove patch/minor-specific wording where it will drift. Keep README, docs/reference/crates.md, and Cargo.toml synchronized as part of release checks.

### daml-fmt / type-safety

- daml-fmt-type-safety-001 (medium, high): Introduce small private domain types for the formatter coordinate spaces, for example ByteOffset(usize), LineIndex(usize), IndentColumn(usize or i64), and IndentDelta(i64), or a LineStarts abstraction with typed lookup methods. Convert daml_parser Span boundaries into ByteOffset at module boundaries and keep slicing operations centralized. This is an internal clean break, so avoid compatibility shims unless requested.
- daml-fmt-type-safety-002 (low, high): Replace the u8 with a private enum such as ImportGroup { DamlStdlib, DaLibrary, LocalOrExternal } deriving Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug. Keep the current sort order in the enum variant order or an explicit rank method, and update tests to continue asserting the group boundary behavior.
- daml-fmt-type-safety-003 (low, high): Update docs/reference/crates.md to list lex_diagnostics(src: &str) -> Vec<FormatDiagnostic>, source_diagnostics(src: &str) -> Vec<FormatDiagnostic>, try_format_source(src: &str) -> Result<String, FormatError>, and try_format_source_with_options(src: &str, options: FormatOptions) -> Result<String, FormatError>. Keep the docs aligned with the typed accessors on FormatDiagnostic and diagnostics() on FormatError.

### daml-fmt / error-handling

- DFMT-EH-001 (medium, medium): Make malformed input an explicit coverage contract. Prefer changing the public coverage path to return `Result<FormatCoverage, FormatError>` or adding a fallible `try_coverage` and using it from the coverage CLI; if best-effort recovered coverage is intentional, document that prominently and add tests for malformed input.
- DFMT-EH-002 (low, high): Change the stdin read branch to preserve and print the source error, e.g. `if let Err(e) = ... { eprintln!("daml-fmt: failed to read stdin: {e}"); exit(2); }`. If CLI error handling grows, consider a small typed CLI error enum for consistent display and exit-code mapping.
- DFMT-EH-003 (low, high): Have each dev-tool `collect` return or accumulate `io::Result`/read errors instead of panicking. Report directory traversal failures with the path and source error, increment the existing error count, and exit nonzero consistently with file read failures.
- DFMT-EH-004 (low, high): Update `docs/reference/crates.md` to list the typed diagnostics and fallible formatting functions, correct `lex_diagnostics` to `Vec<FormatDiagnostic>`, and mention that `FormatError` implements `Display` and `std::error::Error`. Align the reference with README and rustdoc.

### daml-fmt / interoperability

- daml-fmt-interoperability-001 (low, high): Add an explicit Default impl for ImportOrder returning ImportOrder::Organize, and consider a Display impl with stable user-facing labels such as organize and preserve. Add focused tests in crates/daml-fmt/tests/library_behavior.rs for ImportOrder::default and Display output if Display is made public contract.
- daml-fmt-interoperability-002 (low, medium): Keep diagnostics() for readability, but add impl AsRef<[FormatDiagnostic]> for FormatError returning self.diagnostics(). If collection-style iteration is desired, add IntoIterator for &FormatError rather than exposing construction semantics. Avoid FromIterator/Extend unless the crate explicitly defines whether an empty FormatError is valid.

### daml-fmt / documentation

- FMT-DOC-001 (medium, high): Change the try_format_source_with_options and try_format_source error docs to say they reject diagnostics as reported by source_diagnostics, explicitly documenting that CPP-conditional parser recovery diagnostics are ignored while lexical diagnostics are still errors.
- FMT-DOC-002 (medium, high): Update the daml-fmt public API table to current signatures and include the diagnostic/error types and try_format entry points, including a short note that FormatDiagnostic exposes typed line, column, category, and message accessors.
- FMT-DOC-004 (medium, high): Broaden the CLI reference to say lexical or parser diagnostics are reported on stderr and exit 2, with write mode leaving input unchanged. Consider also linking this behavior to source_diagnostics and noting the CPP-conditional parser-diagnostic exception if documented for the library API.
- FMT-DOC-003 (low, high): Update the workspace member version table for daml-fmt to 0.6.0, preferably with the other workspace versions if this table is maintained manually.
- FMT-DOC-005 (low, medium): Add short rustdoc comments to FormatOptions::new and FormatOptions::with_import_order stating that new returns the default organizing configuration and with_import_order sets the import ordering strategy, with a link back to import_order for the package-identity warning.

## High-priority findings

### daml-lint-doc-001 — HIGH — daml-lint / documentation

- Location: `crates/daml-lint/src/ir.rs:16 (rule-facing IR public DTOs)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines documentation and rustdoc missing_docs: public contracts should be documented with field/variant semantics, examples, and failure-relevant behavior; rustdoc crate docs require item-level documentation for public APIs.
- Evidence: The crate root documents the IR as public and versioned (crates/daml-lint/src/lib.rs:8-26), README says custom rule node shapes mirror src/ir.rs (crates/daml-lint/README.md:202-208), and runtime/API tests assert the structured IR contract (crates/daml-lint/tests/custom_rule_runtime_contracts.rs:132-232). However, `cargo doc` with `-D missing_docs` reports many missing public docs in ir.rs, including `Span` and fields (line 16), `SourceSpan` and fields (line 24), `TypeNode` variants/fields (line 57), `LiteralKind` variants (line 182), DTOs like `Template` (line 312), `Choice` (line 346), `Statement` variants/fields (line 381), `ImportStyle` (line 476), and `DamlModule` fields (line 510).
- Recommendation: Add rustdoc to every public IR struct, enum, variant, and field that defines the rule-facing contract. Include Option/null semantics, 1-based position semantics, UTF-16 versus byte offsets, serde/tagged-union shape, raw/Unknown fallback behavior, and non_exhaustive matching guidance. Consider enabling at least `warn(missing_docs)` for the library once the backlog is cleared.


## All findings

### daml-lint-doc-001 — HIGH — daml-lint / documentation

- Location: `crates/daml-lint/src/ir.rs:16 (rule-facing IR public DTOs)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines documentation and rustdoc missing_docs: public contracts should be documented with field/variant semantics, examples, and failure-relevant behavior; rustdoc crate docs require item-level documentation for public APIs.
- Evidence: The crate root documents the IR as public and versioned (crates/daml-lint/src/lib.rs:8-26), README says custom rule node shapes mirror src/ir.rs (crates/daml-lint/README.md:202-208), and runtime/API tests assert the structured IR contract (crates/daml-lint/tests/custom_rule_runtime_contracts.rs:132-232). However, `cargo doc` with `-D missing_docs` reports many missing public docs in ir.rs, including `Span` and fields (line 16), `SourceSpan` and fields (line 24), `TypeNode` variants/fields (line 57), `LiteralKind` variants (line 182), DTOs like `Template` (line 312), `Choice` (line 346), `Statement` variants/fields (line 381), `ImportStyle` (line 476), and `DamlModule` fields (line 510).
- Recommendation: Add rustdoc to every public IR struct, enum, variant, and field that defines the rule-facing contract. Include Option/null semantics, 1-based position semantics, UTF-16 versus byte offsets, serde/tagged-union shape, raw/Unknown fallback behavior, and non_exhaustive matching guidance. Consider enabling at least `warn(missing_docs)` for the library once the backlog is cleared.

### FMT-DOC-001 — MEDIUM — daml-fmt / documentation

- Location: `crates/daml-fmt/src/lib.rs:291 (try_format_source_with_options)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines C-FAILURE requires error conditions to be documented for Result-returning APIs; the Rust Book error-handling model treats recoverable input failures as Result errors and requires callers to know when they occur.
- Evidence: The docs say try_format_source_with_options returns FormatError with lexical or parser diagnostics, and the # Errors section says it returns FormatError when src produces lexical or parser diagnostics. However source_diagnostics special-cases CPP conditionals at lines 156-166 by returning only lex_diagnostics, tests/library_behavior.rs lines 56-59 asserts CPP parser recovery diagnostics are suppressed, and a probe showed source_diagnostics=0 and try_ok=true for CPP-conditional source.
- Recommendation: Change the try_format_source_with_options and try_format_source error docs to say they reject diagnostics as reported by source_diagnostics, explicitly documenting that CPP-conditional parser recovery diagnostics are ignored while lexical diagnostics are still errors.

### FMT-DOC-002 — MEDIUM — daml-fmt / documentation

- Location: `docs/reference/crates.md:190 (daml-fmt public library API table)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines documentation guidance requires public contracts to match the actual API surface and behavior.
- Evidence: The public API table documents lex_diagnostics(src: &str) -> Vec<String>, but crates/daml-fmt/src/lib.rs line 152 returns Vec<FormatDiagnostic>. The same table omits current public diagnostic/error APIs and DTOs: source_diagnostics, try_format_source, try_format_source_with_options, FormatDiagnostic, and FormatError, while the README lines 96-100 and lib.rs lines 66-164 and 289-315 document and expose them.
- Recommendation: Update the daml-fmt public API table to current signatures and include the diagnostic/error types and try_format entry points, including a short note that FormatDiagnostic exposes typed line, column, category, and message accessors.

### FMT-DOC-004 — MEDIUM — daml-fmt / documentation

- Location: `docs/reference/cli.md:38 (daml-fmt CLI error documentation)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines C-FAILURE applies to documented failure modes; CLI reference docs should match recoverable input errors surfaced to users.
- Evidence: The CLI reference says malformed lexical input is reported and exit code 2 means malformed lexical input. The actual CLI source documents lexical or parser diagnostics at crates/daml-fmt/src/bin/daml-fmt.rs lines 9-10, report_diagnostics calls source_diagnostics at lines 21-28, and tests/cli.rs lines 83-118 and 122-135 verify parser diagnostics exit 2 for stdin, --check, and --write without rewriting input.
- Recommendation: Broaden the CLI reference to say lexical or parser diagnostics are reported on stderr and exit 2, with write mode leaving input unchanged. Consider also linking this behavior to source_diagnostics and noting the CPP-conditional parser-diagnostic exception if documented for the library API.

### DFMT-EH-001 — MEDIUM — daml-fmt / error-handling

- Location: `crates/daml-fmt/src/lib.rs:345 (coverage)`
- Actionable: yes
- Confidence: medium
- Principle: Rust Book error handling: expected failures should return Result; malformed parser input is an expected/recoverable failure. Rust API Guidelines documentation C-FAILURE: public functions should document error behavior.
- Evidence: The public `coverage(src: &str) -> FormatCoverage` delegates to `layout_ast::coverage` without checking `source_diagnostics`. `layout_ast::coverage` parses with `SourceFile::parse(src)` at crates/daml-fmt/src/layout_ast.rs:264 and then counts modeled constructs from the recovered module. Tests exercise valid/canonical/messy coverage only in crates/daml-fmt/tests/library_behavior.rs:27 and crates/daml-fmt/tests/coverage.rs:29; there is no malformed-input coverage contract. This differs from `try_format_source_with_options`, which returns `Result<String, FormatError>` for lexical or parser diagnostics at crates/daml-fmt/src/lib.rs:297.
- Recommendation: Make malformed input an explicit coverage contract. Prefer changing the public coverage path to return `Result<FormatCoverage, FormatError>` or adding a fallible `try_coverage` and using it from the coverage CLI; if best-effort recovered coverage is intentional, document that prominently and add tests for malformed input.

### daml-fmt-type-safety-001 — MEDIUM — daml-fmt / type-safety

- Location: `crates/daml-fmt/src/layout_ast.rs:1146 (Edit)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines type safety: newtypes provide static distinctions and deliberate types should encode meaning/invariants instead of ambiguous primitives (https://rust-lang.github.io/api-guidelines/type-safety.html).
- Evidence: The core reindent model stores child_start and block_end as usize byte offsets and delta as i64 indentation delta at crates/daml-fmt/src/layout_ast.rs:1146-1150. Nearby helpers pass multiple primitive coordinate spaces in the same signatures: line_start_table returns Vec<usize> byte offsets at 1270-1274, line_of takes byte: usize and returns a usize line index at 1275-1280, push_line_edit takes line: usize plus target: i64 at 1833, push_block_edit takes first_byte/end_byte/head_line/target primitives at 1851-1859, and push_span_block_edit repeats first_byte/end_byte/target at 2628-2635. Tests cover many formatting behaviors, but the type system does not distinguish byte offsets, line indices, indentation columns, and signed indentation deltas.
- Recommendation: Introduce small private domain types for the formatter coordinate spaces, for example ByteOffset(usize), LineIndex(usize), IndentColumn(usize or i64), and IndentDelta(i64), or a LineStarts abstraction with typed lookup methods. Convert daml_parser Span boundaries into ByteOffset at module boundaries and keep slicing operations centralized. This is an internal clean break, so avoid compatibility shims unless requested.

### daml-lint-doc-002 — MEDIUM — daml-lint / documentation

- Location: `crates/daml-lint/src/detectors/script.rs:11 (ScriptLoadError)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines documentation C-FAILURE and interoperability C-GOOD-ERR: public error types and fallible APIs should document their error conditions and provide meaningful contracts for callers.
- Evidence: `load_script` and `load_script_with_options` document `# Errors` and return `ScriptLoadError` (crates/daml-lint/src/detectors/script.rs:210-240), while tests exercise missing metadata, bad severity, syntax/runtime errors, and interruption behavior (crates/daml-lint/src/detectors/script.rs:556-650). But the public `ScriptLoadError` enum and all of its variants/fields lack rustdoc (line 11), and `cargo doc -D missing_docs` reports missing docs for every variant and field plus the public `ScriptDetector` struct at line 146.
- Recommendation: Document `ScriptLoadError` itself and each public variant/field with the user-visible condition it represents. Distinguish script load/validation failures from invocation-time visitor failures and runaway-loop interruption. If callers should not match variants exhaustively, mark the enum `#[non_exhaustive]` and document Display/accessor expectations.

### daml-lint-doc-003 — MEDIUM — daml-lint / documentation

- Location: `crates/daml-lint/src/detector.rs:232 (Detector::detect)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines documentation C-FAILURE and Rust Book error handling: expected/recoverable failures should use Result, and public functions that can panic should document panic conditions.
- Evidence: `Detector::detect` is a public convenience method that calls `try_detect(module).unwrap_or_else(|e| panic!(...))` (crates/daml-lint/src/detector.rs:232-240). The surrounding docs call it a panic-first convenience layer, and crate/README docs tell library callers to use `try_detect` for rule errors (crates/daml-lint/src/lib.rs:28-33, crates/daml-lint/README.md:248-251), but the method lacks a dedicated `# Panics` section. The recoverable path is available via `try_detect` (crates/daml-lint/src/detector.rs:241-246).
- Recommendation: Add a `# Panics` section to `Detector::detect` that states it panics when `try_detect` returns `Err`, and add or link an example showing library callers using `try_detect` to handle custom rule/runtime failures without panicking.

### daml-lint-doc-004 — MEDIUM — daml-lint / documentation

- Location: `crates/daml-lint/src/parser.rs:31 (ParseDiagnostic and ParseResult)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines documentation: public DTO contracts should document fields and examples; rustdoc should make public data shapes understandable without requiring source-code inspection.
- Evidence: `parse_daml_with_diagnostics` is the documented entry point (crates/daml-lint/src/lib.rs:7, crates/daml-lint/src/parser.rs:93-104), and main/reporter callers depend on diagnostic line, column, end_column, message, and category (crates/daml-lint/src/main.rs:116-134). But `cargo doc -D missing_docs` reports missing field docs for `ParseDiagnostic` fields (crates/daml-lint/src/parser.rs:31-36), missing docs for `ParseResult` and its fields (line 88), and missing docs for `ParseDiagnosticCategory::as_str` and `from_parser_category` (lines 62 and 74). Related reporter DTOs have the same issue for `OutputFormat` variants and `ParseError` fields (crates/daml-lint/src/reporter.rs:10-63).
- Recommendation: Document parser and reporter DTO fields with exact coordinate basis, absence semantics for `end_column`, category stability guarantees, and relationship between parser diagnostics and formatted parse errors. Add docs for `OutputFormat` variants and `ParseResult` fields so rustdoc users do not need README/tests to understand the API contract.

### DL-LINT-EH-001 — MEDIUM — daml-lint / error-handling

- Location: `crates/daml-lint/src/detector.rs:13 (DetectError)`
- Actionable: yes
- Confidence: high
- Principle: Rust Book: return Result for functions that might fail and reserve panic for unrecoverable states; Rust API Guidelines C-GOOD-ERR: public Result error types should be meaningful, implement Error/Display, be Send/Sync where possible, and support source chaining where useful (https://doc.rust-lang.org/book/ch09-00-error-handling.html, https://rust-lang.github.io/api-guidelines/interoperability.html#error-types-are-meaningful-and-well-behaved-c-good-err).
- Evidence: DetectError stores only detector and message strings (lines 13-15) and implements Error with no source() (line 47). ScriptDetector::try_detect converts ScriptLoadError into DetectError::new(self.name(), format!("{}: {e}", self.path)) at lines 519-522, and ConfiguredDetector::try_detect wraps an inner DetectError by copying only e.message().to_string() at lines 314-319. README lines 248-251 tell library callers to use Detector::try_detect for rule errors, and the unit test at crates/daml-lint/src/detectors/script.rs:631-636 asserts only detector/message text for runtime failures.
- Recommendation: Make DetectError preserve structured causes instead of flattening them into message strings. For example, add a non-exhaustive cause enum or source field such as Box<dyn Error + Send + Sync + 'static> where feasible, implement Error::source(), and keep detector/path as typed context accessors. Update ConfiguredDetector to preserve an inner DetectError as the source rather than copying only its message.

### DL-LINT-EH-002 — MEDIUM — daml-lint / error-handling

- Location: `crates/daml-lint/src/detectors/script.rs:10 (ScriptLoadError)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines C-GOOD-ERR: meaningful error types should implement Error/Display and preserve source chains where useful; Rust Book: recoverable failures should give callers options through Result rather than collapsed text (https://rust-lang.github.io/api-guidelines/interoperability.html#error-types-are-meaningful-and-well-behaved-c-good-err, https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html).
- Evidence: ScriptLoadError has typed sources for RuntimeInit and IoRead (lines 12-18), but Error is implemented as an empty impl with no source() (line 108), so even those typed causes are not exposed. Several variants named source store String instead of a typed error or clearly named message/detail field: UnknownSeverity, RegisterConfig, RegisterReport, Invoke, ParseNode, and EvalError (lines 25-56). The constructors convert QuickJS and serde_json failures with e.to_string() at lines 188-194, 202-207, 267-272, 343-366, and 369-400.
- Recommendation: Implement Error::source() for variants that already carry std::io::Error or rquickjs::Error, and store typed sources for serde_json::Error and SeverityParseError where available. For JavaScript exception text that cannot be represented as an Error source, rename the field to message/detail to avoid pretending it is a source chain. Then have DetectError preserve ScriptLoadError as its source when custom rule detection fails.

### DL-LINT-EH-004 — MEDIUM — daml-lint / error-handling

- Location: `crates/daml-lint/src/detector.rs:237 (Detector::detect)`
- Actionable: yes
- Confidence: high
- Principle: Rust Book: returning Result is the default for functions that might fail, while panics should be reserved for unrecoverable bugs, impossible states, tests/examples, or documented caller contract violations; Rust API Guidelines C-FAILURE: public panic conditions should be documented in a Panics section (https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html, https://rust-lang.github.io/api-guidelines/documentation.html#function-docs-include-error-panic-and-safety-considerations-c-failure).
- Evidence: The public trait method Detector::detect calls self.try_detect(module).unwrap_or_else(|e| panic!("detector '{}' failed: {}", self.name(), e)) at lines 237-240. The same trait documents try_detect as the fallible API with an Errors section at lines 241-246, and README lines 248-251 state that library callers can use Detector::try_detect to receive custom-rule runtime errors without terminating the host process. The main CLI uses try_detect rather than detect at src/main.rs lines 137-143. There is no formal # Panics section on detect, even though custom JavaScript detector failures are expected/recoverable rule failures, not impossible states.
- Recommendation: Either make the panic contract explicit with a # Panics section saying detect is only for detectors whose try_detect cannot fail, or de-emphasize/deprecate detect for public library use and update docs/examples toward try_detect. If detect remains, document that custom ScriptDetector runtime errors will panic through this adapter.

### daml-lint-interoperability-001 — MEDIUM — daml-lint / interoperability

- Location: `crates/daml-lint/src/parser.rs:42 (ParseDiagnosticCategory)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines interoperability C-CONV-TRAITS and C-COMMON-TRAITS: conversions should use standard traits where they make sense, and public types should eagerly implement applicable common traits. Source: https://rust-lang.github.io/api-guidelines/interoperability.html
- Evidence: ParseDiagnosticCategory is a public, stable, machine-readable enum with an inherent as_str() at lines 60-71 and from_parser_category() at lines 73-83. Callers and reporters use category.as_str() in reporter.rs and main.rs, and docs/reference/crates.md documents the string tags. The enum has Debug/Clone/Copy/PartialEq/Eq but no Display, From<ParserDiagnosticCategory>, or FromStr/TryFrom<&str> for the documented tags.
- Recommendation: Implement Display for ParseDiagnosticCategory using the same kebab-case strings as as_str(), replace from_parser_category() with or supplement it by impl From<daml_parser::ast::DiagnosticCategory>, and consider FromStr/TryFrom<&str> with a meaningful error if the documented tags are intended to be accepted from users or external data.

### daml-lint-interoperability-004 — MEDIUM — daml-lint / interoperability

- Location: `crates/daml-lint/src/ir.rs:310 (Template, InterfaceInstance, EnsureClause, Choice, Function, Import, InterfaceMethod, Interface, DamlModule)`
- Actionable: yes
- Confidence: medium
- Principle: Rust API Guidelines interoperability C-COMMON-TRAITS: crates defining public types should eagerly implement applicable common traits such as Clone, Eq, PartialEq, Hash, Debug, Display, and Default where semantically valid. Source: https://rust-lang.github.io/api-guidelines/interoperability.html
- Evidence: Several public rule-facing IR DTOs derive Debug, Clone, and Serialize but not PartialEq/Eq: Template at ir.rs:310-326, InterfaceInstance at 328-335, EnsureClause at 337-342, Choice at 344-356, Function at 454-462, Import at 464-471, InterfaceMethod at 488-494, Interface at 496-506, and DamlModule at 508-519. Nearby public DTOs such as Field, RecordField, LetBinding, CaseAlt, Finding, and FindingLocation already derive PartialEq, showing inconsistent common-trait coverage. Parser tests manually compare individual fields in parser_ir_contracts.rs instead of being able to compare larger IR values directly.
- Recommendation: Derive PartialEq consistently for the public IR DTO graph, and derive Eq where all contained fields support Eq. Consider Hash only for stable identifier-like or immutable value types where hashing the full value is semantically useful; do not add traits where equality/hash semantics would be misleading.

### daml-lint-interoperability-005 — MEDIUM — daml-lint / interoperability

- Location: `crates/daml-lint/src/detectors/script.rs:221 (load_script)`
- Actionable: yes
- Confidence: medium
- Principle: Rust API Guidelines interoperability C-RW-VALUE: generic reader/writer APIs should take R: Read / W: Write by value where applicable, because &mut readers/writers also implement those traits. Source: https://rust-lang.github.io/api-guidelines/interoperability.html
- Evidence: The public custom-rule loading API is path-only: load_script(path: &Path) at detectors/script.rs:221-224 and load_script_with_options(path: &Path, ...) at 237-245 read with std::fs::read_to_string. The source-based loaders used for embedded built-ins are crate-private at detectors/script.rs:248-259 and detectors/mod.rs:57-60. Integration tests exercising public APIs must write temporary script files before loading rules in custom_rule_runtime_contracts.rs:19-33, even when the script source is already in memory.
- Recommendation: Expose an in-memory or reader-based public loader, for example load_script_source(label: impl Into<String>, source: &str) and/or load_script_reader<R: std::io::Read>(label: impl Into<String>, reader: R, options: &serde_json::Value). Also consider changing path parameters to impl AsRef<Path> for call-site flexibility while keeping the existing path-based functions as convenience wrappers if compatibility is required.

### daml-lint-type-safety-001 — MEDIUM — daml-lint / type-safety

- Location: `crates/daml-lint/src/ir.rs:24 (SourceSpan)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines type safety: newtypes provide static distinctions and arguments should convey meaning through types, not bare primitives. The audit focus also calls out avoiding accidental mixing of byte, char, UTF-16, line, and column spaces.
- Evidence: SourceSpan publicly exposes line, column, start, end, byte_start, and byte_end all as usize. from_text_range fills these from three coordinate spaces: char line/column, UTF-16 offsets, and UTF-8 byte offsets. README lines 221-224 and examples/daml-lint.d.ts lines 17-26 document that start/end are JavaScript UTF-16 offsets while byte_start/byte_end are parser byte offsets; custom_rule_runtime_contracts.rs lines 154-156 exercises source.slice(span.start, span.end). The workspace already has daml_syntax coordinate newtypes such as LineNumber, CharColumn, ByteOffset, Utf16Offset, ByteLineCol, and CharLineCol documented in crates/daml-syntax/src/lib.rs lines 20-22 and coordinate.rs lines 101-128.
- Recommendation: Introduce typed Rust-side coordinate fields or accessors for SourceSpan, reusing daml_syntax LineNumber, CharColumn, ByteOffset, Utf16Offset, or a typed SourceRange wrapper. If the JavaScript/JSON contract must remain numeric, keep serde serialization as numbers while making the Rust API prevent cross-space assignment.

### daml-lint-type-safety-002 — MEDIUM — daml-lint / type-safety

- Location: `crates/daml-lint/src/detector.rs:181 (FindingLocation::new)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines type safety: core primitives like usize have many possible interpretations, so deliberate domain types should encode meaning and invariants where mixups are plausible.
- Evidence: FindingLocation::new takes file, line: usize, column: usize, and Finding stores public line and column usize fields. Tests in detector_contracts.rs and compile_pass/finding_field_reads.rs construct locations with raw integer pairs such as FindingLocation::new("src/Main.daml", 7, 4). Parser diagnostics and reporter ParseError similarly expose line, column, and end_column as usize/Option<usize>, while daml_syntax diagnostics already retain LineNumber and CharColumn internally before daml-lint converts them with Coordinate::get in parser.rs lines 153-156.
- Recommendation: Add a public SourceLocation or ReportLocation type with LineNumber and CharColumn fields, and constructors that validate non-zero coordinates. Keep existing numeric JSON/SARIF output as a serialization detail. If immediate breaking changes are too large, add typed constructors/accessors first and steer new public code away from raw usize pairs.

### daml-parser-doc-001 — MEDIUM — daml-parser / documentation

- Location: `crates/daml-parser/README.md:50 (README dependency example)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines documentation C-CRATE-DOC and C-EXAMPLE (https://rust-lang.github.io/api-guidelines/documentation.html): crate docs and examples should be accurate and copyable.
- Evidence: README usage shows `daml-parser = "0.7"`, while `crates/daml-parser/Cargo.toml` declares version `0.8.0` and the workspace dependency also uses `0.8.0`. This is a direct docs/version mismatch.
- Recommendation: Update the README dependency snippet to the current published package line, or use a release-maintained placeholder/process so the install example changes with the crate version.

### daml-parser-doc-002 — MEDIUM — daml-parser / documentation

- Location: `crates/daml-parser/src/ast.rs:4 (ast module docs / Type)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines documentation C-CRATE-DOC (https://rust-lang.github.io/api-guidelines/documentation.html): public contracts must match current behavior.
- Evidence: The module docs state that every node carries a source position and byte span, and README lines 152-153 make a similar claim. But `Type` variants at `crates/daml-parser/src/ast.rs:321-352` carry `Span` only and no `Pos`; tests such as `type_node_spans_are_tight` verify type spans, not positions.
- Recommendation: Narrow the wording to distinguish AST families: declarations/expressions/patterns generally carry `Pos` plus `Span`, while `Type` nodes carry `Span` only. Add this contract near `Type` and in the README source-position section.

### daml-parser-doc-003 — MEDIUM — daml-parser / documentation

- Location: `crates/daml-parser/src/ast.rs:132 (Pat, Expr, TemplateBodyDecl, Decl, and public AST DTO fields)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines documentation C-CRATE-DOC/C-EXAMPLE (https://rust-lang.github.io/api-guidelines/documentation.html): public structs, enums, variants, and fields should explain their contracts; type-safety docs require custom types to convey meaning (https://rust-lang.github.io/api-guidelines/type-safety.html).
- Evidence: Many public AST shapes are exposed for direct downstream matching but have sparse or missing field/variant contracts: `Pat` starts at line 132 with mostly undocumented fields, `Expr` at line 182 has several undocumented variants/fields, `TemplateDecl` fields at lines 519-523 are undocumented, `ImportDecl` fields at lines 568-572 are undocumented, and `Module` fields `name`, `pos`, `imports`, and `decls` are not documented. Immediate consumers in `daml-lint/src/parser.rs` and `daml-fmt/src/layout_ast.rs` pattern-match these shapes directly.
- Recommendation: Add targeted rustdoc to public AST DTOs and fields explaining ordering, optionality (`None` meanings), raw-preservation variants, parser-created construction expectations, and `pos`/`span` semantics. Prioritize shapes consumed by daml-lint and daml-fmt.

### daml-parser-doc-005 — MEDIUM — daml-parser / documentation

- Location: `crates/daml-parser/src/lexer.rs:257 (TokenKind, LexErrorKind, LexOutput, LexWithTriviaOutput)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines documentation C-CRATE-DOC/C-FAILURE (https://rust-lang.github.io/api-guidelines/documentation.html): public token/data/error contracts should be documented; interoperability C-GOOD-ERR asks for meaningful error types (https://rust-lang.github.io/api-guidelines/interoperability.html).
- Evidence: `TokenKind` literal variants at lines 270-273 do not document whether strings/chars are raw or decoded; `LexErrorKind` variants at lines 565-574 lack per-variant contracts; `LexOutput` and `LexWithTriviaOutput` public fields at lines 598-614 lack docs. Tests verify behavior such as decoded string/char literals, tab-aware positions, invalid escapes, and trivia preservation, but the public docs do not expose those contracts.
- Recommendation: Document lexer DTO fields and variants with the observable contracts from tests: decoded literal values versus source spans, trivia inclusion/exclusion, ordering, tab/position semantics, and what each recoverable lexical error means.

### daml-parser-error-handling-001 — MEDIUM — daml-parser / error-handling

- Location: `crates/daml-parser/src/ast.rs:654 (ParseDiagnostic)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines C-GOOD-ERR recommends meaningful, well-behaved error types implementing Error/Display and avoiding string-only error channels; Rust Book error handling recommends Result for expected failures and preserving caller choice. Sources: https://rust-lang.github.io/api-guidelines/interoperability.html and https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html.
- Evidence: ParseDiagnostic exposes message: String plus a broad DiagnosticCategory at crates/daml-parser/src/ast.rs:654-661. parse_module converts LexError into ParseDiagnostic by storing e.to_string() and category Lex at crates/daml-parser/src/parse.rs:139-148, dropping the typed LexErrorKind/source chain even though LexError itself has kind and implements Error at crates/daml-parser/src/lexer.rs:391-439. Parser diagnostics are also emitted through diag(message: impl Into<String>) and diag_cat(..., message: impl Into<String>) at crates/daml-parser/src/parse.rs:308-324. Tests and downstream wrappers assert/copy message strings directly, e.g. crates/daml-parser/tests/diagnostics_recovery.rs:97-109 and crates/daml-syntax/src/lib.rs:321-336.
- Recommendation: Introduce a typed diagnostic detail/kind alongside the human message, such as ParseDiagnosticKind variants for lexical errors, expected-token failures, malformed type annotations, unsupported syntax, and recursion limits. Preserve LexErrorKind or LexError-derived structured data for lexical diagnostics, and consider Display/Error impls on ParseDiagnostic if it is intended to be nested under ParseModuleError. Keep message() as presentation, not the machine-readable contract.

### daml-parser-type-safety-001 — MEDIUM — daml-parser / type-safety

- Location: `crates/daml-parser/src/ast.rs:102 (FieldAssign)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines type safety C-CUSTOM-TYPE/C-NEWTYPE: use deliberate domain types to convey interpretation and invariants instead of ambiguous Option/primitives; source: https://rust-lang.github.io/api-guidelines/type-safety.html.
- Evidence: FieldAssign stores name: Identifier and value: Option<Expr>, with docs saying None means both record puns and '..' wildcards. The parser confirms the ambiguity: record_fields creates wildcard fields with name '..'.into() and value None at crates/daml-parser/src/parse.rs:2271-2278, and puns with a normal field name and value None at crates/daml-parser/src/parse.rs:2303-2309. This also allows the wildcard sentinel to inhabit Identifier even though it is not identifier-like text.
- Recommendation: Replace the sentinel/Option encoding with a domain enum such as RecordField::Assign { name, value }, RecordField::Pun { name }, and RecordField::Wildcard, or add a FieldAssignKind enum. This would make wildcard versus pun explicit and remove the need for Identifier('..').

### daml-parser-type-safety-002 — MEDIUM — daml-parser / type-safety

- Location: `crates/daml-parser/src/ast.rs:280 (Expr::Section)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines type safety C-CUSTOM-TYPE: related choices should be encoded as types/enums that rule out invalid state combinations; source: https://rust-lang.github.io/api-guidelines/type-safety.html.
- Evidence: Expr::Section combines operand: Option<Box<Expr>> with side: SectionSide. Parser behavior shows '(+)' becomes operand None with SectionSide::Right at crates/daml-parser/src/parse.rs:2880-2887, while '(+ 1)' and '(1 +)' use Some with Right/Left at crates/daml-parser/src/parse.rs:2889-2897 and 2913-2924. The public AST can still construct nonsensical states such as operand None with SectionSide::Left, and Expr::render ignores side when operand is None at crates/daml-parser/src/ast.rs:813-819. Tests assert the current None/Right shape for '(+)' in crates/daml-parser/tests/module_parse_behavior.rs:56-77.
- Recommendation: Model the section shape directly, for example OperatorReference { op }, LeftSection { op, operand }, and RightSection { op, operand }, or introduce a SectionOperand/SectionKind enum that cannot represent None+Left.

### daml-parser-type-safety-003 — MEDIUM — daml-parser / type-safety

- Location: `crates/daml-parser/src/ast.rs:21 (Span)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines C-NEWTYPE recommends newtypes for static distinctions; this audit focus specifically calls out avoiding mixed byte, char, UTF-16, line, and column spaces; source: https://rust-lang.github.io/api-guidelines/type-safety.html.
- Evidence: Span exposes pub start: usize and pub end: usize at crates/daml-parser/src/ast.rs:21-24; Pos exposes pub line: usize and pub column: usize at crates/daml-parser/src/lexer.rs:248-253; Token and Trivia expose byte offsets through start()/end() accessors at crates/daml-parser/src/lexer.rs:313-320 and 377-384. README documents that AST nodes carry 1-based Pos and byte Span at crates/daml-parser/README.md:152-188. Immediate caller daml-syntax has to add separate ByteOffset, LineNumber, ByteColumn, CharColumn, and Utf16Offset newtypes and validate ParserSpan conversion at crates/daml-syntax/src/coordinate.rs:1-128 and crates/daml-syntax/src/lib.rs:541-583.
- Recommendation: Introduce parser-owned coordinate newtypes or private-field wrappers, such as ByteOffset, ByteSpan, LineNumber, and CharColumn, and expose Token::span()/Trivia::span() returning a byte-span type. Keep raw usize extraction explicit for slicing. This can be planned as a breaking 0.x API cleanup because the README already says public AST shape changes are SemVer-relevant.

### daml-syntax-doc-001 — MEDIUM — daml-syntax / documentation

- Location: `crates/daml-syntax/src/lib.rs:88 (Diagnostic::end_column)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines documentation C-FAILURE/C-EXAMPLE expects public API contracts to state behavior precisely; rustdoc examples/tests should keep those contracts current (https://rust-lang.github.io/api-guidelines/documentation.html, https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html).
- Evidence: The accessor is documented only as "1-based character column of the diagnostic end when the span is single-line" at crates/daml-syntax/src/lib.rs:88. The implementation sets `end_column` only when the diagnostic slice is non-empty and contains no newline at crates/daml-syntax/src/lib.rs:325-328, and computes `diagnostic.pos.column + s.chars().count()`, which makes it an exclusive end column. Parser diagnostics can be zero-width at EOF via `cur_span` in crates/daml-parser/src/parse.rs:439-449, so a same-line zero-width span gets `None`, not `Some(column)`. The immediate caller forwards this to lint diagnostics at crates/daml-lint/src/parser.rs:153-157 and reporters emit it as JSON/SARIF endColumn at crates/daml-lint/src/reporter.rs:155-158 and 343-350.
- Recommendation: Clarify the public contract: document that `end_column` is an exclusive 1-based Unicode-scalar end column, present only for non-empty single-line diagnostic spans, and absent for multi-line or zero-width spans. Add or update a public API test that pins at least one `Some(end_column)` case and one zero-width or multi-line `None` case if this behavior is intentional.

### daml-syntax-error-handling-001 — MEDIUM — daml-syntax / error-handling

- Location: `crates/daml-syntax/src/lib.rs:220 (LineIndex::utf16_col)`
- Actionable: yes
- Confidence: high
- Principle: Rust Book ch09: use Result for recoverable errors and reserve panic for unrecoverable states or caller contract violations (https://doc.rust-lang.org/book/ch09-00-error-handling.html, https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html). Rust API Guidelines C-FAILURE: document error and panic contracts (https://rust-lang.github.io/api-guidelines/documentation.html).
- Evidence: The docs say both coordinates must be valid 1-based values, but the implementation handles an out-of-range line with `.unwrap_or(self.source_len)` and an out-of-range byte column with `.min(self.source_len)`, silently mapping invalid caller input to EOF. Tests cover valid `utf16_col` calls and `utf16_range` clamping, but do not document or verify `utf16_col` clamping for invalid line/column coordinates.
- Recommendation: Make the out-of-range contract explicit. Prefer a fallible API such as `try_utf16_col(...) -> Result<Utf16Offset, CoordinateRangeError>` with typed variants for invalid line/column, or change the existing API in a clean break if that is the intended public contract. If silent clamping is intentional, document it precisely and add tests for line-past-EOF and column-past-line/source behavior.

### daml-syntax-interoperability-001 — MEDIUM — daml-syntax / interoperability

- Location: `crates/daml-syntax/src/coordinate.rs:54 (LineNumber::try_new / ByteColumn::try_new / CharColumn::try_new / Coordinate::get)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines interoperability C-CONV-TRAITS recommends standard From/TryFrom/AsRef/AsMut conversions where they make sense; type-safety C-NEWTYPE supports newtypes for domain distinctions.
- Evidence: The coordinate newtypes are well-typed, but one-based coordinates expose fallible construction only as try_new(value) -> Option<Self> at lines 52-59 and raw extraction through the crate-specific Coordinate::get trait at lines 95-99. Tests exercise these ad-hoc APIs in crates/daml-syntax/tests/coordinate_contracts.rs:10-45. Only ByteOffset has standard conversion coverage with From<TextSize> and TryFrom<ByteOffset> for TextSize at lines 131-143.
- Recommendation: Add standard conversion impls where semantically valid: TryFrom<usize> for one-based coordinates with a small meaningful error type for zero, From<LineNumber/ByteColumn/CharColumn/ByteOffset/Utf16Offset> for usize if explicit raw extraction remains intended, and possibly From<usize> for zero-based coordinates. Keep new()/try_new() only if they remain useful constructors, but make standard conversions the interoperable path used in examples/tests.

### daml-syntax-interoperability-002 — MEDIUM — daml-syntax / interoperability

- Location: `crates/daml-syntax/src/lib.rs:450 (ParserSpanToTextRangeErrorKind)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines C-COMMON-TRAITS says public types should eagerly implement applicable common traits, including Display; C-GOOD-ERR says public Result error types should be meaningful and provide Display/Error impls.
- Evidence: ParserSpanToTextRangeErrorKind is a public non_exhaustive enum with Debug/Clone/Copy/PartialEq/Eq only at lines 447-459. ParserSpanToTextRangeError implements Display at lines 508-526, but OutOfBounds, InvertedSpan, and NonUtf8Boundary all render the same generic message. Integration tests assert the generic strings for distinct failure kinds in crates/daml-syntax/tests/source_api.rs:73-123, so the user-facing Display output currently loses the specific reason available through kind().
- Recommendation: Implement Display for ParserSpanToTextRangeErrorKind and include the specific kind text in ParserSpanToTextRangeError's Display output, while keeping the typed kind() accessor. Update tests to assert distinct messages for out-of-bounds, inverted, non-UTF-8-boundary, and TextSize overflow failures.

### daml-syntax-type-safety-001 — MEDIUM — daml-syntax / type-safety

- Location: `crates/daml-syntax/src/lib.rs:90 (Diagnostic::end_column)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines type safety C-CUSTOM-TYPE: arguments and API values should convey meaning through deliberate types rather than ambiguous Option/core types; source: https://rust-lang.github.io/api-guidelines/type-safety.html.
- Evidence: The public accessor returns Option<CharColumn>, documented as present when the span is single-line. The implementation at crates/daml-syntax/src/lib.rs:325-328 returns None both for empty spans and spans containing a newline. Tests inspect diagnostic line/column but do not pin end_column semantics in crates/daml-syntax/tests/source_api.rs:126-138. This makes None encode multiple states that callers cannot distinguish by type.
- Recommendation: Replace or supplement Option<CharColumn> with a small public enum or struct that names the cases, for example SameLineEnd(CharColumn), Multiline, and EmptySpan, or document and test a deliberately narrower invariant if the distinction is intentionally not exposed.

### FMT-DOC-003 — LOW — daml-fmt / documentation

- Location: `docs/reference/crates.md:53 (daml-fmt version reference)`
- Actionable: yes
- Confidence: high
- Principle: Documentation should match current package metadata so downstream users do not rely on stale crate contracts.
- Evidence: docs/reference/crates.md lists daml-fmt as version 0.5.0, but crates/daml-fmt/Cargo.toml line 3 and cargo metadata report version 0.6.0.
- Recommendation: Update the workspace member version table for daml-fmt to 0.6.0, preferably with the other workspace versions if this table is maintained manually.

### FMT-DOC-005 — LOW — daml-fmt / documentation

- Location: `crates/daml-fmt/src/lib.rs:243 (FormatOptions::new)`
- Actionable: yes
- Confidence: medium
- Principle: Rust API Guidelines documentation guidance says public items should have clear contracts and examples where useful; rustdoc renders public methods as part of the downstream API surface.
- Evidence: FormatOptions has useful type-level docs and an example at lines 211-227, and import_order has contract docs at lines 249-256, but the public constructor FormatOptions::new at line 243 and builder with_import_order at line 263 have no method-level rustdoc. Tests/library_behavior.rs lines 15-23 verify that new matches Default and that with_import_order sets ImportOrder::Preserve.
- Recommendation: Add short rustdoc comments to FormatOptions::new and FormatOptions::with_import_order stating that new returns the default organizing configuration and with_import_order sets the import ordering strategy, with a link back to import_order for the package-identity warning.

### DFMT-EH-002 — LOW — daml-fmt / error-handling

- Location: `crates/daml-fmt/src/bin/daml-fmt.rs:102 (main)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines interoperability C-GOOD-ERR: errors should be meaningful and well-behaved; Rust Book: recoverable IO failures should be reported with useful information rather than collapsed into opaque failures.
- Evidence: The stdin path uses `std::io::stdin().read_to_string(&mut text).is_err()` and then prints only `daml-fmt: failed to read stdin` at line 103. The underlying `io::Error` is discarded, unlike file read/write paths that include `{e}` at lines 121 and 145. CLI tests cover parser diagnostics from stdin but not stdin IO error detail.
- Recommendation: Change the stdin read branch to preserve and print the source error, e.g. `if let Err(e) = ... { eprintln!("daml-fmt: failed to read stdin: {e}"); exit(2); }`. If CLI error handling grows, consider a small typed CLI error enum for consistent display and exit-code mapping.

### DFMT-EH-003 — LOW — daml-fmt / error-handling

- Location: `crates/daml-fmt/src/bin/coverage.rs:19 (collect)`
- Actionable: yes
- Confidence: high
- Principle: Rust Book: panic is appropriate for impossible states, tests/examples, or caller contract violations; recoverable filesystem failures in command-line inputs should be returned/reported. Rust API Guidelines C-GOOD-ERR prefers meaningful errors over panics for expected failures.
- Evidence: The dev-tool `collect` functions panic on directory traversal failures: `coverage.rs` uses `read_dir(path).unwrap_or_else(|e| panic!(...))` and `e.expect("valid read_dir entry")` at lines 19-21. The same pattern appears in `crates/daml-fmt/src/bin/ast-check.rs:18` and `crates/daml-fmt/src/bin/lossless-check.rs:16`. An unreadable directory or failed directory entry is a normal filesystem error for a CLI argument, not an impossible formatter invariant. The coverage tests assert a missing file produces `READ-ERR`, but do not cover unreadable directory traversal.
- Recommendation: Have each dev-tool `collect` return or accumulate `io::Result`/read errors instead of panicking. Report directory traversal failures with the path and source error, increment the existing error count, and exit nonzero consistently with file read failures.

### DFMT-EH-004 — LOW — daml-fmt / error-handling

- Location: `docs/reference/crates.md:190 (daml-fmt public library API table)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines documentation C-FAILURE: public API docs should accurately describe errors and related contracts; interoperability C-GOOD-ERR depends on callers knowing the concrete error type.
- Evidence: The reference table says `lex_diagnostics(src: &str) -> Vec<String>`, but the actual public API returns `Vec<FormatDiagnostic>` at crates/daml-fmt/src/lib.rs:152. The same table omits `FormatDiagnostic`, `FormatError`, `source_diagnostics`, `try_format_source`, and `try_format_source_with_options`, even though README lines 96-100 and lib.rs lines 289-314 document those typed error APIs. This can mislead callers away from the intended typed error path.
- Recommendation: Update `docs/reference/crates.md` to list the typed diagnostics and fallible formatting functions, correct `lex_diagnostics` to `Vec<FormatDiagnostic>`, and mention that `FormatError` implements `Display` and `std::error::Error`. Align the reference with README and rustdoc.

### daml-fmt-interoperability-001 — LOW — daml-fmt / interoperability

- Location: `crates/daml-fmt/src/lib.rs:201 (ImportOrder)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines interoperability: public types should eagerly implement applicable common traits such as Display and Default where semantically valid (C-COMMON-TRAITS); documentation and user-facing APIs should expose stable contracts. Sources: https://rust-lang.github.io/api-guidelines/interoperability.html and https://rust-lang.github.io/api-guidelines/documentation.html
- Evidence: ImportOrder is a public, non-exhaustive formatter option enum derived only as Debug, Clone, Copy, PartialEq, Eq at crates/daml-fmt/src/lib.rs:201-208. FormatOptions::default and FormatOptions::new both hard-code ImportOrder::Organize at crates/daml-fmt/src/lib.rs:233-247, README documents ImportOrder as part of the library API at crates/daml-fmt/README.md:118-120, and docs/reference/crates.md:189 describes Organize as the default and Preserve as the CLI flag behavior. The package search found no Default or Display impl for ImportOrder.
- Recommendation: Add an explicit Default impl for ImportOrder returning ImportOrder::Organize, and consider a Display impl with stable user-facing labels such as organize and preserve. Add focused tests in crates/daml-fmt/tests/library_behavior.rs for ImportOrder::default and Display output if Display is made public contract.

### daml-fmt-interoperability-002 — LOW — daml-fmt / interoperability

- Location: `crates/daml-fmt/src/lib.rs:118 (FormatError)`
- Actionable: yes
- Confidence: medium
- Principle: Rust API Guidelines interoperability: conversions and borrowed views should use standard traits like AsRef where they make sense, and public error types should be meaningful and well-behaved (C-CONV-TRAITS, C-GOOD-ERR). Source: https://rust-lang.github.io/api-guidelines/interoperability.html
- Evidence: FormatError is a public error type wrapping Vec<FormatDiagnostic> at crates/daml-fmt/src/lib.rs:118-129. It implements Display and std::error::Error at crates/daml-fmt/src/lib.rs:132-144, and tests consume the diagnostics through the custom diagnostics() accessor at crates/daml-fmt/tests/library_behavior.rs:93-96. The package search found no AsRef<[FormatDiagnostic]> or iterator impl for FormatError, so generic APIs expecting a borrowed diagnostics slice cannot use the standard AsRef path.
- Recommendation: Keep diagnostics() for readability, but add impl AsRef<[FormatDiagnostic]> for FormatError returning self.diagnostics(). If collection-style iteration is desired, add IntoIterator for &FormatError rather than exposing construction semantics. Avoid FromIterator/Extend unless the crate explicitly defines whether an empty FormatError is valid.

### daml-fmt-type-safety-002 — LOW — daml-fmt / type-safety

- Location: `crates/daml-fmt/src/layout_ast.rs:1046 (import_group)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines type safety: core primitives like u8 have many possible interpretations; custom enums/structs convey meaning and make later expansion safer (https://rust-lang.github.io/api-guidelines/type-safety.html).
- Evidence: Import organization classifies modules by returning raw u8 values from import_group at crates/daml-fmt/src/layout_ast.rs:1046-1054, storing that value in BorrowedImport.group at 969-974, and sorting/comparing the numeric group at 1012-1036. Tests in crates/daml-fmt/tests/library_behavior.rs:103-116 verify grouping/sorting behavior, and README/docs warn that import organization has package-identity consequences, so the grouping categories are domain concepts rather than arbitrary numbers.
- Recommendation: Replace the u8 with a private enum such as ImportGroup { DamlStdlib, DaLibrary, LocalOrExternal } deriving Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Debug. Keep the current sort order in the enum variant order or an explicit rank method, and update tests to continue asserting the group boundary behavior.

### daml-fmt-type-safety-003 — LOW — daml-fmt / type-safety

- Location: `docs/reference/crates.md:190 (lex_diagnostics)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines documentation: public contracts should be documented accurately, and public items should have examples/contracts that explain the typed API surface (https://rust-lang.github.io/api-guidelines/documentation.html).
- Evidence: The reference table documents lex_diagnostics(src: &str) -> Vec<String> at docs/reference/crates.md:190, but the actual public API returns Vec<FormatDiagnostic> at crates/daml-fmt/src/lib.rs:152 and README.md:96-100 describes typed FormatDiagnostic values. The same reference table omits source_diagnostics and the try_format_* Result-returning APIs that are documented in README.md:96-100 and tested in crates/daml-fmt/tests/library_behavior.rs:61-100. This does not break the code, but it hides the typed diagnostics/error contract from users relying on the reference docs.
- Recommendation: Update docs/reference/crates.md to list lex_diagnostics(src: &str) -> Vec<FormatDiagnostic>, source_diagnostics(src: &str) -> Vec<FormatDiagnostic>, try_format_source(src: &str) -> Result<String, FormatError>, and try_format_source_with_options(src: &str, options: FormatOptions) -> Result<String, FormatError>. Keep the docs aligned with the typed accessors on FormatDiagnostic and diagnostics() on FormatError.

### daml-lint-doc-005 — LOW — daml-lint / documentation

- Location: `crates/daml-lint/README.md:318 (Public API Stability)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines documentation: crate documentation and README should match current behavior and released artifact metadata.
- Evidence: The package manifest reports `daml-lint` version 0.8.1 (crates/daml-lint/Cargo.toml:1-3). README public API stability text still says the CLI exit codes and documented feature flags are the stable user contract for `0.7.x` (crates/daml-lint/README.md:316-324), and docs/reference/crates.md lists `daml-lint` as version `0.8.0` (docs/reference/crates.md:48-53).
- Recommendation: Update version-specific docs to the current 0.8 line or remove patch/minor-specific wording where it will drift. Keep README, docs/reference/crates.md, and Cargo.toml synchronized as part of release checks.

### DL-LINT-EH-003 — LOW — daml-lint / error-handling

- Location: `crates/daml-lint/src/config.rs:154 (ConfigError)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines C-GOOD-ERR: error types returned in Result should provide appropriate Error and Display impls and preserve source chains for downstream error handling libraries (https://rust-lang.github.io/api-guidelines/interoperability.html#error-types-are-meaningful-and-well-behaved-c-good-err).
- Evidence: ConfigError carries underlying source errors for MissingCurrentDir, ReadConfig, ParseConfig, PluginManifestRead, PluginManifestParse, and RuleLoadFailed (lines 11-20, 30-36, and 52-56), but its Error impl is empty at line 154. The CLI currently prints Display text from ConfigError in src/main.rs lines 50-58 and exits, so users see the message, but programmatic error chains are not available if this module is reused or moved into the library surface.
- Recommendation: Implement std::error::Error::source() for ConfigError by matching variants with source fields and returning Some(source). For RuleLoadFailed, return the boxed ScriptLoadError. This is a surgical internal change because ConfigError is currently binary-private.

### DL-LINT-EH-005 — LOW — daml-lint / error-handling

- Location: `crates/daml-lint/src/detector.rs:214 (parse_severity)`
- Actionable: yes
- Confidence: medium
- Principle: Rust Book: use Result for expected/recoverable failures so callers can inspect and handle errors; Rust API Guidelines C-GOOD-ERR and C-CONV-TRAITS favor meaningful typed errors and standard conversion traits (https://doc.rust-lang.org/book/ch09-00-error-handling.html, https://rust-lang.github.io/api-guidelines/interoperability.html#error-types-are-meaningful-and-well-behaved-c-good-err).
- Evidence: Severity::from_str returns Result<Self, SeverityParseError> at lines 129-143, and SeverityParseError implements Display and Error at lines 62-72. The public helper parse_severity instead returns Option<Severity> and discards that typed error with s.parse().ok() at lines 212-215. Its main inspected caller, ScriptLoadError construction in detectors/script.rs lines 281-286, reconstructs an UnknownSeverity error with a formatted String rather than preserving SeverityParseError. README line 330 explicitly documents the improved Severity::from_str error type, but parse_severity remains a lossy public alternative.
- Recommendation: Prefer the FromStr API directly or change parse_severity to return Result<Severity, SeverityParseError> in the next breaking window. Internally, parse custom-rule severities with severity_str.parse::<Severity>() and carry SeverityParseError as a typed source in ScriptLoadError::UnknownSeverity.

### daml-lint-interoperability-002 — LOW — daml-lint / interoperability

- Location: `crates/daml-lint/src/detector.rs:214 (parse_severity)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines interoperability C-CONV-TRAITS and meaningful errors C-GOOD-ERR: prefer From/TryFrom/FromStr over ad-hoc conversion helpers, and preserve meaningful error types. Rust Book error handling also favors Result for recoverable errors. Sources: https://rust-lang.github.io/api-guidelines/interoperability.html and https://doc.rust-lang.org/book/ch09-00-error-handling.html
- Evidence: Severity already implements FromStr with SeverityParseError at detector.rs:129-143, but the public parse_severity(s: &str) -> Option<Severity> helper at detector.rs:214-215 discards that error. The script runtime imports and uses this helper at detectors/script.rs:1 and detectors/script.rs:281-287, manually reconstructing an UnknownSeverity message instead of using SeverityParseError.
- Recommendation: Prefer severity_str.parse::<Severity>() at call sites and either remove parse_severity before the next breaking release or deprecate it in favor of FromStr. If a helper remains, return Result<Severity, SeverityParseError> rather than Option.

### daml-lint-interoperability-003 — LOW — daml-lint / interoperability

- Location: `crates/daml-lint/src/reporter.rs:10 (OutputFormat)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines interoperability C-COMMON-TRAITS: public types should eagerly implement applicable common traits such as Display. Source: https://rust-lang.github.io/api-guidelines/interoperability.html
- Evidence: OutputFormat is a public CLI/report enum with FromStr at reporter.rs:38-49 and public usage in format_findings at reporter.rs:90-99 and main.rs:20-22. Tests cover parsing and OutputFormatParseError Display in reporter_contracts.rs:30-44, but OutputFormat itself has no Display implementation for canonical user-facing names.
- Recommendation: Implement Display for OutputFormat returning canonical lowercase values such as sarif, markdown, and json. This lets CLI/help/config/docs code use standard formatting instead of duplicating strings.

### daml-lint-interoperability-006 — LOW — daml-lint / interoperability

- Location: `crates/daml-lint/src/ir.rs:361 (Consuming, ImportStyle)`
- Actionable: yes
- Confidence: medium
- Principle: Rust API Guidelines interoperability C-COMMON-TRAITS and C-CONV-TRAITS: public enums with stable external names should implement common formatting/conversion traits where semantically valid. Source: https://rust-lang.github.io/api-guidelines/interoperability.html
- Evidence: Consuming at ir.rs:361-371 and ImportStyle at ir.rs:476-486 are public enums serialized as kebab-case and documented in README.md:210-212 as string enums surfaced to custom rules to avoid boolean ambiguity. Rust callers currently get Serialize plus boolean predicates is_consuming()/is_qualified(), but no Display for the same canonical tags and no FromStr/TryFrom<&str> if those tags need to round-trip in Rust code.
- Recommendation: Implement Display for Consuming and ImportStyle using their serialized kebab-case tags. Add FromStr/TryFrom<&str> with meaningful parse errors only if Rust callers are expected to accept these tags from external rule/config data.

### daml-lint-type-safety-003 — LOW — daml-lint / type-safety

- Location: `crates/daml-lint/src/detector.rs:258 (ConfiguredDetector::new)`
- Actionable: yes
- Confidence: medium
- Principle: Rust API Guidelines type safety: arguments should convey meaning through custom types rather than ambiguous Option or primitive parameters; builders are appropriate when construction has optional configuration.
- Evidence: ConfiguredDetector::new is public and takes name_override: Option<String> plus severity_override: Option<Severity>. That permits ambiguous states such as no overrides, both overrides, or an empty name without expressing intent in the type. Immediate callers avoid the ambiguity by using ConfiguredDetector::with_name in config.rs lines 242-245 and ConfiguredDetector::with_severity in config.rs lines 263-265; tests also use the named constructors.
- Recommendation: Make the general constructor private or replace it with a DetectorOverrides struct/builder whose methods are named by intent. Keep with_name and with_severity as the public simple paths, and validate or strongly type override names if external callers can supply them.

### daml-lint-type-safety-004 — LOW — daml-lint / type-safety

- Location: `crates/daml-lint/src/detector.rs:214 (parse_severity)`
- Actionable: yes
- Confidence: high
- Principle: Rust Book error handling: expected recoverable failures should return Result; Rust API Guidelines interoperability: use standard conversion traits and meaningful error types.
- Evidence: Severity implements FromStr with a meaningful SeverityParseError at detector.rs lines 129-143, and tests assert that the error reports the bad value and allowed levels in detector_contracts.rs lines 41-47. The public parse_severity helper still returns Option<Severity>, discarding that error detail. main.rs lines 218-219 already uses value.parse::<Severity>(), while script.rs line 282 calls parse_severity and reconstructs a separate UnknownSeverity message.
- Recommendation: Deprecate or change parse_severity to return Result<Severity, SeverityParseError>, or remove it from the public surface in favor of Severity::from_str/str::parse. Update script loading to use the same meaningful error path instead of losing detail through Option.

### daml-parser-doc-004 — LOW — daml-parser / documentation

- Location: `crates/daml-parser/src/parse.rs:20 (ParseModuleResult)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines documentation C-FAILURE (https://rust-lang.github.io/api-guidelines/documentation.html) and Rust Book error handling (https://doc.rust-lang.org/book/ch09-00-error-handling.html): recoverable failures should be explicit and documented.
- Evidence: `ParseModuleResult` is public and its fields `module` and `diagnostics` are public at lines 20-22, but the struct and fields lack direct rustdoc. Function docs describe tolerant parsing, and `strict_parse_contract.rs` verifies the tolerant/strict behavior, but users reading the DTO alone do not get the partial-module and source-ordered-diagnostics contract.
- Recommendation: Document `ParseModuleResult` and its fields directly: `module` is always present and may be partial; `diagnostics` are source-ordered recoverable parse/lex issues; use `has_errors`, `into_parts`, or `into_result` depending on tolerance.

### daml-parser-doc-006 — LOW — daml-parser / documentation

- Location: `crates/daml-parser/src/ast_span.rs:21 (AstSpanError)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines documentation C-FAILURE and interoperability C-GOOD-ERR (https://rust-lang.github.io/api-guidelines/documentation.html, https://rust-lang.github.io/api-guidelines/interoperability.html): public errors should be meaningful, well-behaved, and documented.
- Evidence: `AstSpanError` is public and implements `Display` and `std::error::Error`, but none of its public variants at lines 22-62 have rustdoc. `render_from_ast` has an `# Errors` section, and tests cover individual variants, but callers matching the non-exhaustive error enum lack per-variant contract docs. The same pattern appears for `RenderLosslessError` in `crates/daml-parser/src/lexer.rs:626-638`.
- Recommendation: Add one-line rustdoc to each public error variant explaining the invariant violation and whether byte fields are source offsets, interval endpoints, or diagnostic payloads. Keep the function-level `# Errors` sections as the overview.

### daml-parser-doc-007 — LOW — daml-parser / documentation

- Location: `crates/daml-parser/README.md:147 (README examples and parse_module_strict doctest)`
- Actionable: yes
- Confidence: medium
- Principle: Rust API Guidelines documentation C-QUESTION-MARK (https://rust-lang.github.io/api-guidelines/documentation.html): fallible examples should prefer `?` over `unwrap`/`expect` so copied examples do not normalize panics for recoverable errors.
- Evidence: README examples use `expect` at lines 147 and 158 and `unwrap` at lines 203 and 218. The `parse_module_strict` doctest uses `unwrap`/`unwrap_err` at `crates/daml-parser/src/parse.rs:171-174`. These are not production-code bugs and doctests pass, but the examples teach panic-style handling for fallible rendering/strict parsing APIs.
- Recommendation: Where examples call fallible APIs, wrap them in a hidden `fn main() -> Result<(), Box<dyn std::error::Error>>` and use `?`, or make the text explicit that the unwrap/expect is only asserting the example's fixed input. Keep `unwrap_err` only where demonstrating expected failure is the point.

### daml-parser-error-handling-002 — LOW — daml-parser / error-handling

- Location: `crates/daml-parser/src/parse.rs:171 (parse_module_strict docs)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines C-QUESTION-MARK says fallible examples should prefer ? over unwrap so copied examples do not normalize panicking error handling; C-FAILURE also expects error/panic behavior to be explicit. Source: https://rust-lang.github.io/api-guidelines/documentation.html.
- Evidence: The parse_module_strict rustdoc example uses unwrap() for a successful strict parse and unwrap_err() for an expected error at crates/daml-parser/src/parse.rs:168-175. README examples for render_lossless and render_from_ast also use unwrap() on public Result-returning APIs at crates/daml-parser/README.md:195-218. These are examples, not production code, but they are public copy-paste surfaces for fallible APIs.
- Recommendation: Rewrite fallible public examples to use hidden `fn main() -> Result<(), Box<dyn std::error::Error>>` plus `?` for success paths. For expected-error examples, prefer explicit `match`, `assert!(result.is_err())`, or a small helper that avoids suggesting unwrap as normal handling.

### daml-parser-interoperability-001 — LOW — daml-parser / interoperability

- Location: `crates/daml-parser/src/ast.rs:633 (DiagnosticCategory)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines interoperability recommends implementing common traits where they make sense, and standard display traits are the idiomatic user-facing formatting hook; documentation/error guidance also favors clear public contracts for diagnostics.
- Evidence: DiagnosticCategory is a public diagnostic classification enum with a public as_str method at crates/daml-parser/src/ast.rs:633-645, but there is no Display impl for it in the inspected impl list. Downstream formatter display code manually calls self.category.as_str() in crates/daml-fmt/src/lib.rs:101-113, and daml-lint maps the parser category explicitly in crates/daml-lint/src/parser.rs:73-83. Tests verify the stable string tags in crates/daml-parser/tests/diagnostics_recovery.rs:340-353.
- Recommendation: Add impl std::fmt::Display for DiagnosticCategory that delegates to as_str(). Keep as_str() for stable machine-readable tags, but let user-facing formatting use the standard Display path.

### daml-parser-type-safety-004 — LOW — daml-parser / type-safety

- Location: `crates/daml-parser/src/ast.rs:448 (ChoiceDecl::return_ty)`
- Actionable: yes
- Confidence: medium
- Principle: Rust API Guidelines C-CUSTOM-TYPE says Option can be ambiguous when multiple meanings are possible; Rust Book error-handling guidance says recoverable failures should be represented so callers can handle them; sources: https://rust-lang.github.io/api-guidelines/type-safety.html and https://doc.rust-lang.org/book/ch09-00-error-handling.html.
- Evidence: Several public AST fields use Option<Type> for both absence and failed parsing. ChoiceDecl::return_ty is documented as None if the type could not be parsed cleanly or the choice declared no return type at crates/daml-parser/src/ast.rs:451-453; TemplateBodyDecl::Key ty is None if absent or not cleanly parseable at crates/daml-parser/src/ast.rs:482-485. The parser emits diagnostics for malformed annotations via parse_type_annotation at crates/daml-parser/src/parse.rs:327-346, and tests verify malformed type diagnostics in crates/daml-parser/tests/diagnostics_recovery.rs:88-119, but the detached AST field itself loses absent-versus-malformed information.
- Recommendation: For public AST fields where absence and malformed input are distinct downstream states, consider a small enum such as TypeAnnotation::Present(Type), TypeAnnotation::Absent, TypeAnnotation::Malformed { span } or TypeParseState. Keep diagnostics as the detailed error channel, but avoid collapsing semantically different states into None.

### daml-parser-type-safety-005 — LOW — daml-parser / type-safety

- Location: `crates/daml-parser/src/lexer.rs:8 (Identifier / Operator / ModuleName)`
- Actionable: yes
- Confidence: medium
- Principle: Rust API Guidelines C-NEWTYPE supports static distinctions, but C-CUSTOM-TYPE and the Rust Book custom validation discussion favor constructors that preserve stated invariants when the type name implies one; sources: https://rust-lang.github.io/api-guidelines/type-safety.html and https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html.
- Evidence: Identifier, Operator, and ModuleName are distinct newtypes and implement common conversion/display traits, but From<String> and From<&str> accept any text at crates/daml-parser/src/lexer.rs:31-41, 109-119, and 187-197. Documentation describes them as identifier-like, symbolic operator text, and module-style qualified names at crates/daml-parser/src/lexer.rs:8, 86, and 164. Tests and AST construction use unchecked .into() values, e.g. crates/daml-parser/tests/ast_render.rs:16-19 and the wildcard sentinel described in finding 001.
- Recommendation: Decide and document whether these are purely tagged strings or validated lexical domain types. If invariants are intended, replace or supplement From<&str>/From<String> with TryFrom plus meaningful validation errors, and reserve unchecked constructors for parser-internal or test-only construction. If unchecked construction is intentional, document the relaxed invariant in rustdoc to avoid misleading downstream callers.

### daml-syntax-doc-002 — LOW — daml-syntax / documentation

- Location: `crates/daml-syntax/src/lib.rs:215 (LineIndex::utf16_col)`
- Actionable: yes
- Confidence: medium
- Principle: Rust API Guidelines documentation asks public contracts to explain behavior and edge cases, while Rust Book error-handling guidance distinguishes recoverable/expected invalid input from contract violations (https://rust-lang.github.io/api-guidelines/documentation.html, https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html).
- Evidence: `utf16_col` says it returns the UTF-16 code-unit offset from the start of `line` to `byte_col`, and only notes that the coordinate types cannot represent zero at crates/daml-syntax/src/lib.rs:215-218. The implementation accepts out-of-range line and byte-column values, using EOF for unknown lines and clamping the computed byte position to `source_len` at crates/daml-syntax/src/lib.rs:221-229. Existing tests cover normal coordinates and source-end range clamping, but not out-of-range line/column behavior, in crates/daml-syntax/src/lib.rs:630-720.
- Recommendation: Document the out-of-range contract for `utf16_col`: either state that callers must pass coordinates known to belong to this `LineIndex`, or explicitly describe the current EOF/source-length clamping behavior. If clamping is intended API behavior, add a small unit test that pins it.

### daml-syntax-doc-003 — LOW — daml-syntax / documentation

- Location: `crates/daml-syntax/README.md:40 (README doctest note)`
- Actionable: yes
- Confidence: high
- Principle: Rustdoc documentation-test guidance says documentation examples are extracted and run as tests so they stay up to date; documentation should not overstate what is actually tested (https://doc.rust-lang.org/rustdoc/write-documentation/documentation-tests.html).
- Evidence: The README states, "README examples are also compile-tested via `cargo test -p daml-syntax --doc`" at crates/daml-syntax/README.md:40. The passing doctest output shows tests from `crates/daml-syntax/src/lib.rs` line 24 and the hidden `readme_examples` module at line 589, while the README snippets are mirrored manually in crates/daml-syntax/src/lib.rs:586-603 rather than being included directly from README.md.
- Recommendation: Either include the README in rustdoc so the README code blocks themselves are tested, or rephrase the README note to say the README examples are mirrored in `src/lib.rs` doctests. This keeps the documentation process contract accurate and avoids stale README snippets being assumed covered automatically.

### daml-syntax-error-handling-002 — LOW — daml-syntax / error-handling

- Location: `crates/daml-syntax/src/lib.rs:508 (ParserSpanToTextRangeError::fmt)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines C-GOOD-ERR: error types should be meaningful and well-behaved, including useful Display messages (https://rust-lang.github.io/api-guidelines/interoperability.html).
- Evidence: `ParserSpanToTextRangeErrorKind` distinguishes `OutOfBounds`, `InvertedSpan`, `NonUtf8Boundary`, and `TextSizeOverflow`, but Display only gives a specific message for `TextSizeOverflow`; the other three kinds all format as `parser span [start, end) is invalid for source length len`. The integration tests in `crates/daml-syntax/tests/source_api.rs` assert these generic messages for out-of-bounds, inverted, and non-UTF-8-boundary spans.
- Recommendation: Keep the typed `kind()` accessor, but make Display messages kind-specific, for example by saying that the span is out of bounds, inverted, or not on a UTF-8 boundary. Update the tests to lock in those more actionable messages.

### daml-syntax-error-handling-003 — LOW — daml-syntax / error-handling

- Location: `crates/daml-syntax/src/coordinate.rs:54 (LineNumber::try_new / ByteColumn::try_new / CharColumn::try_new)`
- Actionable: yes
- Confidence: medium
- Principle: Rust Book ch09 recommends Result for expected recoverable failures; Rust API Guidelines C-CONV-TRAITS recommends standard conversion traits such as TryFrom where they make sense (https://doc.rust-lang.org/book/ch09-00-error-handling.html, https://rust-lang.github.io/api-guidelines/interoperability.html).
- Evidence: The one-based coordinate macro exposes `try_new(value: usize) -> Option<Self>` for zero rejection and `new(value)` panics on zero. Tests verify `try_new(0).is_none()`. Because invalid zero is expected caller input at crate boundaries, callers cannot use a typed error or standard `TryFrom<usize>` conversion with `?`; however, the single failure mode makes the current Option approach defensible and low severity.
- Recommendation: Consider adding standard `TryFrom<usize>` implementations for the one-based coordinate newtypes with a small typed error such as `InvalidOneBasedCoordinate`, while retaining or documenting the Option-returning constructor as a NonZero-style convenience. If the project intentionally prefers Option for the single zero case, document that rationale and leave the API unchanged.

### daml-syntax-interoperability-003 — LOW — daml-syntax / interoperability

- Location: `crates/daml-syntax/src/lib.rs:247 (SourceTokens / SourceFile)`
- Actionable: yes
- Confidence: medium
- Principle: Rust API Guidelines C-COMMON-TRAITS recommends eagerly implementing applicable common traits because downstream crates cannot add impls for foreign public types.
- Evidence: SourceTokens derives only Debug at lines 244-252 and SourceFile derives only Debug at lines 294-302. Their exposed semantic fields are immutable parsed/tokenized source data; upstream field types support Clone/PartialEq/Eq (for example daml-parser Module derives Debug, Clone, PartialEq, Eq at crates/daml-parser/src/ast.rs:596-608, and Token/Trivia/LexError derive Debug, Clone, PartialEq, Eq at crates/daml-parser/src/lexer.rs:291-390). Immediate tests and callers use accessors rather than comparing or cloning whole values, so this is an interoperability ergonomics gap rather than a current behavior bug.
- Recommendation: Consider implementing Clone, PartialEq, and Eq for SourceTokens and SourceFile if equality can intentionally ignore or normalize lazy OnceLock caches. Avoid a naive derive if initialized vs uninitialized cache state would make semantically identical values compare unequal; use manual impls if needed.

### daml-syntax-interoperability-004 — LOW — daml-syntax / interoperability

- Location: `crates/daml-syntax/src/lib.rs:491 (ParserSpanToTextRangeError::span_start_usize / span_end_usize)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines C-CONV-TRAITS prefers standard conversion traits over ad-hoc conversion helpers where the conversion is part of the public API.
- Evidence: ParserSpanToTextRangeError exposes typed ByteOffset accessors at lines 477-486, but also exposes span_start_usize() and span_end_usize() ad-hoc conversion helpers at lines 489-499. The only inspected uses are package tests in crates/daml-syntax/tests/source_api.rs:92-93; external immediate callers in daml-lint and daml-fmt use SourceFile, Coordinate::get, or try_parser_span_to_text_range but do not rely on these helpers.
- Recommendation: Prefer typed accessors plus standard conversions, e.g. usize::from(err.span_start()) after adding From<ByteOffset> for usize. If keeping span_start_usize/span_end_usize for ergonomics, document them as convenience wrappers rather than the primary conversion path and update tests to cover the standard conversion path.

### daml-syntax-type-safety-002 — LOW — daml-syntax / type-safety

- Location: `crates/daml-syntax/src/lib.rs:234 (LineIndex::utf16_range)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines type safety C-NEWTYPE/C-CUSTOM-TYPE: use deliberate domain types to distinguish interpretations and invariants; source: https://rust-lang.github.io/api-guidelines/type-safety.html.
- Evidence: LineIndex::utf16_range returns a bare (Utf16Offset, Utf16Offset). Both tuple elements have the same type, so start/end meaning and ordering are positional convention only. Immediate caller crates/daml-lint/src/ir.rs:42 destructures the tuple into start/end and then stores raw usize values at lines 47-48. Unit tests assert tuple equality at crates/daml-syntax/src/lib.rs:625-626, 654-656, and 716-719.
- Recommendation: Introduce a named public Utf16Range or Utf16Span struct with start and end fields/accessors. That keeps the UTF-16 coordinate-space newtype while also encoding the range endpoints by name, mirroring the clarity already provided by TextRange for byte ranges.

### daml-syntax-type-safety-003 — LOW — daml-syntax / type-safety

- Location: `crates/daml-syntax/src/lib.rs:473 (ParserSpanToTextRangeError::source_len)`
- Actionable: yes
- Confidence: medium
- Principle: Rust API Guidelines type safety C-NEWTYPE: newtypes statically distinguish different interpretations of the same primitive; Rust Book error-handling guidance also recommends using the type system to enforce valid inputs; sources: https://rust-lang.github.io/api-guidelines/type-safety.html and https://doc.rust-lang.org/book/ch09-03-to-panic-or-not-to-panic.html.
- Evidence: ParserSpanToTextRangeError stores span_start and span_end as ByteOffset at crates/daml-syntax/src/lib.rs:465-466, but source_len remains a raw usize at line 464 and is exposed as source_len() -> usize at line 473. The tests assert this raw value in crates/daml-syntax/tests/source_api.rs:86, 105, and 120. In a crate explicitly separating byte, char, UTF-16, line, and column spaces, this lone public usize length can be mixed with character or UTF-16 lengths by downstream callers.
- Recommendation: Expose the checked source length as a typed byte-space value, such as source_len_bytes() -> ByteOffset for the EOF byte offset or a new ByteLen newtype if length-vs-offset should remain distinct. Keep raw usize extraction explicit only at final interop boundaries.

### DFMT-EH-005 — INFO — daml-fmt / error-handling

- Location: `crates/daml-fmt/src/lib.rs:118 (FormatError)`
- Actionable: no
- Confidence: high
- Principle: Rust API Guidelines interoperability C-GOOD-ERR: public Result error types should be meaningful, implement Error/Display, and be Send/Sync where possible; Rust API Guidelines type safety encourages typed domain data over strings for meaningful distinctions.
- Evidence: `try_format_source_with_options` returns `Result<String, FormatError>`; `FormatError` stores `Vec<FormatDiagnostic>`, exposes diagnostics via an accessor, implements `Display` at lines 132-142 and `std::error::Error` at line 144. `FormatDiagnostic` exposes typed `LineNumber`, `CharColumn`, and `DiagnosticCategory` fields rather than only a string message. No public `Result<_, ()>` or stringly public error return was found in daml-fmt.
- Recommendation: No code change required for the existing formatter error type. Optionally add compile-time assertions for `FormatError: Send + Sync + 'static` if the project wants regression coverage for the auto-trait contract.

### daml-parser-interoperability-002 — INFO — daml-parser / interoperability

- Location: `crates/daml-parser/src/lexer.rs:257 (TokenKind)`
- Actionable: yes
- Confidence: medium
- Principle: Rust API Guidelines interoperability/common-traits guidance supports common trait impls for public types where semantically valid; Display is appropriate only if the output contract is documented as human-facing and not source-exact.
- Evidence: TokenKind is a public token enum exposed in README usage tables and consumed by daml-fmt, but it has no Display impl in the inspected impl list. The crate already has source-exact reconstruction APIs, so a TokenKind Display impl would need a clearly documented normalized, non-lossless contract. The layout unit tests currently define local token rendering logic for assertions in crates/daml-parser/src/layout.rs:275-300.
- Recommendation: Consider adding Display for TokenKind only if downstream users need normalized token spelling in diagnostics/logs. Document that it is not a lossless source renderer and keep render_lossless as the source-exact API. If no public need exists, leave this as-is.

### daml-parser-interoperability-003 — INFO — daml-parser / interoperability

- Location: `crates/daml-parser/src/parse.rs:19 (ParseModuleResult)`
- Actionable: yes
- Confidence: high
- Principle: Rust API Guidelines interoperability/common-traits guidance recommends common derives for public types where semantically valid and inexpensive.
- Evidence: ParseModuleResult is a public result wrapper with public Module and Vec<ParseDiagnostic> fields at crates/daml-parser/src/parse.rs:19-23, but derives only Debug and Clone. Its component types support equality, and analogous lexer output wrappers LexOutput and LexWithTriviaOutput derive Debug, Clone, PartialEq, Eq at crates/daml-parser/src/lexer.rs:597-614. Tests currently decompose ParseModuleResult with into_parts instead of comparing the wrapper directly.
- Recommendation: Derive PartialEq and Eq for ParseModuleResult to match the lexer output wrappers and make public API contract tests/downstream assertions more ergonomic.

### daml-syntax-error-handling-004 — INFO — daml-syntax / error-handling

- Location: `crates/daml-syntax/src/lib.rs:420 (try_parser_span_to_text_range)`
- Actionable: no
- Confidence: high
- Principle: Rust Book ch09: return Result for recoverable failures and use panic only for unrecoverable/contract-violation paths; Rust API Guidelines C-FAILURE and C-GOOD-ERR: document errors/panics and provide meaningful error types (https://doc.rust-lang.org/book/ch09-00-error-handling.html, https://rust-lang.github.io/api-guidelines/documentation.html, https://rust-lang.github.io/api-guidelines/interoperability.html).
- Evidence: The fallible parser-span conversion returns `Result<TextRange, ParserSpanToTextRangeError>`, documents its `# Errors`, exposes a typed non-exhaustive error kind, and implements Display and Error. The panicking wrappers at `SourceFile::parser_span_to_text_range` and free `parser_span_to_text_range` are documented under `# Panics`. Tests cover success plus out-of-bounds, inverted, and non-UTF-8-boundary failures. Immediate callers in daml-lint use the panicking wrapper for AST spans from the same SourceFile, while daml-fmt corpus tests use the fallible function to validate parser diagnostics before constructing SourceFile.
- Recommendation: No code change required for the overall span-conversion shape; keep the fallible API as the preferred path for external or untrusted spans and reserve the panicking wrappers for documented same-source parser-span contracts.


## Package/category summaries

### daml-fmt / documentation

- Status: completed
- Findings: 5
- Files inspected: 17
- Blockers: none

Completed a read-only documentation audit for daml-fmt. Crate-level docs, README examples, public DTO accessor docs, feature notes, CLI docs, tests, and doctests are generally strong; cargo test -p daml-fmt and doctests passed. Actionable documentation gaps remain around error-contract wording for CPP-conditional parser diagnostics, stale/incomplete workspace reference docs, CLI parser-diagnostic behavior, and two undocumented public FormatOptions methods.

### daml-fmt / error-handling

- Status: completed
- Findings: 5
- Files inspected: 21
- Blockers: none

Completed read-only error-handling audit for daml-fmt. The public formatting API is generally strong: malformed formatter input has typed diagnostics via FormatError/FormatDiagnostic, and the main CLI reports parser diagnostics without rewriting malformed files. Actionable gaps remain around one public metric API that silently accepts malformed input, discarded stdin IO error detail, dev-tool directory traversal panics for recoverable IO failures, and stale error API reference docs.

### daml-fmt / interoperability

- Status: completed
- Findings: 2
- Files inspected: 16
- Blockers: none

Completed a read-only interoperability audit for daml-fmt. The public library surface already uses typed diagnostics/options, implements Display and Error for FormatError, exposes typed diagnostic accessors, and has no existing public reader/writer API that violates the Rust API Guidelines C-RW-VALUE rule. Two low-severity interoperability gaps remain: ImportOrder has a documented semantic default and user-facing names but lacks Default/Display, and FormatError exposes its diagnostic slice only through an ad-hoc accessor rather than a standard borrowed-view trait.

### daml-fmt / type-safety

- Status: completed
- Findings: 3
- Files inspected: 16
- Blockers: none

Completed a read-only type-safety audit for daml-fmt. The public library API already uses several strong type-safety patterns: FormatOptions plus ImportOrder instead of boolean options, typed FormatDiagnostic/FormatError with Result-returning try_format_* APIs, and typed LineNumber/CharColumn accessors. I found two actionable internal type-safety hardening opportunities in layout_ast around primitive coordinate/group representations, plus one documentation mismatch that obscures the typed diagnostics contract.

### daml-lint / documentation

- Status: completed
- Findings: 5
- Files inspected: 19
- Blockers: none

Completed a read-only documentation audit for daml-lint. The crate has useful crate-level and README documentation, but public rustdoc for the rule-facing IR, error types, parser/report DTOs, and panic contracts is incomplete relative to the documented public API posture and Rust documentation guidelines.

### daml-lint / error-handling

- Status: completed
- Findings: 5
- Files inspected: 21
- Blockers: none

Read-only audit completed for daml-lint error handling. The crate generally uses Result for parse/config/script/load paths and has typed public severity/output-format parse errors, but several recoverable rule/config failure paths stringify nested errors and do not expose Error::source chains. The main actionable issues are source-chain loss in DetectError, ScriptLoadError, and ConfigError; a panic-first Detector::detect convenience API that can panic on recoverable custom-rule failures without a formal Panics contract; and a public parse_severity helper that discards SeverityParseError detail.

### daml-lint / interoperability

- Status: completed
- Findings: 6
- Files inspected: 16
- Blockers: none

Completed a read-only interoperability audit for daml-lint. Checked public exports, immediate callers, tests, README, Cargo metadata, and source-backed Rust API Guidelines/Rust Book principles. Findings focus on actionable trait/conversion/reader API gaps; no custom collection type was found where FromIterator/Extend clearly applies.

### daml-lint / type-safety

- Status: completed
- Findings: 4
- Files inspected: 23
- Blockers: none

Completed a read-only type-safety audit for daml-lint. The crate already uses several deliberate enums and typed contracts, including Severity, Consuming, ImportStyle, ParseDiagnosticCategory, and FindingLocation. I found four actionable public-API type-safety issues: primitive coordinate spaces in SourceSpan, primitive line/column report locations, an ambiguous Option-based detector override constructor, and a public severity parser that discards its meaningful error type.

### daml-parser / documentation

- Status: completed
- Findings: 7
- Files inspected: 21
- Blockers: none

Completed a read-only documentation audit for daml-parser. Crate-level docs and README are strong overall, there are no package feature flags to document, and `cargo test -p daml-parser` passes including doctests. Actionable gaps remain around stale README install version, one AST documentation mismatch, and incomplete public DTO/error/lexer contract docs.

### daml-parser / error-handling

- Status: completed
- Findings: 2
- Files inspected: 23
- Blockers: none

Completed a read-only daml-parser error-handling audit. The crate mostly follows the expected Result-vs-tolerant-diagnostics split: strict parse/render APIs return typed Result errors, public error enums implement Display/Error, and cargo test -p daml-parser passes. Two actionable issues remain: parser diagnostics are still string-centered and lose typed lexical source detail when folded into ParseDiagnostic, and public fallible examples still model unwrap-style handling.

### daml-parser / interoperability

- Status: completed
- Findings: 3
- Files inspected: 24
- Blockers: none

Completed a read-only interoperability audit for daml-parser. The crate already uses domain newtypes for identifier-like text, standard From/AsRef/Display impls for those newtypes, Result-returning strict/error APIs, Display+Error for error types, and has no applicable Read/Write stream APIs. Findings are limited to small interoperability polish items around public common trait impls and Display for public user-facing classifications.

### daml-parser / type-safety

- Status: completed
- Findings: 5
- Files inspected: 19
- Blockers: none

Completed read-only type-safety audit for daml-parser. The crate already uses several useful domain enums/newtypes and meaningful Result-returning APIs, and cargo test -p daml-parser passes. Actionable gaps remain around public AST shapes that encode distinct concepts with Option/String/usize rather than domain types, especially record fields, operator sections, type-annotation state, and source-coordinate spaces.

### daml-syntax / documentation

- Status: completed
- Findings: 3
- Files inspected: 15
- Blockers: none

Completed read-only documentation audit for daml-syntax. Crate-level docs, README quick starts, public dependency notes, panic/error sections, doctests, and package metadata are generally strong; `cargo test -p daml-syntax --doc`, `cargo test -p daml-syntax`, and `RUSTDOCFLAGS='-D warnings' cargo doc -p daml-syntax --no-deps` pass. Found only low/medium documentation contract gaps around `Diagnostic::end_column`, `LineIndex::utf16_col`, and the README doctest-process claim. No source edits were made.

### daml-syntax / error-handling

- Status: completed
- Findings: 4
- Files inspected: 13
- Blockers: none

Read-only audit completed for daml-syntax error handling. The crate generally follows the requested direction: malformed parse input is surfaced as diagnostics, invalid parser spans have a fallible Result API with a typed error, and panicking convenience APIs document their contracts. I found no high-severity issues; the actionable findings are around one silent-clamping coordinate API, more specific Display messages for typed span errors, and an optional typed Result/TryFrom path for one-based coordinate construction. No files were edited and git status remained clean.

### daml-syntax / interoperability

- Status: completed
- Findings: 4
- Files inspected: 20
- Blockers: none

Completed read-only interoperability audit for crates/daml-syntax. The crate already uses strong coordinate newtypes, exposes typed diagnostics through accessors, returns Result for recoverable span conversion, and documents/compile-tests its README examples. No reader/writer APIs and no clearly applicable custom collection FromIterator/Extend targets were found. Actionable gaps remain around standard conversion traits, Display/meaningful error output for the public span-conversion error kind, and common traits for public source aggregate types.

### daml-syntax / type-safety

- Status: completed
- Findings: 3
- Files inspected: 12
- Blockers: none

Completed a read-only type-safety audit for daml-syntax. The crate already shows strong type-safety for source coordinates: distinct byte/char/UTF-16/line newtypes, fallible span conversion, typed diagnostics, and compile-fail tests preventing byte/char column interchange. I found three narrow public-API issues where return shapes still leave meaning to convention rather than types.
