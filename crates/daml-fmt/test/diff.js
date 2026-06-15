// Differential test harness (CLAUDE.md plan item 5, fast tier).
//
// For every file in the 924-file corpus manifest:
//   1. format(original) must equal the committed expected/ baseline
//   2. format(format(original)) must equal format(original)  (idempotence)
//
// The formatter under test is the SHIPPED Rust backend (src/lib.rs ->
// layout_ast), invoked via the release binary. Build it first with
// `cargo build --release` (this script will build it if missing).
//
// Exits 1 on any mismatch. This is the deterministic, SDK-free tier;
// the desugar parse/equivalence sweeps (the real semantic bar) are the
// shell commands in CLAUDE.md "Verify commands" and need the Daml SDK.
const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");

const repoRoot = path.resolve(__dirname, "..");
// In a Cargo workspace the binary builds into the shared workspace target dir,
// not crates/daml-fmt/target. Ask cargo where it is so this works in both.
const targetDir = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version", "1", "--no-deps"], {
    cwd: repoRoot,
    maxBuffer: 64 * 1024 * 1024,
  }).toString()
).target_directory;
const bin = path.join(targetDir, "release", "daml-fmt");

if (!fs.existsSync(bin)) {
  console.error("building release binary (cargo build --release)...");
  execFileSync("cargo", ["build", "--release", "--bin", "daml-fmt"], {
    cwd: repoRoot,
    stdio: "inherit",
  });
}

// Format `src` text through the Rust binary's stdin path.
const format = (src) =>
  execFileSync(bin, [], { input: src, cwd: repoRoot, maxBuffer: 64 * 1024 * 1024 }).toString();

const manifest = fs
  .readFileSync(path.join(repoRoot, "corpus", "desugar-ok.txt"), "utf8")
  .trim()
  .split("\n");

const mismatched = [];
const nonIdempotent = [];
const crashed = [];

for (const rel of manifest) {
  const src = fs.readFileSync(path.join(repoRoot, "original", rel), "utf8");
  const expected = fs.readFileSync(path.join(repoRoot, "expected", rel), "utf8");
  let once;
  try {
    once = format(src);
  } catch (err) {
    crashed.push(`${rel}: ${err.message}`);
    continue;
  }
  if (once !== expected) mismatched.push(rel);
  if (format(once) !== once) nonIdempotent.push(rel);
}

const report = (label, list) => {
  if (!list.length) return;
  console.error(`${label} (${list.length}):`);
  for (const item of list) console.error(`  ${item}`);
};
report("crashed", crashed);
report("output differs from expected/", mismatched);
report("non-idempotent", nonIdempotent);

const bad = new Set(
  [...crashed.map((c) => c.split(":")[0]), ...mismatched, ...nonIdempotent]
).size;
console.log(
  `${manifest.length} files: ${manifest.length - bad} ok, ` +
  `${crashed.length} crashed, ${mismatched.length} mismatched, ` +
  `${nonIdempotent.length} non-idempotent`
);
process.exit(bad ? 1 : 0);
