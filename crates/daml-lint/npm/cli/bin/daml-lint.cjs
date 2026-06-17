#!/usr/bin/env node
"use strict";

const { existsSync } = require("node:fs");
const path = require("node:path");
const { spawnSync } = require("node:child_process");

const platformPackages = {
  "darwin:arm64": {
    name: "@daml-tools/daml-lint-darwin-arm64",
    binary: ["bin", "daml-lint"],
  },
  "linux:x64": {
    name: "@daml-tools/daml-lint-linux-x64",
    binary: ["bin", "daml-lint"],
  },
  "win32:x64": {
    name: "@daml-tools/daml-lint-win32-x64",
    binary: ["bin", "daml-lint.exe"],
  },
};

const platformKey = `${process.platform}:${process.arch}`;
const platformPackage = platformPackages[platformKey];

if (!platformPackage) {
  console.error(
    `daml-lint is not distributed for ${process.platform}/${process.arch}. ` +
      "Supported npm platforms are linux/x64, darwin/arm64, and win32/x64.",
  );
  process.exit(1);
}

let binaryPath;

try {
  const packageJsonPath = require.resolve(`${platformPackage.name}/package.json`);
  binaryPath = path.join(path.dirname(packageJsonPath), ...platformPackage.binary);
} catch {
  console.error(
    `The native package ${platformPackage.name} is not installed. ` +
      "Reinstall @daml-tools/daml-lint with optional dependencies enabled.",
  );
  process.exit(1);
}

if (!existsSync(binaryPath)) {
  console.error(`The native daml-lint binary is missing from ${platformPackage.name}.`);
  process.exit(1);
}

const result = spawnSync(binaryPath, process.argv.slice(2), {
  stdio: "inherit",
  windowsHide: false,
});

if (result.error) {
  console.error(`Failed to start daml-lint: ${result.error.message}`);
  process.exit(1);
}

if (result.signal) {
  if (process.platform !== "win32") {
    process.kill(process.pid, result.signal);
  }
  process.exit(1);
}

process.exit(result.status ?? 1);
