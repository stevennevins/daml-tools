// detector-accuracy-audit — reusable adversarial accuracy audit for daml-lint.
//
// Fans out one hunter per detector + the IR/parser substrate + the example
// rules; each hunter constructs positive/negative Daml cases and runs the BUILT
// binary to catch false positives / false negatives / crashes / wrong output.
// Every candidate is then independently verified (re-reproduced) before it is
// reported. This is the follow-up audit for the detector-accuracy roadmap
// (crates/daml-lint/docs/accuracy-roadmap.md).
//
// Run:
//   Workflow({ name: "detector-accuracy-audit" })
//   Workflow({ name: "detector-accuracy-audit",
//              args: { fixed: "describe what the latest round fixed",
//                      targets: ["unguarded_division", "ir_substrate"],   // optional subset
//                      focus: "any extra instructions" } })
//
// args (all optional):
//   repo    — repo root to operate in (default ".").
//   fixed   — prose listing what is ALREADY fixed, so hunters don't re-report it.
//   targets — array of target keys to probe (default: all).
//   focus   — extra hunting instructions appended to every target.

export const meta = {
  name: 'detector-accuracy-audit',
  description: 'Adversarially probe every daml-lint detector + IR/parser substrate + example rules for false positives/negatives, verify each finding',
  phases: [
    { title: 'Hunt', detail: 'one hunter per detector/area, empirical via the built binary' },
    { title: 'Verify', detail: 'independently re-reproduce each candidate bug' },
  ],
}

const REPO = (args && args.repo) || '.'
const FIXED = (args && args.fixed) || 'Nothing extra is documented as fixed; rely on reading the code for current behavior.'
const FOCUS = (args && args.focus) || ''

const HUNT_SCHEMA = {
  type: 'object',
  properties: {
    findings: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          title: { type: 'string' },
          area: { type: 'string', description: 'detector name / ir / parser / example-rule name' },
          kind: { type: 'string', enum: ['false-positive', 'false-negative', 'wrong-line', 'crash', 'wrong-output', 'other'] },
          repro_daml: { type: 'string', description: 'the exact minimal .daml source you ran' },
          command: { type: 'string', description: 'the exact daml-lint command you ran' },
          expected: { type: 'string' },
          actual: { type: 'string', description: 'what the binary actually produced (paste the relevant JSON/markdown)' },
          severity_guess: { type: 'string', enum: ['high', 'medium', 'low', 'nit'] },
        },
        required: ['title', 'area', 'kind', 'repro_daml', 'command', 'expected', 'actual', 'severity_guess'],
      },
    },
    coverage_notes: { type: 'string', description: 'what you probed and what you deliberately did NOT cover' },
  },
  required: ['findings', 'coverage_notes'],
}

const VERDICT_SCHEMA = {
  type: 'object',
  properties: {
    is_real: { type: 'boolean' },
    confidence: { type: 'string', enum: ['high', 'medium', 'low'] },
    corrected_severity: { type: 'string', enum: ['high', 'medium', 'low', 'nit', 'invalid'] },
    release_blocking: { type: 'boolean' },
    reasoning: { type: 'string', description: 'what you re-ran to confirm/refute' },
    fix_hint: { type: 'string' },
  },
  required: ['is_real', 'confidence', 'corrected_severity', 'release_blocking', 'reasoning', 'fix_hint'],
}

const ctx = `daml-lint is an EXPERIMENTAL, not-for-production static-analysis / security scanner for Daml, in the repo rooted at "${REPO}". You are hunting for DETECTOR-ACCURACY bugs: false positives (flags safe code) and false negatives (misses a real vulnerability), plus crashes / wrong line numbers / wrong output.

The built binary is at ${REPO}/target/release/daml-lint (build with \`cargo build --release --bin daml-lint\` from ${REPO} if missing). Built-in detectors: crates/daml-lint/src/detectors/{ensure_decimal,positive_amount,unbounded_fields,head_of_list,unguarded_division,archive_before_execute}.rs. IR lowering: crates/daml-lint/src/parser.rs + ir.rs. Shipped example rules: crates/daml-lint/examples/*.js (run with \`--rules <file>\`).

METHOD — be empirical, not theoretical: read the detector's logic, then CONSTRUCT a battery of minimal .daml files — cases that SHOULD flag (to catch false negatives) and cases that should NOT flag (to catch false positives) — write them to /tmp, run \`${REPO}/target/release/daml-lint /tmp/x.daml --format json\` (or markdown, or with --rules), and compare actual vs expected. Paste the actual output in each finding. Only report a finding you reproduced against the binary. If a detector behaves correctly across your battery, say so in coverage_notes and return few/no findings — do NOT invent nits.

ALREADY FIXED (do NOT re-report these; hunt for DIFFERENT / deeper issues):
${FIXED}

Judge release impact honestly: for an experimental scanner, most accuracy gaps are 'fix soon / document', not ship-stoppers; reserve high severity for false-'all-clear' on a COMMON pattern, or a crash.${FOCUS ? `\n\nEXTRA FOCUS: ${FOCUS}` : ''}`

const ALL_TARGETS = [
  { key: 'ensure_decimal', p: `TARGET: the ensure_decimal detector (missing-ensure-decimal). Probe Decimal/Numeric fields with/without ensure bounds; partial bounds (some fields bounded, others not); ensure using >, >=, /=, &&, ||, not(...), function calls; fields legitimately allowed to be zero; multiple templates. Find FPs and FNs, especially substring/structure mismatches.` },
  { key: 'positive_amount', p: `TARGET: the positive_amount detector (missing-positive-amount). Probe amount/quantity/price params guarded vs unguarded; strict > 0 vs >= 0 (zero allowed is a vuln); flipped 0 < amount; defensive \`when (amount <= 0) abort\`; guards in comments/strings; list/inputHoldingCids min-length checks. Find FPs and FNs.` },
  { key: 'unbounded_fields', p: `TARGET: the unbounded_fields detector. Probe Text/List/TextMap/Map/Set fields with/without a size bound in ensure; lower bound (length f > 0) vs upper bound (length f < N); bounds via length/size/T.length/Map.size; parenthesized/Optional-wrapped collection types; prefix-sibling field names. Find FPs and FNs.` },
  { key: 'head_of_list', p: `TARGET: the head_of_list detector (head-of-list-query). Probe head/last/(!!)/[x]-pattern on queryFilter results (should flag) vs on sorted/explicitly-ordered lists (FP?) vs on lists unrelated to a query (proximity FP?); duplicate findings; query result consumed safely. Find FPs and FNs.` },
  { key: 'unguarded_division', p: `TARGET: the unguarded_division detector. Probe x/y (spaced and unspaced), \`x \\\`div\\\` y\`, prefix \`div x y\`, denominators that are expressions x/(a+b), multiple divisions per line, division by literal constants, guards via ensure vs assert, guards before vs after the division, numeric-wrapper (intToDecimal) edge cases. Find FPs and FNs.` },
  { key: 'archive_before_execute', p: `TARGET: the archive_before_execute detector. Probe archival via archive / fetchAndArchive / \`exercise cid Archive\` before a try; archive inside the try body vs before it; multiple archives before one try; nested try/catch; archive in a let binding; comments/strings mentioning archive. Find FPs and FNs and double-reports.` },
  { key: 'ir_substrate', p: `TARGET: the IR/parser substrate (parser.rs lowering + ir.rs) that ALL detectors read. Probe type parsing of Numeric n, Map/Set/GenMap, qualified types, deeply nested Optional [ContractId Foo], tuples; statement classification of exercise/create/fetch with binders, record-update payloads, forA/mapA loops; signatory/observer/controller extraction with multiple parties or function calls; choice line/span correctness. Find lowering bugs that mislead detectors and rules.` },
  { key: 'example_rules', p: `TARGET: the shipped example rules crates/daml-lint/examples/*.js (the documented template users copy). Run EACH (.js) with --rules on crafted inputs that should and should not trigger it (list them with \`ls crates/daml-lint/examples/\`). Check rules that mis-fire, miss, crash, or rely on IR fields that are wrong/absent vs the documented daml-lint.d.ts contract.` },
]

const wanted = args && Array.isArray(args.targets) && args.targets.length
  ? ALL_TARGETS.filter((t) => args.targets.includes(t.key))
  : ALL_TARGETS

log(`Auditing ${wanted.length} target(s): ${wanted.map((t) => t.key).join(', ')}`)

const results = await pipeline(
  wanted,
  (t) => agent(`${ctx}\n\n${t.p}`, { label: `hunt:${t.key}`, phase: 'Hunt', schema: HUNT_SCHEMA })
    .then((r) => ({ key: t.key, findings: (r && r.findings) || [] })),
  (hunted) => parallel(
    (hunted.findings || []).map((f) => () =>
      agent(
        `${ctx}\n\nA hunter probing ${hunted.key} reported this candidate bug. Independently REPRODUCE it against the binary and try to REFUTE it. Re-run the exact repro, confirm actual vs expected, and decide if it is a genuine accuracy bug (not a reasonable design choice or a mis-expectation).\n\nTITLE: ${f.title}\nKIND: ${f.kind}\nREPRO .daml:\n${f.repro_daml}\nCOMMAND: ${f.command}\nEXPECTED: ${f.expected}\nACTUAL(claimed): ${f.actual}\n\nReturn is_real, corrected_severity (or 'invalid'), whether it blocks release of an experimental scanner, and a fix hint.`,
        { label: `verify:${hunted.key}`, phase: 'Verify', schema: VERDICT_SCHEMA }
      ).then((v) => ({ key: hunted.key, finding: f, verdict: v }))
    )
  )
)

const all = results.flat().filter(Boolean)
const confirmed = all.filter((x) => x.verdict && x.verdict.is_real && x.verdict.corrected_severity !== 'invalid')
const order = { high: 0, medium: 1, low: 2, nit: 3 }
confirmed.sort((a, b) => (order[a.verdict.corrected_severity] ?? 9) - (order[b.verdict.corrected_severity] ?? 9))

log(`Audit: ${all.length} candidates, ${confirmed.length} confirmed real, ${confirmed.filter((x) => x.verdict.release_blocking).length} release-blocking`)

return {
  candidates: all.length,
  confirmed_count: confirmed.length,
  blocking: confirmed.filter((x) => x.verdict.release_blocking).map((x) => x.finding.title),
  confirmed: confirmed.map((x) => ({
    area: x.finding.area,
    kind: x.finding.kind,
    severity: x.verdict.corrected_severity,
    release_blocking: x.verdict.release_blocking,
    title: x.finding.title,
    repro_daml: x.finding.repro_daml,
    expected: x.finding.expected,
    actual: x.finding.actual,
    fix_hint: x.verdict.fix_hint,
    verify: x.verdict.reasoning,
  })),
}
