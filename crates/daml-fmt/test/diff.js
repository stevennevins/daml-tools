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

// Always rebuild before testing — Cargo no-ops when sources are unchanged, but
// an existing or cache-restored binary must never silently shadow local edits
// (otherwise the differential can pass against a stale formatter).
console.error("building release binary (cargo build --release --bin daml-fmt)...");
execFileSync("cargo", ["build", "--release", "--bin", "daml-fmt"], {
  cwd: repoRoot,
  stdio: "inherit",
});

// Format `src` text through the Rust binary's stdin path.
const format = (src) =>
  execFileSync(bin, [], { input: src, cwd: repoRoot, maxBuffer: 64 * 1024 * 1024 }).toString();

const readManifest = (relPath) => {
  const abs = path.join(repoRoot, relPath);
  return fs
    .readFileSync(abs, "utf8")
    .split("\n")
    .map((line) => line.replace(/\r$/, ""))
    .filter((line) => line.trim().length > 0)
    .map((line) => line.trim());
};

const walkFiles = (rootDir, prefix = "") => {
  const out = [];
  for (const entry of fs.readdirSync(rootDir, { withFileTypes: true })) {
    const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
    const abs = path.join(rootDir, entry.name);
    if (entry.isDirectory()) out.push(...walkFiles(abs, rel));
    else out.push(rel);
  }
  return out;
};

const validateManifestIntegrity = () => {
  const manifestPath = path.join(repoRoot, "corpus", "desugar-ok.txt");
  const rawLines = fs
    .readFileSync(manifestPath, "utf8")
    .split("\n")
    .map((line) => line.replace(/\r$/, ""));

  const errors = [];
  for (let i = 0; i < rawLines.length; i += 1) {
    if (rawLines[i].trim().length === 0) {
      if (i === rawLines.length - 1 && rawLines[i] === "") continue;
      errors.push(`manifest line ${i + 1}: blank entry`);
    }
  }

  const manifest = readManifest("corpus/desugar-ok.txt");
  const seen = new Set();
  for (const rel of manifest) {
    if (seen.has(rel)) errors.push(`manifest duplicate: ${rel}`);
    seen.add(rel);
  }

  for (const rel of manifest) {
    if (!fs.existsSync(path.join(repoRoot, "original", rel))) {
      errors.push(`missing original: ${rel}`);
    }
    if (!fs.existsSync(path.join(repoRoot, "expected", rel))) {
      errors.push(`missing expected: ${rel}`);
    }
  }

  const manifestSet = new Set(manifest);
  for (const rel of walkFiles(path.join(repoRoot, "expected"))) {
    if (!manifestSet.has(rel)) errors.push(`expected without manifest entry: ${rel}`);
  }

  const documentedOutsideManifest = new Set([
    ...readManifest("corpus/excluded-error-annotated.txt"),
    ...readManifest("corpus/desugar-fail.txt"),
  ]);
  for (const rel of walkFiles(path.join(repoRoot, "original"))) {
    if (!manifestSet.has(rel) && !documentedOutsideManifest.has(rel)) {
      errors.push(`original without manifest entry: ${rel}`);
    }
  }

  if (errors.length) {
    console.error(`manifest integrity (${errors.length}):`);
    for (const item of errors) console.error(`  ${item}`);
    process.exit(1);
  }
  return manifest;
};

const manifest = validateManifestIntegrity();

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
