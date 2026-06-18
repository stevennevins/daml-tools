import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const packageDir = process.argv[2];

if (!packageDir) {
  console.error("Usage: node tools/verify-npm-cli-registry-package.mjs <package-dir>");
  process.exit(1);
}

const packageJsonPath = path.join(packageDir, "package.json");
const localPackage = JSON.parse(readFileSync(packageJsonPath, "utf8"));
const packageSpec = `${localPackage.name}@${localPackage.version}`;
const comparableFields = ["os", "cpu", "libc", "bin", "optionalDependencies"];
const platformPackagePattern = /^@daml-tools\/(daml-lint|daml-fmt)-(darwin-arm64|linux-arm64|linux-x64|win32-x64)$/;
const platformBinarySuffixes = {
  "darwin-arm64": "",
  "linux-arm64": "",
  "linux-x64": "",
  "win32-x64": ".exe",
};

let registryPackage;

function execNpm(args) {
  return execFileSync("npm", args, {
    encoding: "utf8",
    shell: process.platform === "win32",
    stdio: ["ignore", "pipe", "pipe"],
  });
}

try {
  const registryFields = ["version", "dist.integrity", ...comparableFields];
  const output = execNpm([
    "view",
    packageSpec,
    ...registryFields,
    "--json",
    "--prefer-online",
  ]);
  const registryFieldsPackage = JSON.parse(output);
  registryPackage = {
    ...registryFieldsPackage,
    dist: {
      integrity: registryFieldsPackage["dist.integrity"],
    },
  };
} catch (error) {
  const stderr = error.stderr?.toString().trim();
  if (stderr) {
    console.error(stderr);
  }
  console.error(`${packageSpec} metadata could not be read from the npm registry.`);
  console.error(error.message);
  process.exit(1);
}

const errors = [];

function normalize(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return value;
  }

  return Object.fromEntries(
    Object.entries(value)
      .sort(([left], [right]) => left.localeCompare(right))
      .map(([key, nestedValue]) => [key, normalize(nestedValue)]),
  );
}

function compareField(field) {
  const expected = normalize(localPackage[field]);
  const actual = normalize(registryPackage[field]);

  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    errors.push(
      `${packageSpec} registry ${field} is ${JSON.stringify(actual)}, expected ${JSON.stringify(expected)}.`,
    );
  }
}

function packRegistryPackage() {
  const tempRoot = mkdtempSync(path.join(tmpdir(), "daml-tools-npm-registry-"));

  try {
    const output = execNpm([
      "pack",
      packageSpec,
      "--json",
      "--pack-destination",
      tempRoot,
      "--ignore-scripts",
    ]);
    const [pack] = JSON.parse(output);
    return pack;
  } catch (error) {
    const stderr = error.stderr?.toString().trim();
    if (stderr) {
      console.error(stderr);
    }
    console.error(`${packageSpec} could not be packed from the npm registry.`);
    process.exit(1);
  } finally {
    rmSync(tempRoot, { recursive: true, force: true });
  }
}

function expectedRegistryFiles() {
  const files = [{ path: "package.json", executable: false }];

  if (localPackage.bin) {
    return files.concat(
      Object.values(localPackage.bin).map((binPath) => ({
        path: binPath,
        executable: true,
      })),
    );
  }

  const match = localPackage.name.match(platformPackagePattern);
  if (!match) {
    errors.push(`${packageSpec} is neither a CLI wrapper nor a known platform package.`);
    return files;
  }

  const [, tool, platform] = match;
  return files.concat({
    path: `bin/${tool}${platformBinarySuffixes[platform]}`,
    executable: platform !== "win32-x64",
  });
}

if (registryPackage.version !== localPackage.version) {
  errors.push(`${packageSpec} registry version is ${registryPackage.version}.`);
}

if (!registryPackage.dist?.integrity) {
  errors.push(`${packageSpec} registry metadata is missing dist.integrity.`);
}

for (const field of comparableFields) {
  compareField(field);
}

const registryPack = packRegistryPackage();
const registryFiles = new Map(registryPack.files.map((file) => [file.path, file]));

if (registryPackage.dist?.integrity && registryPack.integrity !== registryPackage.dist.integrity) {
  errors.push(
    `${packageSpec} packed tarball integrity is ${registryPack.integrity}, expected ${registryPackage.dist.integrity}.`,
  );
}

for (const expectedFile of expectedRegistryFiles()) {
  const registryFile = registryFiles.get(expectedFile.path);

  if (!registryFile) {
    errors.push(`${packageSpec} registry tarball is missing ${expectedFile.path}.`);
  } else if (expectedFile.executable && (registryFile.mode & 0o111) === 0) {
    errors.push(`${packageSpec} registry tarball ${expectedFile.path} is not executable.`);
  }
}

if (errors.length > 0) {
  console.error(errors.join("\n"));
  process.exit(1);
}

console.log(`${packageSpec} registry metadata and tarball contents match ${packageJsonPath}.`);
