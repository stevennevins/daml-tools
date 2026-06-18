import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const allTools = ["daml-lint", "daml-fmt"];
const args = process.argv.slice(2);
const checkOnly = args.includes("--check");
const requestedTools = args.filter((arg) => arg !== "--check");

for (const tool of requestedTools) {
  if (!allTools.includes(tool)) {
    console.error(`Unexpected argument: ${tool}`);
    process.exit(1);
  }
}

const selectedTools = requestedTools.length > 0 ? requestedTools : allTools;
const metadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version=1", "--no-deps"], {
    encoding: "utf8",
  }),
);

const platformPackages = {
  "darwin-arm64": {
    os: ["darwin"],
    cpu: ["arm64"],
  },
  "linux-x64": {
    os: ["linux"],
    cpu: ["x64"],
    libc: ["glibc"],
  },
  "win32-x64": {
    os: ["win32"],
    cpu: ["x64"],
  },
};

const cliPackages = {
  "daml-lint": {
    crate: "daml-lint",
    packagePrefix: "@daml-tools/daml-lint",
    root: "crates/daml-lint/npm",
    extraManifests: [],
  },
  "daml-fmt": {
    crate: "daml-fmt",
    packagePrefix: "@daml-tools/daml-fmt",
    root: "crates/daml-fmt/npm",
    extraManifests: ["crates/daml-fmt/package.json"],
  },
};

function readJson(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function writeJson(path, value) {
  writeFileSync(path, `${JSON.stringify(value, null, 2)}\n`);
}

function normalizeLineEndings(value) {
  return value.replace(/\r\n/g, "\n");
}

function crateVersion(crateName) {
  const crate = metadata.packages.find((pkg) => pkg.name === crateName);

  if (!crate) {
    console.error(`Could not find the ${crateName} package in cargo metadata.`);
    process.exit(1);
  }

  return crate.version;
}

function syncJson(path, update) {
  const original = readFileSync(path, "utf8");
  const json = JSON.parse(original);
  update(json);
  const next = `${JSON.stringify(json, null, 2)}\n`;

  if (normalizeLineEndings(original) === next) {
    return false;
  }

  if (!checkOnly) {
    writeJson(path, json);
    console.log(`Updated ${path}`);
  }

  return true;
}

let changed = false;

for (const tool of selectedTools) {
  const config = cliPackages[tool];
  const version = crateVersion(config.crate);
  const optionalDependencies = Object.fromEntries(
    Object.keys(platformPackages)
      .sort()
      .map((platform) => [`${config.packagePrefix}-${platform}`, version]),
  );

  changed =
    syncJson(`${config.root}/cli/package.json`, (packageJson) => {
      packageJson.version = version;
      packageJson.optionalDependencies = optionalDependencies;
    }) || changed;

  for (const platform of Object.keys(platformPackages)) {
    const platformConfig = platformPackages[platform];
    changed =
      syncJson(`${config.root}/${platform}/package.json`, (packageJson) => {
        packageJson.version = version;
        if (platformConfig.libc) {
          packageJson.libc = platformConfig.libc;
        } else {
          delete packageJson.libc;
        }
      }) || changed;
  }

  for (const manifestPath of config.extraManifests) {
    changed =
      syncJson(manifestPath, (packageJson) => {
        packageJson.version = version;
      }) || changed;
  }
}

if (checkOnly && changed) {
  console.error("npm CLI package metadata is out of sync with Cargo package versions.");
  process.exit(1);
}

if (!changed) {
  console.log("npm CLI package metadata already matches Cargo package versions.");
}
