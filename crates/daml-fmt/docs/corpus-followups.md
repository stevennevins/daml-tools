# Corpus audit follow-ups

This file tracks formatter follow-ups found during the June 2026 manual corpus
audit over Daml compiler sources, Daml Finance, and adversarial fixtures under
`/tmp`.

The audit used:

```sh
/home/snevins/code/daml-fmt/target/release/daml-fmt
```

## Resolved (June 2026 contrarian review)

### Malformed input should not look successfully formatted — FIXED

Repro:

```sh
/home/snevins/code/daml-fmt/target/release/daml-fmt /tmp/contrarian/hostile/Unterm.daml
```

Was: exit `0`, empty stderr, output byte-equivalent to input despite an
unterminated string and unterminated block comment. Formatter success could be
mistaken for parse success.

Fix: `lex_diagnostics` (src/lib.rs) surfaces the lexer's existing
unterminated-string / unterminated-block-comment errors. The CLI now prints a
`file: line:col: message` diagnostic to stderr and exits `2` for malformed
input in every mode (stdin, stdout, `--check`, `-w`); `-w` never rewrites a
malformed file. Output stays a byte-faithful passthrough (`format_source`
unchanged). All 924 corpus files lex clean, so none are flagged — idempotence
and desugar-safety guarantees are untouched.

### Duplicate space after type-signature colon — FIXED

Was: `name:  Type` (two spaces after the colon) passed through unchanged, while
`name : Type` normalized to `name: Type`. Asymmetric.

Fix: `normalize_gaps` (src/lib.rs) now collapses same-line space(s) *after* a
canonicalized type-annotation colon to one space, reusing the same gate as the
before-colon collapse (only at brace/paren depth 0, never after `)`, never
across a newline). Six corpus files normalized (`DA/Logic.daml`,
`RecordDotUpdates.daml`, four `upgrades/stable/*.daml` continuation-line
colons); all idempotent and laid-out-token-equivalent. `expected/`
regenerated.

### Unicode identifier type signatures — NOT REPRODUCIBLE (struck)

Repro:

```sh
/home/snevins/code/daml-fmt/target/release/daml-fmt /tmp/contrarian/hostile/Unicode.daml
```

The doc claimed `émoji : Text` stayed unchanged while ASCII `zwj : Text`
normalized. Verified twice: the current binary normalizes *every* signature
colon identically — `émoji : Text` -> `émoji: Text` like `zwj : Text` ->
`zwj: Text`. The lexer tokenizes non-ASCII identifiers, so the lone-colon
collapse fires regardless of script. Unicode in strings/comments stays
byte-preserved. Finding does not reproduce; kept as a regression case below.

## Comment and layout review cases

The audit did not find direct comment loss or doc-comment detachment in the
sample, but these cases should become regression tests before expanding layout
coverage:

- Non-ASCII identifier signature colon consistency:
  `/tmp/contrarian/hostile/Unicode.daml`
- Nested block comments:
  `/tmp/contrarian/hostile/Comments.daml`
- `-- |` and `-- ^` documentation comments:
  `/tmp/daml-repo/sdk/compiler/damlc/daml-stdlib-src/DA/Internal/LF.daml`
  `/tmp/daml-repo/sdk/compiler/damlc/tests/daml-test-files/LfInterfaces.daml`
  `/tmp/daml-finance/src/main/daml/Daml/Finance/Interface/Account/V4/Account.daml`
- Commented-out FpML fields followed by docs:
  `/tmp/daml-finance/src/main/daml/Daml/Finance/Interface/Instrument/Swap/V0/Fpml/FpmlTypes.daml`
- Parser syntax fixtures with unusual existing layout:
  `/tmp/daml-repo/sdk/compiler/damlc/tests/daml-test-files/ChoiceSyntaxes.daml`
- Heavy case/if indentation rewrites:
  `/tmp/daml-repo/sdk/compiler/damlc/daml-stdlib-src/DA/List.daml`
- Large script and upgrade diffs:
  `/tmp/daml-repo/sdk/daml-script/test/daml/ScriptTest.daml`
  `/tmp/daml-repo/sdk/daml-script/test/daml/upgrades/stable/ContractKeys.daml`

Acceptance criteria:

- `format(format(x)) == format(x)` for each case.
- Comments remain present and attached to the same declarations or fields.
- Pragmas and block-comment interiors are preserved.
- Any new rewrite remains desugar-equivalent on buildable SDK 3.4.11 corpus
  files.

## Regression commands

Per-file idempotence check:

```sh
f=/tmp/contrarian/hostile/Unicode.daml
a=$(mktemp)
b=$(mktemp)
/home/snevins/code/daml-fmt/target/release/daml-fmt "$f" > "$a"
/home/snevins/code/daml-fmt/target/release/daml-fmt "$a" > "$b"
cmp -s "$a" "$b"
```

Corpus oracle:

```sh
npm test
tools/verify-rust.sh
tools/verify-rust.sh --desugar  # optional full-corpus desugar sweep
```

Keep using temporary outputs for external corpora. Do not rewrite files under
`/tmp/daml-repo`, `/tmp/daml-finance`, or `/tmp/contrarian` while investigating.
