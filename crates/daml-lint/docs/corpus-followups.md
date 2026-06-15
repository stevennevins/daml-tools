# Corpus audit follow-ups

This file tracks linter follow-ups found during the June 2026 manual corpus
audit over Daml compiler sources, Daml Finance, and adversarial fixtures under
`/tmp`.

The audit used:

```sh
/home/snevins/code/daml-lint/target/debug/daml-lint
```

## High priority

### Parse failures must be visible to automation — RESOLVED (2026-06-13)

Fixed. Parse/lex diagnostics are now carried alongside findings and surfaced
in every output mode, and a distinct exit code distinguishes invalid input
from a clean scan:

- Exit code `3` when any parse diagnostic was emitted (independent of
  `--fail-on`, which still governs findings severity; `0` clean, `1` findings
  over threshold, `2` usage/IO errors).
- Markdown prints a `## Parse Errors (N)` section and no longer claims
  `No findings.` for invalid input.
- JSON adds a top-level `parseErrors` array and `summary.parseErrors` count
  (additive — existing `findings`/`summary` consumers unaffected).
- SARIF reports parse errors as `invocations[].toolExecutionNotifications`
  with `executionSuccessful: false`, not as synthetic `results`.

Regression tests: `src/reporter.rs` (clean vs parse-error output in all three
formats). Original report below for context.

### Parse failures must be visible to automation

Repro:

```sh
/home/snevins/code/daml-lint/target/debug/daml-lint /tmp/contrarian/hostile/Unterm.daml
```

Observed behavior:

- Exit code is `0`.
- Stdout reports `No findings`.
- Stderr contains parse diagnostics for an unterminated string literal and an
  unterminated block comment.

Why it matters:

CI or editor integrations that check only exit status, stdout, JSON, or SARIF
can treat an invalid source file as clean.

Expected behavior:

- Invalid syntax should produce a non-zero exit code, or an explicit diagnostic
  in every output format.
- Stdout should not say `No findings` when parse diagnostics were emitted.
- JSON and SARIF should include parse diagnostics or a parse-error summary that
  callers can reliably detect.

## Medium priority

### Re-check structured outputs for invalid files

Re-run the malformed fixture through each output mode:

```sh
/home/snevins/code/daml-lint/target/debug/daml-lint --format markdown /tmp/contrarian/hostile/Unterm.daml
/home/snevins/code/daml-lint/target/debug/daml-lint --format json /tmp/contrarian/hostile/Unterm.daml
/home/snevins/code/daml-lint/target/debug/daml-lint --format sarif /tmp/contrarian/hostile/Unterm.daml
```

Acceptance criteria:

- Each mode exposes the parse failure without requiring stderr scraping.
- Exit status distinguishes clean scans from invalid input.
- Existing detector findings and `--fail-on` behavior remain unchanged for
  valid files.

### Review Daml Finance division findings — REVIEWED (2026-06-13)

A contrarian review rejected the original suggestion to suppress/downgrade
findings on "domain invariant" grounds. Domain invariants are non-local and
unverifiable from the AST; suppressing on them would hide real bugs, and a
security linter preferring false positives over false negatives is the correct
default. The acceptance criteria already say to keep findings absent a local
non-zero proof — so the 13 findings stay.

The review did find a genuine detector bug: `extract_denominator` grabbed the
numeric-conversion wrapper as the denominator, so the common idiom
`x / intToDecimal n` reported `Unguarded division by 'intToDecimal'` (9 of the
13 sites) and the guard search matched the wrapper name instead of `n`, making
guard detection inert for that idiom. Fixed in `src/detectors/unguarded_division.rs`
by skipping known numeric wrappers (`intToDecimal`, `intToNumeric`); the
denominator and guard search now target the real value. Finding counts are
unchanged for the finance corpus (no false guards matched) — only the messages
are now correct and guard detection works. Regression tests added.

Deliberately NOT done: literal-constant recognition (`if leap then 366 else 365`)
and `case denom of 0 ->` discriminator recognition. These add parsing complexity
to a text-based detector and carry false-negative risk for little gain.

Original notes below for context.

The finance sample produced 13 high-severity `unguarded-division` findings.
They look like reasonable conservative detections, but several may be protected
by domain invariants rather than local guards.

Review candidates:

- `/tmp/daml-finance/src/main/daml/Daml/Finance/Claims/V3/Util/Builders.daml:264`
- `/tmp/daml-finance/src/main/daml/Daml/Finance/Instrument/Swap/V0/Fpml/Util.daml:164`
- `/tmp/daml-finance/src/main/daml/Daml/Finance/Instrument/Swap/V0/Fpml/Util.daml:232`
- `/tmp/daml-finance/src/main/daml/Daml/Finance/Instrument/Swap/V0/Fpml/Util.daml:629`
- `/tmp/daml-finance/src/main/daml/Daml/Finance/Util/V4/Date/DayCount.daml:93`
- `/tmp/daml-finance/src/main/daml/Daml/Finance/Util/V4/Date/DayCount.daml:97`
- `/tmp/daml-finance/src/main/daml/Daml/Finance/Util/V4/Date/DayCount.daml:98`
- `/tmp/daml-finance/src/main/daml/Daml/Finance/Util/V4/Date/DayCount.daml:117`
- `/tmp/daml-finance/src/main/daml/Daml/Finance/Util/V4/Date/DayCount.daml:122`
- `/tmp/daml-finance/src/main/daml/Daml/Finance/Util/V4/Date/DayCount.daml:124`
- `/tmp/daml-finance/src/main/daml/Daml/Finance/Util/V4/Date/DayCount.daml:127`
- `/tmp/daml-finance/src/main/daml/Daml/Finance/Util/V4/Date/DayCount.daml:139`
- `/tmp/daml-finance/src/main/daml/Daml/Finance/Util/V4/Date/DayCount.daml:162`

Acceptance criteria:

- Keep findings when no local non-zero proof is visible.
- Suppress or downgrade only when the detector can identify a clear guard or
  a defensible invariant pattern.
- Add regression fixtures for any newly recognized guard pattern.

## Regression cases to add

- Invalid string and block comment input must not scan as clean.
- JSON and SARIF must expose parse diagnostics for invalid input.
- Linting formatter output for valid files should preserve finding counts.
- Comment-heavy valid files should continue to parse without false parse errors.

Useful audit files:

- `/tmp/contrarian/hostile/Comments.daml`
- `/tmp/contrarian/hostile/Layout.daml`
- `/tmp/contrarian/hostile/Unicode.daml`
- `/tmp/daml-finance/src/main/daml/ContingentClaims/Valuation/V0/Stochastic.daml`
- `/tmp/daml-repo/sdk/daml-script/test/daml/ScriptTest.daml`
