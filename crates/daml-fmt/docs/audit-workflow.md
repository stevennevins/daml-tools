# daml-fmt audit workflow

This workflow audits the full 924-file formatter corpus by piping each original
sample through `daml-fmt` stdin, saving the formatted output, generating a
source-to-formatted diff, and running the compiler desugar equivalence check.

Run from `crates/daml-fmt`:

```sh
npm run audit
```

The default output directory is `target/daml-fmt-audit`. It contains:

- `SUMMARY.md` — run totals and links to each review batch.
- `samples.jsonl` — one machine-readable result per sample.
- `desugar-hashes.tsv` — original/formatted compiler desugar SHA-256 hashes.
- `formatted/` — formatter stdout for each sample.
- `diffs/` — source-to-formatted unified diffs.
- `batches/` — one review packet per subagent.
- `reviews/` — one note template per batch.
- `SUBAGENT_PROMPTS.md` — batch-specific prompts.

Default batching is 25 samples per subagent:

```sh
npm run audit -- --batch-size 25
```

For 924 samples this produces 37 batches; batches 1-36 contain 25 samples and
batch 37 contains 24 samples. A reviewer owns exactly one batch packet in
`target/daml-fmt-audit/batches`.

Reviewer responsibility:

- Inspect every diff and formatted file listed in the assigned batch.
- Judge whether the formatting is accurate and consistent with the documented
  `daml-fmt` layout rules.
- Treat formatter failures, expected-baseline mismatches, non-idempotence, or
  desugar mismatches as blocking.
- Record suspicious formatting, inconsistent choices, or unclear cases in the
  paired file under `target/daml-fmt-audit/reviews`.

The mechanical oracle is still the repo's established compiler check:

```sh
daml --no-legacy-assistant-warning damlc desugar <file> -o -
```

The audit writes that byte stream to temporary files and compares the bytes
before and after formatting. It reports byte equality and clean compiler exit
status separately, because a compiler output problem should be visible without
being confused with a semantic mismatch. This is the same semantic bar used by
`tools/verify-rust.sh --desugar`.

Useful variants:

```sh
npm run audit -- --batch 7        # generate only one 25-sample packet
npm run audit -- --no-desugar     # generate diff packets without SDK checks
FMT=/path/to/daml-fmt npm run audit
```
