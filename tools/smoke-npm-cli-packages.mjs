import { execFileSync } from "node:child_process";
import {
  chmodSync,
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";

const platformPackages = {
  "darwin-arm64": {
    packagePlatform: "darwin:arm64",
    binaryExtension: "",
  },
  "linux-x64": {
    packagePlatform: "linux:x64",
    binaryExtension: "",
  },
  "win32-x64": {
    packagePlatform: "win32:x64",
    binaryExtension: ".exe",
  },
};

const cliPackages = {
  "daml-lint": {
    root: "crates/daml-lint/npm",
    expectedVersion: /^daml-lint \d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/,
  },
  "daml-fmt": {
    root: "crates/daml-fmt/npm",
    expectedVersion: /^\d+\.\d+\.\d+(-[0-9A-Za-z.-]+)?$/,
  },
};

const hostPlatform = Object.entries(platformPackages).find(
  ([, config]) => config.packagePlatform === `${process.platform}:${process.arch}`,
)?.[0];
const smokePlatform =
  hostPlatform ?? (process.platform === "linux" ? "linux-x64" : undefined);
const platformOverride = hostPlatform
  ? undefined
  : smokePlatform
    ? { platform: "linux", arch: "x64" }
    : undefined;

if (!smokePlatform) {
  console.error(
    `No npm smoke-test platform is defined for ${process.platform}/${process.arch}.`,
  );
  process.exit(1);
}

const tempRoot = mkdtempSync(path.join(tmpdir(), "daml-tools-npm-smoke-"));
const packageRoot = path.join(tempRoot, "packages");
const tarballRoot = path.join(tempRoot, "tarballs");
const consumerRoot = path.join(tempRoot, "consumer");
const keepTemp = process.env.KEEP_NPM_SMOKE_DIR === "1";

function run(command, args, options = {}) {
  const { capture = false, ...execOptions } = options;

  return execFileSync(command, args, {
    stdio: capture ? ["ignore", "pipe", "inherit"] : "inherit",
    encoding: capture ? "utf8" : undefined,
    ...execOptions,
  });
}

function npmPack(packageDir) {
  const output = run("npm", ["pack", "--json", "--pack-destination", tarballRoot], {
    cwd: packageDir,
    capture: true,
  });
  const [pack] = JSON.parse(output);
  return path.join(tarballRoot, pack.filename);
}

function copyPackage(sourceDir, destinationDir) {
  cpSync(sourceDir, destinationDir, {
    recursive: true,
    filter(source) {
      return !source.includes(`${path.sep}node_modules${path.sep}`);
    },
  });
}

function writeBinary(packageDir, tool, platform) {
  const platformConfig = platformPackages[platform];
  const binDir = path.join(packageDir, "bin");
  const binaryName = `${tool}${platformConfig.binaryExtension}`;
  const binaryPath = path.join(binDir, binaryName);

  mkdirSync(binDir, { recursive: true });

  if (platform === smokePlatform) {
    const builtBinary = path.join("target", "release", binaryName);

    if (!existsSync(builtBinary)) {
      throw new Error(`Expected ${builtBinary} after cargo build.`);
    }

    cpSync(builtBinary, binaryPath);
  } else {
    writeFileSync(binaryPath, "placeholder for non-host npm smoke tests\n");
  }

  chmodSync(binaryPath, 0o755);
}

function runtimeEnv() {
  if (!platformOverride) {
    return process.env;
  }

  const platformShim = path.join(tempRoot, "platform-shim.cjs");
  writeFileSync(
    platformShim,
    [
      `Object.defineProperty(process, "platform", { value: ${JSON.stringify(platformOverride.platform)} });`,
      `Object.defineProperty(process, "arch", { value: ${JSON.stringify(platformOverride.arch)} });`,
      "",
    ].join("\n"),
  );

  return {
    ...process.env,
    NODE_OPTIONS: [process.env.NODE_OPTIONS, `--require=${platformShim}`]
      .filter(Boolean)
      .join(" "),
  };
}

function binPath(tool) {
  const executable = process.platform === "win32" ? `${tool}.cmd` : tool;
  return path.join(consumerRoot, "node_modules", ".bin", executable);
}

try {
  mkdirSync(packageRoot, { recursive: true });
  mkdirSync(tarballRoot, { recursive: true });
  mkdirSync(consumerRoot, { recursive: true });

  run("cargo", [
    "build",
    "--release",
    "--locked",
    "--bin",
    "daml-lint",
    "--bin",
    "daml-fmt",
  ]);

  const devDependencies = {};
  const optionalDependencies = {};

  for (const [tool, config] of Object.entries(cliPackages)) {
    const wrapperDir = path.join(packageRoot, tool, "cli");
    copyPackage(path.join(config.root, "cli"), wrapperDir);
    devDependencies[`@daml-tools/${tool}`] = `file:${npmPack(wrapperDir)}`;

    for (const platform of Object.keys(platformPackages)) {
      const platformDir = path.join(packageRoot, tool, platform);
      copyPackage(path.join(config.root, platform), platformDir);
      writeBinary(platformDir, tool, platform);
      optionalDependencies[`@daml-tools/${tool}-${platform}`] = `file:${npmPack(platformDir)}`;
    }
  }

  writeFileSync(
    path.join(consumerRoot, "package.json"),
    `${JSON.stringify(
      {
        private: true,
        devDependencies,
        optionalDependencies,
      },
      null,
      2,
    )}\n`,
  );

  const installArgs = ["install", "--ignore-scripts", "--no-audit", "--no-fund"];

  if (platformOverride) {
    installArgs.push(`--os=${platformOverride.platform}`, `--cpu=${platformOverride.arch}`);
  }

  run("npm", installArgs, {
    cwd: consumerRoot,
  });

  const env = runtimeEnv();

  for (const [tool, config] of Object.entries(cliPackages)) {
    const output = run(binPath(tool), ["--version"], {
      cwd: consumerRoot,
      capture: true,
      env,
    }).trim();

    if (!config.expectedVersion.test(output)) {
      throw new Error(`${tool} --version returned ${JSON.stringify(output)}.`);
    }

    console.log(`${tool} npm smoke test passed: ${output}`);
  }
} finally {
  if (keepTemp) {
    console.log(`Kept npm smoke-test directory: ${tempRoot}`);
  } else {
    rmSync(tempRoot, { recursive: true, force: true });
  }
}
