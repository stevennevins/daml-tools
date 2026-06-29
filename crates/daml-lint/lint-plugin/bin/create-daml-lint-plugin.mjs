#!/usr/bin/env node
import {
  cpSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const PACKAGE_PREFIX = "daml-lint-plugin-";
const PLUGIN_NAME_PATTERN = /^[a-z][a-z0-9]*(?:-[a-z0-9]+)*$/;
const TEXT_EXTENSIONS = new Set([
  ".daml",
  ".json",
  ".md",
  ".mjs",
  ".ts",
  ".yaml",
  ".yml",
]);

const templateRoot = join(dirname(fileURLToPath(import.meta.url)), "..", "templates", "project");

function printHelp() {
  console.log(`Usage: create-daml-lint-plugin <plugin-name> [target-dir]

Create a multi-rule daml-lint plugin package.

Arguments:
  plugin-name   Short plugin namespace (for example, ledger-style).
  target-dir    Output directory. Defaults to daml-lint-plugin-<plugin-name>.

Examples:
  create-daml-lint-plugin ledger-style
  create-daml-lint-plugin ledger-style ./packages/daml-lint-plugin-ledger-style
  create-daml-lint-plugin acme daml-lint-plugin-acme
`);
}

function fail(message) {
  console.error(`error: ${message}`);
  process.exit(1);
}

function parseArgs(argv) {
  if (argv.length === 0 || argv.includes("-h") || argv.includes("--help")) {
    printHelp();
    process.exit(argv.length === 0 ? 1 : 0);
  }

  // Be forgiving if a caller accidentally repeats the bin name as the first argument.
  if (argv[0] === "create-daml-lint-plugin") {
    argv = argv.slice(1);
  }

  if (argv.length === 0) {
    printHelp();
    process.exit(1);
  }

  const pluginArg = argv[0];
  const targetArg = argv[1];

  if (argv.length > 2) {
    fail(`unexpected arguments: ${argv.slice(2).join(" ")}`);
  }

  let pluginName = pluginArg;
  if (pluginName.startsWith(PACKAGE_PREFIX)) {
    pluginName = pluginName.slice(PACKAGE_PREFIX.length);
  }

  let targetDir;
  if (targetArg) {
    targetDir = resolve(targetArg);
  } else if (pluginArg.startsWith(PACKAGE_PREFIX)) {
    targetDir = resolve(pluginArg);
  } else {
    targetDir = resolve(`${PACKAGE_PREFIX}${pluginName}`);
  }

  if (!PLUGIN_NAME_PATTERN.test(pluginName)) {
    fail(
      `plugin name must match ${PLUGIN_NAME_PATTERN}; got ${JSON.stringify(pluginName)}`,
    );
  }

  const packageName = `${PACKAGE_PREFIX}${pluginName}`;

  return {
    pluginName,
    packageName,
    targetDir,
  };
}

function substitute(content, replacements) {
  let next = content;
  for (const [token, value] of Object.entries(replacements)) {
    next = next.replaceAll(token, value);
  }
  return next;
}

function copyTemplate(sourceDir, targetDir, replacements) {
  for (const entry of readdirSync(sourceDir, { withFileTypes: true })) {
    const sourcePath = join(sourceDir, entry.name);
    const targetPath = join(targetDir, entry.name);

    if (entry.isDirectory()) {
      mkdirSync(targetPath, { recursive: true });
      copyTemplate(sourcePath, targetPath, replacements);
      continue;
    }

    if (!entry.isFile()) {
      continue;
    }

    const extension = entry.name.slice(entry.name.lastIndexOf("."));
    if (TEXT_EXTENSIONS.has(extension)) {
      const content = readFileSync(sourcePath, "utf8");
      writeFileSync(targetPath, substitute(content, replacements), "utf8");
      continue;
    }

    cpSync(sourcePath, targetPath);
  }
}

function assertTemplateExists() {
  if (!existsSync(templateRoot) || !statSync(templateRoot).isDirectory()) {
    fail(`starter template is missing at ${templateRoot}`);
  }
}

const { pluginName, packageName, targetDir } = parseArgs(process.argv.slice(2));

assertTemplateExists();

if (existsSync(targetDir)) {
  const entries = readdirSync(targetDir);
  if (entries.length > 0) {
    fail(`target directory already exists and is not empty: ${targetDir}`);
  }
}

mkdirSync(targetDir, { recursive: true });
copyTemplate(templateRoot, targetDir, {
  "__PLUGIN_NAME__": pluginName,
  "__PACKAGE_NAME__": packageName,
});

console.log(`Created ${packageName} at ${targetDir}`);
console.log("");
console.log("Next steps:");
console.log(`  cd ${targetDir}`);
console.log("  npm install");
console.log("  npm run check");
console.log("  npm run build");
console.log("  npm run test:rules");
