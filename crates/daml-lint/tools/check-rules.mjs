import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative } from "node:path";

const checkedPaths = [
  "rules",
  "examples",
  "lint-plugin",
  "tools",
  "package.json",
  "package-lock.json",
  "tsconfig.json",
];

function filesUnder(path) {
  if (!existsSync(path)) return [];
  const stat = statSync(path);
  if (stat.isFile()) return [path];
  if (!stat.isDirectory()) return [];
  return readdirSync(path, { withFileTypes: true })
    .filter((entry) => entry.name !== "node_modules")
    .flatMap((entry) => filesUnder(join(path, entry.name)));
}

function snapshot(paths) {
  const entries = new Map();
  for (const path of paths) {
    for (const file of filesUnder(path)) {
      const hash = createHash("sha256").update(readFileSync(file)).digest("hex");
      entries.set(relative(process.cwd(), file), hash);
    }
  }
  return entries;
}

function changedFiles(before, after) {
  const changed = new Set();
  for (const [file, hash] of before) {
    if (after.get(file) !== hash) changed.add(file);
  }
  for (const file of after.keys()) {
    if (!before.has(file)) changed.add(file);
  }
  return [...changed].sort();
}

function run(command) {
  const result = spawnSync(command, { stdio: "inherit", shell: true });
  if (result.status !== 0) process.exit(result.status ?? 1);
}

const before = snapshot(checkedPaths);

run("npm run check:types");
run("npm run build:rules");
run("npm run build:examples");
run("npm run check:examples-clean");
run("npm run check:lint-plugin-package");

const after = snapshot(checkedPaths);
const changed = changedFiles(before, after);
if (changed.length > 0) {
  console.error("Generated lint-rule artifacts changed during npm run check:rules:");
  for (const file of changed) console.error(`  ${file}`);
  console.error("Re-run npm run check:rules and keep the generated outputs.");
  process.exit(1);
}
