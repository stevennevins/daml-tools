---
description: How daml-fmt uses token checks, compiler desugar equivalence, corpus baselines, and human audit to protect source semantics.
---

# Formatter verification

`daml-fmt` changes source text, so its central safety question is not whether
the output looks nicer. The central question is whether formatting preserves
the program.

The formatter's verification model layers several checks because no single
check explains every kind of safety. Token equivalence protects the formatter
while it constructs output. Desugar equivalence checks the compiler's view of
the result. Corpus diffing and expected baselines make changes reviewable.
Idempotence prevents unstable formatting. Audit packets make large corpus
reviews tractable for humans.

Together, these checks let the formatter make layout decisions while keeping
semantic risk visible.

## Token equivalence is a construction gate

The formatter is built on `daml-parser`, which records source tokens, trivia,
layout, and byte spans. Formatting works by rewriting layout around known
syntax while preserving comments, strings, and unmodeled regions.

For pure reindentation and final whitespace normalization, candidate output is
re-lexed and compared against the previous laid-out token stream, including
virtual layout tokens for Daml's offside rule. If the tokens differ, the
formatter falls back to a safer output and can ultimately return the input
unchanged.

This gate matters because spacing can change meaning in subtle ways. For
example, changing whitespace around operators or layout-sensitive blocks can
alter the token stream even when the text looks harmless. Token equivalence
keeps the formatter from returning a rewrite that changes the parser's view of
the program.

Some formatter rules intentionally change layout form, such as expanding inline
expressions or organizing imports. Those rules are not token-equivalence
preserving; their safety is checked by focused fixtures, idempotence, and the
compiler desugar oracle.

## Desugar equivalence is the semantic oracle

Token equivalence is a strong local guard, but the compiler remains the
authority on Daml semantics. The formatter therefore also checks desugar
equivalence: the original file and the formatted file are passed through the
Daml compiler's desugar step, and the resulting byte streams are compared.

If desugared output is byte-identical, the compiler has seen the same program
after formatting. This is the highest semantic bar in the formatter's
verification story.

Import organization is the one narrower comparison. Reordering imports can
change package identity even when the import set and program body are
unchanged, so the verifier falls back to comparing desugar output with import
declarations sorted. That still catches changed program bodies and added or
removed imports, while permitting import-order/package-identity noise for this
rule.

The distinction between token equivalence and desugar equivalence is important.
Token equivalence explains why individual formatter rewrites are designed to be
safe. Desugar equivalence verifies that safety against the compiler over real
files.

## Corpus diffing makes behavior reviewable

The formatter is tested over a large corpus of real Daml files. The corpus is
not only a regression set; it is a source of design pressure. It exposes syntax
combinations, comments, templates, interfaces, scripts, records, choices, and
layout patterns that small handwritten examples would miss.

For each corpus file, the formatter output is compared with the committed
`expected/` baseline. That baseline is the formatter's current chosen layout,
not an independent definition of correctness. A baseline mismatch means the
formatter's behavior changed and the diff needs review.

This keeps layout evolution explicit. When a formatter rule changes
intentionally, the expected output changes with it. When output changes
unexpectedly, corpus diffing makes the change visible before it becomes a
release artifact.

## Idempotence prevents formatting drift

A formatter must reach a fixed point. After one pass, formatting the result
again should produce the same result.

Idempotence catches unstable rules: rules that toggle spacing, move layout back
and forth, or expose a second change only after the first pass. Without
idempotence, users could see different output each time they run the formatter,
and diffs would become noisy even when the program is unchanged.

In `daml-fmt`, idempotence is checked across the full corpus. This makes it a
property of the formatter's behavior over realistic source, not only over
isolated unit examples.

## Audit packets add human judgment

Mechanical checks can prove important facts, but they do not decide whether the
chosen layout is readable, consistent, or surprising. The audit workflow
packages corpus samples with formatted output, diffs, desugar results, and
review notes so humans can inspect formatter behavior in batches.

Audit packets are especially useful because formatter quality is partly
aesthetic and partly semantic. A file can be desugar-equivalent and idempotent
while still showing a poor layout choice. Conversely, a visually large diff may
be acceptable if it is consistent, stable, and semantically unchanged.

The audit workflow separates these concerns. Mechanical failures, such as
formatter crashes, expected-baseline mismatches, non-idempotence, or desugar
mismatches, are blocking. Human reviewers then judge the formatting choices
that remain.

## What the verification model protects

The verification model protects three different promises:

- The formatter should not change what the Daml compiler sees.
- The formatter should produce stable output.
- Formatter behavior changes should be visible and reviewable.

It does not claim that every possible Daml construct is fully modeled.
Unmodeled constructs can pass through verbatim. That is a deliberate tradeoff:
it is safer for the formatter to leave syntax alone than to guess at a layout
it cannot yet justify.

The result is a conservative formatter architecture. It can grow by modeling
more syntax over time, but each new rule has to pass through the relevant
safety layers: preserve the token stream where the rule is pure reindentation,
preserve compiler desugaring, stay idempotent, update expected baselines
deliberately, and survive human audit.
