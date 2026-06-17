import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";

const checkOnly = process.argv.includes("--check");
const unexpectedArgs = process.argv.slice(2).filter((arg) => arg !== "--check");

if (unexpectedArgs.length > 0) {
  console.error(`Unexpected argument: ${unexpectedArgs.join(" ")}`);
  process.exit(1);
}

const metadata = JSON.parse(
  execFileSync("cargo", ["metadata", "--format-version=1", "--no-deps"], {
    encoding: "utf8",
  }),
);
const damlLintPackage = metadata.packages.find((pkg) => pkg.name === "daml-lint");

if (!damlLintPackage) {
  console.error("Could not find the daml-lint package in cargo metadata.");
  process.exit(1);
}

const damlLintVersion = damlLintPackage.version;
const lintPluginDependency = `^${damlLintVersion}`;
const syncTargets = [
  {
    path: "package.json",
    update(packageJson) {
      packageJson.version = damlLintVersion;
    },
    validate(packageJson) {
      if (packageJson.version !== damlLintVersion) {
        return `daml-lint-rules version ${packageJson.version} does not match daml-lint ${damlLintVersion}.`;
      }
      return null;
    },
  },
  {
    path: "package-lock.json",
    update(packageLock) {
      packageLock.version = damlLintVersion;
      packageLock.packages ??= {};
      packageLock.packages[""] ??= {};
      packageLock.packages[""].version = damlLintVersion;
    },
    validate(packageLock) {
      const errors = [];

      if (packageLock.version !== damlLintVersion) {
        errors.push(
          `package-lock.json version ${packageLock.version} does not match daml-lint ${damlLintVersion}.`,
        );
      }

      const rootPackageVersion = packageLock.packages?.[""]?.version;

      if (rootPackageVersion !== damlLintVersion) {
        errors.push(
          `package-lock.json root package version ${rootPackageVersion} does not match daml-lint ${damlLintVersion}.`,
        );
      }

      return errors.length > 0 ? errors.join("\n") : null;
    },
  },
  {
    path: "lint-plugin/package.json",
    update(packageJson) {
      packageJson.version = damlLintVersion;
    },
    validate(packageJson) {
      if (packageJson.version !== damlLintVersion) {
        return `@daml-tools/lint-plugin version ${packageJson.version} does not match daml-lint ${damlLintVersion}.`;
      }
      return null;
    },
  },
  {
    path: "lint-plugin/templates/project/package.json",
    update(packageJson) {
      packageJson.devDependencies ??= {};
      packageJson.devDependencies["@daml-tools/lint-plugin"] = lintPluginDependency;
    },
    validate(packageJson) {
      const actual = packageJson.devDependencies?.["@daml-tools/lint-plugin"];

      if (actual !== lintPluginDependency) {
        return `Template @daml-tools/lint-plugin dependency ${actual} does not match ${lintPluginDependency}.`;
      }
      return null;
    },
  },
];

const errors = [];
let updated = false;

for (const target of syncTargets) {
  const original = readFileSync(target.path, "utf8");
  const packageJson = JSON.parse(original);
  const validationError = target.validate(packageJson);

  if (validationError) {
    errors.push(validationError);
  }

  target.update(packageJson);
  const next = `${JSON.stringify(packageJson, null, 2)}\n`;

  if (original !== next) {
    if (!checkOnly) {
      writeFileSync(target.path, next);
      console.log(`Updated ${target.path}`);
    }
    updated = true;
  }
}

if (checkOnly && errors.length > 0) {
  console.error(
    [
      ...errors,
      "Run npm run build:lint-plugin-version before releasing a new daml-lint tag.",
    ].join("\n"),
  );
  process.exit(1);
}

if (!updated) {
  console.log(`@daml-tools/lint-plugin metadata already matches daml-lint ${damlLintVersion}.`);
}
