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
const manifestPath = path.join(repoRoot, "corpus", "desugar-ok.txt");
const originalDir = path.join(repoRoot, "original");
const expectedDir = path.join(repoRoot, "expected");

const normalizeManifestPath = (rel) => {
  const normalized = path.normalize(rel);
  if (
    path.isAbsolute(normalized) ||
    normalized.startsWith("..") ||
    normalized.includes(`..${path.sep}`)
  ) {
    return null;
  }
  return normalized.split(path.sep).join("/");
};

const listDamlFiles = (rootDir) => {
  const out = [];
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      const full = path.join(dir, entry.name);
      if (entry.isDirectory()) walk(full);
      else if (entry.isFile() && entry.name.endsWith(".daml")) {
        out.push(path.relative(rootDir, full).split(path.sep).join("/"));
      }
    }
  };
  walk(rootDir);
  return out;
};

const failIntegrity = (errors) => {
  console.error(`manifest integrity (${errors.length}):`);
  for (const item of errors) console.error(`  ${item}`);
  process.exit(1);
};

const checkManifestIntegrity = () => {
  const errors = [];
  if (!fs.existsSync(manifestPath)) {
    errors.push("missing manifest: corpus/desugar-ok.txt");
  }
  if (!fs.existsSync(originalDir)) {
    errors.push("missing directory: original/");
  }
  if (!fs.existsSync(expectedDir)) {
    errors.push("missing directory: expected/");
  }
  if (errors.length) failIntegrity(errors);

  const manifest = [];
  const manifestSet = new Set();
  const lines = fs.readFileSync(manifestPath, "utf8").split(/\r?\n/);
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    if (!line) continue;
    const rel = normalizeManifestPath(line);
    if (!rel) {
      errors.push(`manifest line ${i + 1}: unsafe path: ${line}`);
      continue;
    }
    if (manifestSet.has(rel)) {
      errors.push(`manifest duplicate: ${rel}`);
      continue;
    }
    manifestSet.add(rel);
    manifest.push(rel);
    if (!fs.existsSync(path.join(originalDir, rel))) {
      errors.push(`missing original: ${rel}`);
    }
    if (!fs.existsSync(path.join(expectedDir, rel))) {
      errors.push(`missing expected: ${rel}`);
    }
  }
  if (!manifest.length) {
    errors.push("manifest is empty: corpus/desugar-ok.txt");
  }

  for (const rel of listDamlFiles(expectedDir)) {
    if (!manifestSet.has(rel)) errors.push(`stale expected: ${rel}`);
  }

  if (errors.length) failIntegrity(errors);
  return manifest;
};

const manifest = checkManifestIntegrity();
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
