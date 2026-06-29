import { spawnSync } from "node:child_process";
import {
  cpSync,
  mkdtempSync,
  mkdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const damlLintRoot = join(here, "..");
const repoRoot = join(damlLintRoot, "..", "..");
const lintPluginDir = join(damlLintRoot, "lint-plugin");
const damlLintVersion = JSON.parse(
  readFileSync(join(damlLintRoot, "package.json"), "utf8"),
).version;

function run(command, options = {}) {
  const result = spawnSync(command, {
    stdio: "inherit",
    shell: true,
    ...options,
  });
  if (result.status !== 0) {
    process.exit(result.status ?? 1);
  }
  return result;
}

function packLocalDamlLint(workDir) {
  const packDir = join(workDir, "daml-lint-pack");
  mkdirSync(packDir);

  run("cargo build --release --locked -p daml-lint", { cwd: repoRoot });

  const wrapperDir = join(packDir, "wrapper");
  mkdirSync(wrapperDir);
  const binaryPath = join(repoRoot, "target", "release", "daml-lint");
  const launcherPath = join(wrapperDir, "daml-lint.mjs");
  writeFileSync(
    launcherPath,
    `#!/usr/bin/env node
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const binary = join(dirname(fileURLToPath(import.meta.url)), "daml-lint-bin");
const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
process.exit(result.status ?? 1);
`,
  );
  writeFileSync(
    join(wrapperDir, "package.json"),
    JSON.stringify(
      {
        name: "@daml-tools/daml-lint",
        version: damlLintVersion,
        private: true,
        bin: {
          "daml-lint": "./daml-lint.mjs",
        },
        files: ["daml-lint.mjs", "daml-lint-bin"],
      },
      null,
      2,
    ),
  );
  cpSync(binaryPath, join(wrapperDir, "daml-lint-bin"));
  run(`chmod +x "${join(wrapperDir, "daml-lint-bin")}" "${launcherPath}"`);

  const packResult = run(`npm pack "${wrapperDir}" --json --pack-destination "${packDir}"`, {
    stdio: ["ignore", "pipe", "inherit"],
    encoding: "utf8",
  });
  const [packInfo] = JSON.parse(packResult.stdout);
  return join(packDir, packInfo.filename);
}

function scaffoldAndVerify({
  work,
  packedLintPlugin,
  packedDamlLint,
  pluginArgs,
  projectDir,
  expectedPackageName,
}) {
  run(
    `npx --yes --package="${packedLintPlugin}" create-daml-lint-plugin ${pluginArgs.join(" ")}`,
    { cwd: work },
  );

  const packageJson = JSON.parse(readFileSync(join(projectDir, "package.json"), "utf8"));
  if (packageJson.name !== expectedPackageName) {
    console.error(
      `expected package name ${expectedPackageName}, got ${packageJson.name}`,
    );
    process.exit(1);
  }

  run(
    `npm install --save-dev "${packedLintPlugin}" "${packedDamlLint}" typescript@6.0.3 esbuild@0.28.1`,
    { cwd: projectDir },
  );
  run("npm run check", { cwd: projectDir });
  run("npm run build", { cwd: projectDir });
  run("npm run test:rules", { cwd: projectDir });
}

const work = mkdtempSync(join(tmpdir(), "daml-lint-plugin-scaffold-"));
const packDir = join(work, "pack");
const defaultProjectDir = join(work, "project-default");
const explicitProjectDir = join(work, "project-explicit");
mkdirSync(packDir);

try {
  const packResult = run(`npm pack "${lintPluginDir}" --json --pack-destination "${packDir}"`, {
    stdio: ["ignore", "pipe", "inherit"],
    encoding: "utf8",
  });
  const [packInfo] = JSON.parse(packResult.stdout);
  const packedLintPlugin = join(packDir, packInfo.filename);
  const packedDamlLint = packLocalDamlLint(work);

  // Match documented npx usage: npx -p @daml-tools/lint-plugin create-daml-lint-plugin ledger-style
  scaffoldAndVerify({
    work,
    packedLintPlugin,
    packedDamlLint,
    pluginArgs: ["ledger-style", `"${defaultProjectDir}"`],
    projectDir: defaultProjectDir,
    expectedPackageName: "daml-lint-plugin-ledger-style",
  });

  scaffoldAndVerify({
    work,
    packedLintPlugin,
    packedDamlLint,
    pluginArgs: [
      "daml-lint-plugin-ledger-style",
      `"${explicitProjectDir}"`,
    ],
    projectDir: explicitProjectDir,
    expectedPackageName: "daml-lint-plugin-ledger-style",
  });

  console.log("lint-plugin scaffold smoke test passed.");
} finally {
  rmSync(work, { recursive: true, force: true });
}
