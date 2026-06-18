import { execFileSync } from "node:child_process";
import { accessSync, constants, existsSync, readFileSync } from "node:fs";
import path from "node:path";

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
    wrapperBin: "bin/daml-lint.cjs",
  },
  "daml-fmt": {
    crate: "daml-fmt",
    packagePrefix: "@daml-tools/daml-fmt",
    root: "crates/daml-fmt/npm",
    wrapperBin: "bin/daml-fmt.cjs",
  },
};

const errors = [];

function readJson(jsonPath) {
  return JSON.parse(readFileSync(jsonPath, "utf8"));
}

function expect(condition, message) {
  if (!condition) {
    errors.push(message);
  }
}

function crateVersion(crateName) {
  const crate = metadata.packages.find((pkg) => pkg.name === crateName);
  expect(crate, `Could not find ${crateName} in cargo metadata.`);
  return crate?.version;
}

function assertJsonArray(actual, expected, label) {
  expect(
    Array.isArray(actual) &&
      actual.length === expected.length &&
      actual.every((value, index) => value === expected[index]),
    `${label} must be ${JSON.stringify(expected)}, got ${JSON.stringify(actual)}.`,
  );
}

function checkNpmPack(packageDir, expectedFiles) {
  const output = execFileSync("npm", ["pack", "--dry-run", "--json"], {
    cwd: packageDir,
    encoding: "utf8",
  });
  const [pack] = JSON.parse(output);
  const files = new Set(pack.files.map((file) => file.path));

  for (const expectedFile of expectedFiles) {
    expect(files.has(expectedFile), `${packageDir} npm pack is missing ${expectedFile}.`);
  }
}

for (const [tool, config] of Object.entries(cliPackages)) {
  const version = crateVersion(config.crate);
  const wrapperPath = `${config.root}/cli/package.json`;
  const wrapper = readJson(wrapperPath);
  const wrapperBinPath = path.join(config.root, "cli", config.wrapperBin);
  const expectedOptionalDependencies = Object.fromEntries(
    Object.keys(platformPackages)
      .sort()
      .map((platform) => [`${config.packagePrefix}-${platform}`, version]),
  );

  expect(wrapper.name === config.packagePrefix, `${wrapperPath} name must be ${config.packagePrefix}.`);
  expect(wrapper.version === version, `${wrapperPath} version must be ${version}.`);
  expect(wrapper.bin?.[tool] === `./${config.wrapperBin}`, `${wrapperPath} bin.${tool} must point to ./${config.wrapperBin}.`);
  expect(
    JSON.stringify(wrapper.optionalDependencies) === JSON.stringify(expectedOptionalDependencies),
    `${wrapperPath} optionalDependencies must exactly match current platform packages at ${version}.`,
  );
  expect(wrapper.publishConfig?.access === "public", `${wrapperPath} must publish publicly.`);
  expect(existsSync(wrapperBinPath), `${wrapperBinPath} must exist.`);

  try {
    accessSync(wrapperBinPath, constants.X_OK);
  } catch {
    errors.push(`${wrapperBinPath} must be executable.`);
  }

  checkNpmPack(path.join(config.root, "cli"), [
    "package.json",
    "README.md",
    config.wrapperBin,
  ]);

  for (const [platform, platformConfig] of Object.entries(platformPackages)) {
    const packagePath = `${config.root}/${platform}/package.json`;
    const packageJson = readJson(packagePath);
    const expectedName = `${config.packagePrefix}-${platform}`;

    expect(packageJson.name === expectedName, `${packagePath} name must be ${expectedName}.`);
    expect(packageJson.version === version, `${packagePath} version must be ${version}.`);
    assertJsonArray(packageJson.os, platformConfig.os, `${packagePath} os`);
    assertJsonArray(packageJson.cpu, platformConfig.cpu, `${packagePath} cpu`);
    assertJsonArray(packageJson.files, ["bin"], `${packagePath} files`);
    expect(packageJson.publishConfig?.access === "public", `${packagePath} must publish publicly.`);
  }
}

if (errors.length > 0) {
  console.error(errors.join("\n"));
  process.exit(1);
}

console.log("npm CLI package metadata is valid.");
