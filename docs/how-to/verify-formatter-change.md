# Verify a formatter change

Use these checks after changing `daml-fmt` behavior or updating the formatter
baseline.

Run commands from `crates/daml-fmt`.

## Run the fast corpus diff

Build the formatter and compare output with the committed `expected/` baseline:

```sh
cargo build --release --bin daml-fmt
npm test
```

`npm test` runs `node test/diff.js`, formats the 924-file corpus, compares
against `expected/`, and checks idempotence.

## Run the Rust verification harness

Run the default verifier:

```sh
tools/verify-rust.sh
```

This checks full-corpus idempotence. If the Daml SDK is on `PATH`, it also runs
the curated desugar-equivalence subset.

Skip desugar checks when the SDK is not available:

```sh
tools/verify-rust.sh --no-desugar
```

Run the full desugar sweep before accepting a risky formatter change:

```sh
tools/verify-rust.sh --desugar
```

## Regenerate the expected baseline

After an intentional formatting change, regenerate `expected/`:

```sh
tools/gen-expected.sh
```

Review the resulting diff before committing it.

## Produce review packets

Generate audit packets for human review:

```sh
npm run audit
```

Generate one batch only:

```sh
npm run audit -- --batch 7
```

Generate packets without SDK desugar checks:

```sh
npm run audit -- --no-desugar
```

The audit output is written to `target/daml-fmt-audit`. See the formatter
[audit workflow](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-fmt/docs/audit-workflow.md) for the packet
layout and reviewer responsibilities.

## Related

- [Formatter verification model](../explanation/formatter-verification.md)
- [`daml-fmt` crate README](https://github.com/stevennevins/daml-tools/blob/main/crates/daml-fmt/README.md)
