import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";

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

const lintPluginPackage = JSON.parse(
  readFileSync("lint-plugin/package.json", "utf8"),
);

if (lintPluginPackage.version !== damlLintPackage.version) {
  console.error(
    [
      `@daml-tools/lint-plugin version ${lintPluginPackage.version} does not match daml-lint ${damlLintPackage.version}.`,
      "Update lint-plugin/package.json before releasing a new daml-lint tag.",
    ].join("\n"),
  );
  process.exit(1);
}
