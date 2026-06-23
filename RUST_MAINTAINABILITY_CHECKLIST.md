# Rust Maintainability Checklist

Use this checklist package-by-package. Keep changes small and verify each item in
the package where it applies before checking it off.

## daml-parser

- [ ] Enforce `rustfmt` and Clippy in CI for `daml-parser`.
- [ ] Confirm repo-wide lint and format settings apply to `daml-parser`.
- [ ] Confirm `daml-parser` declares the workspace Rust edition and MSRV.
- [ ] Keep `daml-parser` module boundaries explicit and responsibilities clear.
- [ ] Use idiomatic Rust naming for `daml-parser` types, traits, methods, and conversions.
- [ ] Document public `daml-parser` APIs with crate-level docs and rustdoc examples where useful.
- [ ] Use explicit error handling in `daml-parser`; return `Result` for recoverable failures and reserve panics for bugs or impossible states.
- [ ] Prefer clear, domain-specific `daml-parser` error types at public or important internal boundaries.
- [ ] Maintain a simple `daml-parser` test strategy: unit tests for local behavior and integration/API tests for public behavior.
- [ ] Keep `daml-parser` dependencies intentional; avoid adding crates for trivial functionality.
- [ ] Deny or tightly control unsafe code in `daml-parser`; require `SAFETY:` comments if unsafe is ever justified.
- [ ] Document `daml-parser` contributor commands and conventions in repo-level contributor guidance.
- [ ] Keep `daml-parser` PRs small, reviewable, and explicit about tests run and risk areas.

## daml-syntax

- [ ] Enforce `rustfmt` and Clippy in CI for `daml-syntax`.
- [ ] Confirm repo-wide lint and format settings apply to `daml-syntax`.
- [ ] Confirm `daml-syntax` declares the workspace Rust edition and MSRV.
- [ ] Keep `daml-syntax` module boundaries explicit and responsibilities clear.
- [ ] Use idiomatic Rust naming for `daml-syntax` types, traits, methods, and conversions.
- [ ] Document public `daml-syntax` APIs with crate-level docs and rustdoc examples where useful.
- [ ] Use explicit error handling in `daml-syntax`; return `Result` for recoverable failures and reserve panics for bugs or impossible states.
- [ ] Prefer clear, domain-specific `daml-syntax` error types at public or important internal boundaries.
- [ ] Maintain a simple `daml-syntax` test strategy: unit tests for local behavior and integration/API tests for public behavior.
- [ ] Keep `daml-syntax` dependencies intentional; avoid adding crates for trivial functionality.
- [ ] Deny or tightly control unsafe code in `daml-syntax`; require `SAFETY:` comments if unsafe is ever justified.
- [ ] Document `daml-syntax` contributor commands and conventions in repo-level contributor guidance.
- [ ] Keep `daml-syntax` PRs small, reviewable, and explicit about tests run and risk areas.

## daml-lint

- [ ] Enforce `rustfmt` and Clippy in CI for `daml-lint`.
- [ ] Confirm repo-wide lint and format settings apply to `daml-lint`.
- [ ] Confirm `daml-lint` declares the workspace Rust edition and MSRV.
- [ ] Keep `daml-lint` module boundaries explicit and responsibilities clear.
- [ ] Use idiomatic Rust naming for `daml-lint` types, traits, methods, and conversions.
- [ ] Document public `daml-lint` APIs with crate-level docs and rustdoc examples where useful.
- [ ] Use explicit error handling in `daml-lint`; return `Result` for recoverable failures and reserve panics for bugs or impossible states.
- [ ] Prefer clear, domain-specific `daml-lint` error types at public or important internal boundaries.
- [ ] Maintain a simple `daml-lint` test strategy: unit tests for local behavior and integration/API tests for public behavior.
- [ ] Keep `daml-lint` dependencies intentional; avoid adding crates for trivial functionality.
- [ ] Deny or tightly control unsafe code in `daml-lint`; require `SAFETY:` comments if unsafe is ever justified.
- [ ] Document `daml-lint` contributor commands and conventions in repo-level contributor guidance.
- [ ] Keep `daml-lint` PRs small, reviewable, and explicit about tests run and risk areas.

## daml-fmt

- [ ] Enforce `rustfmt` and Clippy in CI for `daml-fmt`.
- [ ] Confirm repo-wide lint and format settings apply to `daml-fmt`.
- [ ] Confirm `daml-fmt` declares the workspace Rust edition and MSRV.
- [ ] Keep `daml-fmt` module boundaries explicit and responsibilities clear.
- [ ] Use idiomatic Rust naming for `daml-fmt` types, traits, methods, and conversions.
- [ ] Document public `daml-fmt` APIs with crate-level docs and rustdoc examples where useful.
- [ ] Use explicit error handling in `daml-fmt`; return `Result` for recoverable failures and reserve panics for bugs or impossible states.
- [ ] Prefer clear, domain-specific `daml-fmt` error types at public or important internal boundaries.
- [ ] Maintain a simple `daml-fmt` test strategy: unit tests for local behavior and integration/API tests for public behavior.
- [ ] Keep `daml-fmt` dependencies intentional; avoid adding crates for trivial functionality.
- [ ] Deny or tightly control unsafe code in `daml-fmt`; require `SAFETY:` comments if unsafe is ever justified.
- [ ] Document `daml-fmt` contributor commands and conventions in repo-level contributor guidance.
- [ ] Keep `daml-fmt` PRs small, reviewable, and explicit about tests run and risk areas.

## References

- Rust API Guidelines checklist: <https://rust-lang.github.io/api-guidelines/checklist.html>
- Rust API Guidelines documentation: <https://rust-lang.github.io/api-guidelines/documentation.html>
- Rustfmt: <https://github.com/rust-lang/rustfmt>
- Rust Style Guide: <https://doc.rust-lang.org/style-guide/>
- Clippy lints: <https://rust-lang.github.io/rust-clippy/master/>
- Rust Book error handling: <https://doc.rust-lang.org/book/ch09-00-error-handling.html>
