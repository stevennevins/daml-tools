import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
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

let registryPackage;

try {
  registryPackage = JSON.parse(
    execFileSync("npm", ["view", packageSpec, "--json", "--prefer-online"], {
      encoding: "utf8",
      stdio: ["ignore", "pipe", "pipe"],
    }),
  );
} catch (error) {
  const stderr = error.stderr?.toString().trim();
  if (stderr) {
    console.error(stderr);
  }
  console.error(`${packageSpec} is not visible in the npm registry.`);
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

if (registryPackage.version !== localPackage.version) {
  errors.push(`${packageSpec} registry version is ${registryPackage.version}.`);
}

if (!registryPackage.dist?.integrity) {
  errors.push(`${packageSpec} registry metadata is missing dist.integrity.`);
}

for (const field of comparableFields) {
  compareField(field);
}

if (errors.length > 0) {
  console.error(errors.join("\n"));
  process.exit(1);
}

console.log(`${packageSpec} registry metadata matches ${packageJsonPath}.`);
