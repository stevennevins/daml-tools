// detector-accuracy-fix — land fixes for confirmed daml-lint accuracy findings.
//
// Companion to detector-accuracy-audit.js. The audit finds + verifies bugs;
// this workflow FIXES them. One fixer agent per cluster, run SEQUENTIALLY in the
// shared working tree (the clusters touch shared files like ir.rs / parser.rs,
// so parallel editing would conflict). Each fixer decides on the structured
// Expr/IR (never raw text), writes a failing-first regression test per finding,
// and self-verifies (cargo test + clippy + fmt) before the next fixer starts.
// A final agent runs the full gauntlet.
//
// Run:
//   Workflow({ name: "detector-accuracy-fix",
//              args: { auditFile: "/abs/path/to/audit-output.json",
//                      repo: ".",
//                      clusters: [ { name, files, ids:[...], hint } , ... ] } })
//
// args:
//   repo       — repo root (default ".").
//   auditFile  — absolute path to a detector-accuracy-audit output JSON. Each
//                fixer reads result.confirmed[<id>] for the findings it owns.
//   clusters   — ORDERED list of clusters; independent detector files first, the
//                parser/cross-cutting cluster last. Each: { name, files, ids[],
//                hint? }. If omitted, nothing runs.

export const meta = {
  name: 'detector-accuracy-fix',
  description: 'Fix confirmed daml-lint accuracy findings, one fixer per cluster, sequentially (shared-tree safe), each TDD + self-verify, then a full gauntlet gate',
  whenToUse: 'After detector-accuracy-audit confirms findings: pass the audit output + ordered clusters to land the fixes.',
  phases: [
    { title: 'Fix', detail: 'one fixer per cluster, in order, editing the shared tree' },
    { title: 'Verify', detail: 'full gauntlet: tests, fmt, clippy, deny, daml-fmt diff' },
  ],
}

// `args` is honored if present (and tolerated as a JSON string), but the
// clusters + audit path are embedded as defaults so the run is robust to args
// not threading through.
let A = args
if (typeof A === 'string') {
  try {
    A = JSON.parse(A)
  } catch (e) {
    A = {}
  }
}
A = A || {}

const DEFAULT_AUDIT =
  '/tmp/claude-1000/-home-snevins-code-daml-tools/1c27e6e2-34ae-4f35-8882-5a7de5dca072/tasks/w2a3w7jto.output'

const DEFAULT_CLUSTERS = [
  {
    name: 'unbounded_size_bound',
    files: ['crates/daml-lint/src/ir.rs', 'crates/daml-lint/src/detectors/unbounded_fields.rs'],
    ids: [5, 6],
    hint: 'is_size_upper_bound currently accepts ANY rhs. Require the bounding operand to be a NUMERIC LITERAL constant: `length f < cap` where cap is a sibling field/variable does NOT bound the size, and `length a == length b` bounds neither collection. Keep `length f < 280`, `280 > length f`, and exact `length f == 3` passing.',
  },
  {
    name: 'positive_amount_list',
    files: ['crates/daml-lint/src/detectors/positive_amount.rs', 'crates/daml-lint/src/ir.rs'],
    ids: [0, 1, 2],
    hint: 'The list min-length check is still text (body_raw.contains). Make it STRUCTURAL: a list param is guarded only by a guaranteed NON-EMPTY assertion — `not (null p)` / `not $ null p`, or a strict lower bound on its length (`length p > 0`, `length p >= 1`, `0 < length p`, `1 <= length p`, `length p /= 0`) — matched on the Expr tree by the param name (refers_to). An UPPER bound `length p < N`/`<= N` must NOT suppress. A check naming a DIFFERENT or superstring-named list (inputHoldingCidsBackup) must NOT suppress. Delete the crude secondary inputHoldingCids/.inputs/maxNumInputs body_raw block.',
  },
  {
    name: 'ensure_decimal_edges',
    files: ['crates/daml-lint/src/ir.rs', 'crates/daml-lint/src/detectors/ensure_decimal.rs'],
    ids: [21],
    hint: '`ensure amount == <positive literal>` pins the field to a positive value, so it IS a positivity bound. Extend is_nonneg_bound to accept `==` when one side refers to the field and the other is a POSITIVE (non-zero) numeric literal. `amount == 0` must still flag.',
  },
  {
    name: 'division_edges',
    files: ['crates/daml-lint/src/detectors/unguarded_division.rs', 'crates/daml-lint/src/ir.rs'],
    ids: [10, 26],
    hint: '[10] a guard on the SAME physical line as the division (one-line do-block) is dropped because ordering uses strict line `<`; use statement/iteration order so a guard that runs before the division on the same line suppresses. [26] division by a non-zero NEGATIVE literal (`x / (-2.0)` = Neg(Lit)) is safe — treat a negative numeric literal as non-zero. Do NOT regress the if-guard, ensure, disjunction, or guard-after-division tests.',
  },
  {
    name: 'head_of_list_edges',
    files: ['crates/daml-lint/src/detectors/head_of_list.rs'],
    ids: [7, 8, 22, 23, 24],
    hint: '[8] (medium) flag monadic destructuring directly from a query bind: `[x] <- query ...` and `(x :: _) <- query ...` — the bind PATTERN is the head-of-list, check the binder pattern of a query-app statement. [7] (medium) a nested `case` on an UNRELATED list inside a query-result case is falsely flagged — fix the body_raw block-scope/indent detection so the inner case is not attributed to the outer query scrutinee. [22][23][24] are low: dropping a re-bound (sorted) name, alias dataflow, and inline `head (query ...)` — do the cheap ones, defer the rest with a note.',
  },
  {
    name: 'example_rules',
    files: [
      'crates/daml-lint/examples/unguarded-division-ast.js',
      'crates/daml-lint/examples/unguarded-division-ast.ts',
      'crates/daml-lint/examples/consuming-choice-signatory-controller.js',
      'crates/daml-lint/examples/consuming-choice-signatory-controller.ts',
      'crates/daml-lint/examples/no-trace.js',
      'crates/daml-lint/examples/no-trace.ts',
    ],
    ids: [19, 20, 27, 28],
    hint: "Keep each .ts source and its .js runtime IN SYNC. [19] unguarded-division-ast: an assert inside an if/case BRANCH is not an unconditional guard — only count asserts guaranteed to run. [20] consuming-choice-signatory-controller: startsWith('signatory') substring-matches a non-signatory field — match the keyword as a whole token. [27][28] no-trace: `trace` inside `{- -}` block comments and inside string literals must not be flagged. Verify each rule empirically with the built binary via --rules.",
  },
  {
    name: 'parser_conditional_flatten',
    files: [
      'crates/daml-lint/src/parser.rs',
      'crates/daml-lint/src/detectors/unguarded_division.rs',
      'crates/daml-lint/src/detectors/positive_amount.rs',
      'crates/daml-lint/src/detectors/archive_before_execute.rs',
      'crates/daml-lint/src/ir.rs',
    ],
    ids: [3, 4, 9, 11, 12, 13, 14, 15, 16, 17, 18, 25],
    hint: 'ARCHITECTURAL ROOT — highest blast radius, do it LAST and carefully. parser.rs `collect_actions` FLATTENS if/case/when branches and let-bound helpers, hoisting conditional asserts/archives to top level so they lose conditionality: conditional guards are misread as unconditional (FN: [4][9][17][18][25]) and conditional/uncalled archives are misread as before-try (FP/FN: [11][12][13][14][15][16]). Preferred fix: stop flattening control flow in lower_do/collect_actions — preserve if/case as a structured Other{expr:If/Case} statement and make the division/positive_amount guard detectors and the archive detector walk into branches CONDITIONALLY, while still surfacing create/exercise for the JS rules. This MUST keep every existing detector test, the script.rs node-kind/`test_every_node_kind_reaches_scripts` tests, the corpus/adversarial tests, AND the daml-fmt 924/924 differential green. ALSO fix [3] (cheap, do it regardless): parser `classify_app` only recognizes UNQUALIFIED assert/assertMsg — accept qualified `DA.Assert.assertMsg`/`assert`. If the full structural change cannot be landed green, fix [3] plus the safest subset and DEFER the rest with a precise explanation for a dedicated round — do NOT leave the tree broken or the formatter differential regressed.',
  },
]

const REPO = A.repo || '.'
const AUDIT = A.auditFile || DEFAULT_AUDIT
const CLUSTERS = Array.isArray(A.clusters) && A.clusters.length ? A.clusters : DEFAULT_CLUSTERS

const FIX_SCHEMA = {
  type: 'object',
  properties: {
    applied: { type: 'boolean', description: 'did you change code to fix the findings?' },
    findings_fixed: { type: 'array', items: { type: 'number' }, description: 'finding ids you actually fixed' },
    findings_deferred: { type: 'array', items: { type: 'number' }, description: 'ids you intentionally did NOT fix (too risky / out of scope), with why in notes' },
    tests_added: { type: 'array', items: { type: 'string' }, description: 'names of regression tests you added' },
    files_changed: { type: 'array', items: { type: 'string' } },
    lib_green: { type: 'boolean', description: 'cargo test -p daml-lint --lib passed after your change' },
    clippy_clean: { type: 'boolean', description: 'cargo clippy -p daml-lint ... -D warnings clean' },
    notes: { type: 'string', description: 'what you changed and why; for any deferred finding, the reason' },
  },
  required: ['applied', 'findings_fixed', 'tests_added', 'lib_green', 'clippy_clean', 'notes'],
}

const VERIFY_SCHEMA = {
  type: 'object',
  properties: {
    gates: {
      type: 'array',
      items: {
        type: 'object',
        properties: {
          name: { type: 'string' },
          pass: { type: 'boolean' },
          detail: { type: 'string' },
        },
        required: ['name', 'pass', 'detail'],
      },
    },
    all_green: { type: 'boolean' },
    summary: { type: 'string' },
  },
  required: ['gates', 'all_green', 'summary'],
}

const baseCtx = `daml-lint is a static-analysis scanner for Daml, rooted at "${REPO}". Round 3 moved every detector's guard/bound/ordering decision onto the structured \`ir::Expr\` / statement tree; the text-scan helpers were deleted. You are FIXING confirmed accuracy bugs while preserving that north star.

HOW TO READ YOUR FINDINGS: the audit output is JSON at ${AUDIT}. Read it and pull the findings you own:
  python3 -c "import json; d=json.load(open('${AUDIT}')); r=d.get('result',d); r=json.loads(r) if isinstance(r,str) else r; c=r['confirmed']; [print(i,'::',c[i]['title'],'\\nREPRO:\\n',c[i]['repro_daml'],'\\nEXPECTED:',c[i]['expected'],'\\nHINT:',c[i]['fix_hint'],'\\n---') for i in __IDS__]"
(substitute your ids for __IDS__, e.g. [0,1,2]).

REUSABLE SUBSTRATE (prefer these over new text scanning): crates/daml-lint/src/ir.rs already has Expr predicates/walkers — \`Expr::conjuncts\` (top-level && only), \`Expr::ref_string\` / \`refers_to\` (structural field/var match, handles this./self.), \`is_zero_lit\` / \`is_nonzero_numeric_lit\` / \`is_nonneg_numeric_lit\`, \`is_nonneg_bound\` / \`is_nonzero_bound\` / \`is_strict_positive_bound\`, \`expr_guarantees_nonzero\` / \`expr_guarantees_strict_positive\`, and the walkers \`walk_body_exprs\` / \`walk_body_stmts\` / \`statement_exprs\` / \`child_exprs\` / \`for_each_subexpr\`. Add new shared predicates here when several detectors need them.

RULES (non-negotiable):
- Decide on the Expr / statement tree, NOT on raw_text / body_raw substrings. raw_text is for evidence/display only.
- TDD: add a FAILING-FIRST regression test for each finding (parse a minimal .daml via crate::parser::parse_daml, assert the corrected behavior), confirm it fails, then fix.
- Keep EVERY existing test green. Run \`cargo test -p daml-lint --lib\`, \`cargo fmt -p daml-lint\`, and \`cargo clippy -p daml-lint --all-targets --all-features -- -D warnings\`. NEVER leave the tree broken — if a fix can't be made green, REVERT it and report it as deferred with the reason.
- Surgical: touch only what the findings require; match surrounding style and comment density.
- A finding that is a reasonable design choice or needs a risky cross-cutting change you can't make safely: defer it, and say exactly why in notes.`

phase('Fix')
log(`Fixing ${CLUSTERS.length} cluster(s): ${CLUSTERS.map((c) => c.name).join(', ')}`)

const results = []
for (const c of CLUSTERS) {
  const prompt = `${baseCtx}

YOUR CLUSTER: "${c.name}". Primary file(s): ${(c.files || []).join(', ') || '(discover from the findings)'}.
Findings you own (result.confirmed indices): [${(c.ids || []).join(', ')}].
${c.hint ? `\nCLUSTER GUIDANCE: ${c.hint}\n` : ''}
Read each finding from ${AUDIT}, reproduce it against the built binary if useful (\`cargo build --release --bin daml-lint\` then \`${REPO}/target/release/daml-lint /tmp/x.daml --format json\`), implement the fix on the Expr/IR tree, add a failing-first regression test per finding, and self-verify. Report what you fixed, deferred, and the test names.`
  const r = await agent(prompt, { label: `fix:${c.name}`, phase: 'Fix', schema: FIX_SCHEMA })
  results.push({ cluster: c.name, ...(r || {}) })
  log(
    `${c.name}: applied=${r && r.applied} lib_green=${r && r.lib_green} clippy=${r && r.clippy_clean} fixed=${r && (r.findings_fixed || []).length} deferred=${r && (r.findings_deferred || []).length}`,
  )
}

phase('Verify')
const verify = await agent(
  `${baseCtx}

All cluster fixers have run. Run the FULL gauntlet from ${REPO} and report each gate pass/fail with detail:
  1. cargo test --workspace --all-features
  2. cargo fmt --all --check
  3. cargo clippy --workspace --all-targets --all-features -- -D warnings
  4. cargo deny check
  5. daml-fmt differential: cd crates/daml-fmt && node test/diff.js  (expect "924 files: 924 ok")
Do NOT change code except to repair a clear regression a fixer introduced (and say so). If a gate fails, capture the exact failing output in the gate detail.`,
  { label: 'verify:gauntlet', phase: 'Verify', schema: VERIFY_SCHEMA },
)

const fixedCount = results.reduce((n, r) => n + ((r.findings_fixed || []).length), 0)
const deferred = results.flatMap((r) => (r.findings_deferred || []).map((id) => ({ cluster: r.cluster, id })))
log(`Fixed ${fixedCount} finding(s); ${deferred.length} deferred; gauntlet all_green=${verify && verify.all_green}`)

return {
  fixed_count: fixedCount,
  deferred,
  per_cluster: results.map((r) => ({
    cluster: r.cluster,
    fixed: r.findings_fixed || [],
    deferred: r.findings_deferred || [],
    lib_green: r.lib_green,
    clippy_clean: r.clippy_clean,
    tests: r.tests_added || [],
    notes: r.notes,
  })),
  verify,
}
