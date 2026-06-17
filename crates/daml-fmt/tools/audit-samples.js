#!/usr/bin/env node
// Generate a reviewable daml-fmt audit over the 924-file corpus.
//
// The audit intentionally exercises the stdin formatter path:
//   original file bytes -> daml-fmt stdin -> formatted audit artifact
//
// Mechanical checks stay automated. Human/subagent review is limited to the
// judgment call: whether each source->formatted diff is accurate and consistent
// with the formatter's documented layout rules.
const crypto = require("crypto");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");

const repoRoot = path.resolve(__dirname, "..");
const maxBuffer = 256 * 1024 * 1024;

function usage(code = 0) {
  const stream = code === 0 ? process.stdout : process.stderr;
  stream.write(`usage: node tools/audit-samples.js [options]

Options:
  --out DIR          audit output directory (default: target/daml-fmt-audit)
  --batch-size N     samples per review batch (default: 25)
  --batch N          only audit one 1-based batch
  --no-desugar       skip daml damlc desugar byte comparison
  -h, --help         show this help

Environment:
  FMT=/path/to/daml-fmt  use a prebuilt formatter binary
`);
  process.exit(code);
}

function parseArgs(argv) {
  const opts = {
    outDir: path.join(repoRoot, "target", "daml-fmt-audit"),
    batchSize: 25,
    batch: null,
    desugar: true,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const arg = argv[i];
    if (arg === "-h" || arg === "--help") usage(0);
    if (arg === "--no-desugar") {
      opts.desugar = false;
    } else if (arg === "--out") {
      opts.outDir = path.resolve(repoRoot, needValue(argv, ++i, arg));
    } else if (arg === "--batch-size") {
      opts.batchSize = parsePositiveInt(needValue(argv, ++i, arg), arg);
    } else if (arg === "--batch") {
      opts.batch = parsePositiveInt(needValue(argv, ++i, arg), arg);
    } else {
      console.error(`unknown option: ${arg}`);
      usage(2);
    }
  }
  return opts;
}

function needValue(argv, index, option) {
  if (index >= argv.length) {
    console.error(`${option} needs a value`);
    usage(2);
  }
  return argv[index];
}

function parsePositiveInt(value, option) {
  if (!/^[1-9][0-9]*$/.test(value)) {
    console.error(`${option} must be a positive integer`);
    usage(2);
  }
  return Number(value);
}

function run(command, args, options = {}) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    encoding: options.encoding ?? "utf8",
    input: options.input,
    maxBuffer,
    stdio: options.stdio,
  });
  if (result.error) throw result.error;
  return result;
}

function mustRun(command, args, options = {}) {
  const result = run(command, args, options);
  if (result.status !== 0) {
    const stderr = typeof result.stderr === "string" ? result.stderr : "";
    throw new Error(`${command} ${args.join(" ")} failed\n${stderr}`);
  }
  return result;
}

function formatterBinary() {
  if (process.env.FMT) return path.resolve(process.env.FMT);
  console.error("building release binary (cargo build --release --bin daml-fmt)...");
  mustRun("cargo", ["build", "--release", "--bin", "daml-fmt"], { stdio: "inherit" });
  const metadata = JSON.parse(
    mustRun("cargo", ["metadata", "--format-version", "1", "--no-deps"]).stdout
  );
  return path.join(metadata.target_directory, "release", process.platform === "win32" ? "daml-fmt.exe" : "daml-fmt");
}

function readManifest() {
  return fs
    .readFileSync(path.join(repoRoot, "corpus", "desugar-ok.txt"), "utf8")
    .split(/\r?\n/)
    .filter(Boolean)
    .map((rel) => {
      const normalized = path.normalize(rel);
      if (path.isAbsolute(normalized) || normalized.startsWith("..") || normalized.includes(`..${path.sep}`)) {
        throw new Error(`unsafe manifest path: ${rel}`);
      }
      return normalized.split(path.sep).join("/");
    });
}

function ensureParent(file) {
  fs.mkdirSync(path.dirname(file), { recursive: true });
}

function writeFile(file, content) {
  ensureParent(file);
  fs.writeFileSync(file, content);
}

function sha256(buffer) {
  return crypto.createHash("sha256").update(buffer).digest("hex");
}

function formatterRun(bin, input) {
  const result = spawnSync(bin, [], {
    cwd: repoRoot,
    encoding: "utf8",
    input,
    maxBuffer,
  });
  if (result.error) throw result.error;
  return {
    ok: result.status === 0,
    status: result.status,
    stdout: result.stdout ?? "",
    stderr: result.stderr ?? "",
  };
}

function unifiedDiff(tmpRoot, oldLabel, oldText, newLabel, newText) {
  if (oldText === newText) return "";
  const dir = fs.mkdtempSync(path.join(tmpRoot, "diff-"));
  const oldFile = path.join(dir, "old.daml");
  const newFile = path.join(dir, "new.daml");
  fs.writeFileSync(oldFile, oldText);
  fs.writeFileSync(newFile, newText);
  const result = spawnSync(
    "diff",
    ["-u", "--label", oldLabel, "--label", newLabel, oldFile, newFile],
    { encoding: "utf8", maxBuffer }
  );
  if (result.error) throw result.error;
  if (result.status !== 0 && result.status !== 1) {
    throw new Error(`diff failed for ${oldLabel}: ${result.stderr}`);
  }
  return result.stdout ?? "";
}

function diffStats(diffText) {
  let added = 0;
  let removed = 0;
  let hunks = 0;
  for (const line of diffText.split(/\r?\n/)) {
    if (line.startsWith("@@")) hunks += 1;
    else if (line.startsWith("+") && !line.startsWith("+++")) added += 1;
    else if (line.startsWith("-") && !line.startsWith("---")) removed += 1;
  }
  return { added, removed, hunks };
}

function runDesugar(tmpRoot, cwd, fileName) {
  const outDir = fs.mkdtempSync(path.join(tmpRoot, "desugar-out-"));
  const outFile = path.join(outDir, "out.daml");
  const result = spawnSync(
    "daml",
    ["--no-legacy-assistant-warning", "damlc", "desugar", fileName, "-o", outFile],
    { cwd, encoding: "utf8", maxBuffer }
  );
  if (result.error) throw result.error;
  const wroteOutput = fs.existsSync(outFile);
  const output = wroteOutput ? fs.readFileSync(outFile) : Buffer.alloc(0);
  return {
    ok: result.status === 0,
    wroteOutput,
    status: result.status,
    stdout: output,
    stderr: result.stderr ?? "",
  };
}

function desugarTarget(rel, source) {
  const moduleMatch = source.match(
    /^\s*module\s+([A-Za-z_][A-Za-z0-9_']*(?:\.[A-Za-z_][A-Za-z0-9_']*)*)\b/m
  );
  if (!moduleMatch) {
    return {
      rootRel: path.posix.dirname(rel) === "." ? "" : path.posix.dirname(rel),
      fileArg: path.posix.basename(rel),
    };
  }

  const modulePath = `${moduleMatch[1].replace(/\./g, "/")}.daml`;
  if (rel === modulePath) return { rootRel: "", fileArg: modulePath };
  if (rel.endsWith(`/${modulePath}`)) {
    return { rootRel: rel.slice(0, -(modulePath.length + 1)), fileArg: modulePath };
  }

  return {
    rootRel: path.posix.dirname(rel) === "." ? "" : path.posix.dirname(rel),
    fileArg: path.posix.basename(rel),
  };
}

function checkDesugar(tmpRoot, rel, originalSource, formatted) {
  const target = desugarTarget(rel, originalSource);
  const originalRoot = path.join(repoRoot, "original", target.rootRel);
  const original = runDesugar(tmpRoot, originalRoot, target.fileArg);

  const formattedDir = path.join(tmpRoot, "formatted-desugar");
  fs.rmSync(formattedDir, { recursive: true, force: true });
  fs.mkdirSync(path.dirname(path.join(formattedDir, target.fileArg)), { recursive: true });
  fs.writeFileSync(path.join(formattedDir, target.fileArg), formatted);
  const after = runDesugar(tmpRoot, formattedDir, target.fileArg);

  const originalHash = original.wroteOutput ? sha256(original.stdout) : null;
  const formattedHash = after.wroteOutput ? sha256(after.stdout) : null;
  return {
    originalOk: original.ok,
    formattedOk: after.ok,
    originalWroteOutput: original.wroteOutput,
    formattedWroteOutput: after.wroteOutput,
    cleanExit: original.ok && after.ok,
    byteIdentical:
      original.wroteOutput &&
      after.wroteOutput &&
      Buffer.compare(original.stdout, after.stdout) === 0,
    originalSha256: originalHash,
    formattedSha256: formattedHash,
    originalStatus: original.status,
    formattedStatus: after.status,
    originalStderr: original.ok ? "" : original.stderr.trim(),
    formattedStderr: after.ok ? "" : after.stderr.trim(),
  };
}

function artifactPath(outDir, kind, rel, suffix = "") {
  return path.join(outDir, kind, `${rel}${suffix}`);
}

function relLink(fromFile, toFile) {
  return path.relative(path.dirname(fromFile), toFile).split(path.sep).join("/");
}

function batchNumber(index, batchSize) {
  return Math.floor(index / batchSize) + 1;
}

function padded(n) {
  return String(n).padStart(3, "0");
}

function status(ok) {
  return ok ? "ok" : "FAIL";
}

function sampleRow(batchFile, result) {
  return [
    result.ordinal,
    `[${result.rel}](${relLink(batchFile, path.join(repoRoot, "original", result.rel))})`,
    `[diff](${relLink(batchFile, result.diffPath)})`,
    `[formatted](${relLink(batchFile, result.formattedPath)})`,
    status(result.formatterOk),
    status(result.matchesExpected),
    status(result.idempotent),
    result.desugarSkipped ? "skipped" : status(result.desugarByteIdentical),
    result.desugarSkipped ? "skipped" : status(result.desugarCleanExit),
  ].join(" | ");
}

function writeBatchReports(outDir, results, opts, totalSamples) {
  const batches = new Map();
  for (const result of results) {
    const batch = batchNumber(result.index, opts.batchSize);
    if (!batches.has(batch)) batches.set(batch, []);
    batches.get(batch).push(result);
  }

  const batchFiles = [];
  for (const [batch, batchResults] of batches) {
    const batchFile = path.join(outDir, "batches", `batch-${padded(batch)}.md`);
    const reviewFile = path.join(outDir, "reviews", `batch-${padded(batch)}.md`);
    const first = batchResults[0].ordinal;
    const last = batchResults[batchResults.length - 1].ordinal;
    const failures = batchResults.filter((r) => !r.mechanicalOk);
    const desugarExitWarnings = batchResults.filter(
      (r) => !r.desugarSkipped && r.desugarByteIdentical && !r.desugarCleanExit
    );
    const changed = batchResults.filter((r) => r.changedFromOriginal).length;

    const body = [
      `# daml-fmt audit batch ${padded(batch)}`,
      "",
      `Samples: ${first}-${last} of ${totalSamples}`,
      `Changed by formatter: ${changed}/${batchResults.length}`,
      `Mechanical failures: ${failures.length}`,
      `Desugar exit warnings: ${desugarExitWarnings.length}`,
      "",
      "Reviewer scope:",
      "- Inspect every source->formatted diff in this batch.",
      "- Confirm the formatted output is accurate and consistent with the documented daml-fmt layout rules.",
      "- Treat any failed formatter, expected-baseline, idempotence, or desugar check as blocking.",
      "- Do not edit formatter code from this batch review; record findings in the paired review note.",
      "",
      `Review note: [reviews/batch-${padded(batch)}.md](${relLink(batchFile, reviewFile)})`,
      "",
      "| # | sample | diff | formatted | formatter | expected | idempotent | desugar bytes | desugar exit |",
      "|---:|---|---|---|---|---|---|---|---|",
      ...batchResults.map((result) => `| ${sampleRow(batchFile, result)} |`),
      "",
    ].join("\n");
    writeFile(batchFile, body);

    const reviewBody = [
      `# Review notes for batch ${padded(batch)}`,
      "",
      "Outcome: pending",
      "",
      "| sample | diff accurate? | consistent? | notes |",
      "|---|---|---|---|",
      ...batchResults.map((result) => `| ${result.rel} | pending | pending |  |`),
      "",
    ].join("\n");
    writeFile(reviewFile, reviewBody);
    batchFiles.push({
      batch,
      batchFile,
      reviewFile,
      count: batchResults.length,
      failures: failures.length,
      desugarExitWarnings: desugarExitWarnings.length,
    });
  }
  return batchFiles;
}

function writeSummary(outDir, results, batchFiles, opts, formatter, elapsedMs) {
  const total = results.length;
  const counts = {
    formatterOk: results.filter((r) => r.formatterOk).length,
    changed: results.filter((r) => r.changedFromOriginal).length,
    expected: results.filter((r) => r.matchesExpected).length,
    idempotent: results.filter((r) => r.idempotent).length,
    desugar: results.filter((r) => r.desugarByteIdentical).length,
    desugarClean: results.filter((r) => r.desugarCleanExit).length,
    mechanical: results.filter((r) => r.mechanicalOk).length,
  };
  const summaryFile = path.join(outDir, "SUMMARY.md");
  const lines = [
    "# daml-fmt audit summary",
    "",
    `Formatter: \`${formatter}\``,
    `Samples audited: ${total}`,
    `Batch size: ${opts.batchSize}`,
    `Batches: ${batchFiles.length}`,
    `Elapsed: ${(elapsedMs / 1000).toFixed(1)}s`,
    "",
    "| check | result |",
    "|---|---:|",
    `| formatter stdin completed | ${counts.formatterOk}/${total} |`,
    `| changed from original | ${counts.changed}/${total} |`,
    `| matches expected baseline | ${counts.expected}/${total} |`,
    `| idempotent | ${counts.idempotent}/${total} |`,
    `| desugar byte-identical | ${opts.desugar ? `${counts.desugar}/${total}` : "skipped"} |`,
    `| desugar clean compiler exit | ${opts.desugar ? `${counts.desugarClean}/${total}` : "skipped"} |`,
    `| all mechanical checks passed | ${counts.mechanical}/${total} |`,
    "",
    "Generated artifacts:",
    "- `samples.jsonl` has one machine-readable result per sample.",
    "- `desugar-hashes.tsv` records compiler desugar hashes when desugar is enabled.",
    "- `formatted/` contains formatter stdout for each sample.",
    "- `diffs/` contains source->formatted unified diffs.",
    "- `batches/` contains one review packet per subagent.",
    "- `reviews/` contains one review-note template per batch.",
    "",
    "| batch | samples | mechanical failures | desugar exit warnings | packet | review note |",
    "|---:|---:|---:|---:|---|---|",
    ...batchFiles.map((batch) => {
      const packet = relLink(summaryFile, batch.batchFile);
      const review = relLink(summaryFile, batch.reviewFile);
      return `| ${batch.batch} | ${batch.count} | ${batch.failures} | ${batch.desugarExitWarnings} | [batch-${padded(batch.batch)}](${packet}) | [review](${review}) |`;
    }),
    "",
  ];
  writeFile(summaryFile, lines.join("\n"));

  const promptFile = path.join(outDir, "SUBAGENT_PROMPTS.md");
  const prompts = [
    "# Subagent prompts",
    "",
    "Assign one batch to each subagent. With the default batch size, 924 samples produce 37 batches; the last batch has 24 samples.",
    "",
    ...batchFiles.map((batch) => {
      const packet = relLink(promptFile, batch.batchFile);
      return [
        `## Batch ${padded(batch.batch)}`,
        "",
        `Review \`${packet}\`. Inspect every diff and formatted output listed there. Report any formatting that looks inaccurate, inconsistent with nearby samples, or inconsistent with the daml-fmt rules. Treat mechanical failures as blocking and include the sample path. Do not change formatter code during review.`,
        "",
      ].join("\n");
    }),
  ];
  writeFile(promptFile, prompts.join("\n"));
}

function main() {
  const opts = parseArgs(process.argv.slice(2));
  const started = Date.now();
  const formatter = formatterBinary();
  const manifest = readManifest();
  const totalBatches = Math.ceil(manifest.length / opts.batchSize);
  if (opts.batch !== null && opts.batch > totalBatches) {
    throw new Error(`--batch ${opts.batch} exceeds ${totalBatches} batches`);
  }

  const selected = manifest
    .map((rel, index) => ({ rel, index }))
    .filter(({ index }) => opts.batch === null || batchNumber(index, opts.batchSize) === opts.batch);

  fs.rmSync(opts.outDir, { recursive: true, force: true });
  fs.mkdirSync(opts.outDir, { recursive: true });
  const tmpRoot = fs.mkdtempSync(path.join(os.tmpdir(), "daml-fmt-audit-"));
  process.on("exit", () => fs.rmSync(tmpRoot, { recursive: true, force: true }));

  const jsonl = [];
  const hashLines = ["sample\toriginal_desugar_sha256\tformatted_desugar_sha256"];
  const results = [];

  for (const { rel, index } of selected) {
    const originalPath = path.join(repoRoot, "original", rel);
    const expectedPath = path.join(repoRoot, "expected", rel);
    const original = fs.readFileSync(originalPath, "utf8");
    const expected = fs.readFileSync(expectedPath, "utf8");
    const formattedPath = artifactPath(opts.outDir, "formatted", rel);
    const diffPath = artifactPath(opts.outDir, "diffs", rel, ".diff");

    const formattedRun = formatterRun(formatter, original);
    const formatted = formattedRun.stdout;
    writeFile(formattedPath, formatted);

    const secondRun = formattedRun.ok ? formatterRun(formatter, formatted) : { ok: false, stdout: "" };
    const diffText = unifiedDiff(tmpRoot, `original/${rel}`, original, `formatted/${rel}`, formatted);
    writeFile(diffPath, diffText || "no changes\n");
    const stats = diffStats(diffText);

    const desugar = opts.desugar
      ? checkDesugar(tmpRoot, rel, original, formatted)
      : {
          originalOk: false,
          formattedOk: false,
          originalWroteOutput: false,
          formattedWroteOutput: false,
          cleanExit: false,
          byteIdentical: false,
          originalSha256: null,
          formattedSha256: null,
          originalStatus: null,
          formattedStatus: null,
          originalStderr: "",
          formattedStderr: "",
        };

    if (opts.desugar) {
      hashLines.push(`${rel}\t${desugar.originalSha256 ?? "FAIL"}\t${desugar.formattedSha256 ?? "FAIL"}`);
    }

    const result = {
      index,
      ordinal: index + 1,
      batch: batchNumber(index, opts.batchSize),
      rel,
      originalPath,
      expectedPath,
      formattedPath,
      diffPath,
      formatterOk: formattedRun.ok,
      formatterStatus: formattedRun.status,
      formatterStderr: formattedRun.stderr.trim(),
      changedFromOriginal: original !== formatted,
      matchesExpected: formattedRun.ok && formatted === expected,
      idempotent: formattedRun.ok && secondRun.ok && secondRun.stdout === formatted,
      diffAddedLines: stats.added,
      diffRemovedLines: stats.removed,
      diffHunks: stats.hunks,
      desugarSkipped: !opts.desugar,
      desugarOriginalOk: desugar.originalOk,
      desugarFormattedOk: desugar.formattedOk,
      desugarOriginalWroteOutput: desugar.originalWroteOutput,
      desugarFormattedWroteOutput: desugar.formattedWroteOutput,
      desugarCleanExit: opts.desugar && desugar.cleanExit,
      desugarByteIdentical: opts.desugar && desugar.byteIdentical,
      desugarOriginalSha256: desugar.originalSha256,
      desugarFormattedSha256: desugar.formattedSha256,
      desugarOriginalStatus: desugar.originalStatus,
      desugarFormattedStatus: desugar.formattedStatus,
      desugarOriginalStderr: desugar.originalStderr,
      desugarFormattedStderr: desugar.formattedStderr,
    };
    result.mechanicalOk =
      result.formatterOk &&
      result.matchesExpected &&
      result.idempotent &&
      (result.desugarSkipped || result.desugarByteIdentical);

    results.push(result);
    jsonl.push(JSON.stringify(result));

    if ((results.length % 25 === 0) || results.length === selected.length) {
      console.error(`audited ${results.length}/${selected.length} samples`);
    }
  }

  writeFile(path.join(opts.outDir, "samples.jsonl"), `${jsonl.join("\n")}\n`);
  writeFile(path.join(opts.outDir, "desugar-hashes.tsv"), `${hashLines.join("\n")}\n`);
  const batchFiles = writeBatchReports(opts.outDir, results, opts, manifest.length);
  writeSummary(opts.outDir, results, batchFiles, opts, formatter, Date.now() - started);

  const failed = results.filter((r) => !r.mechanicalOk);
  console.log(`audit artifacts: ${opts.outDir}`);
  console.log(`samples: ${results.length}/${manifest.length}`);
  console.log(`batches: ${batchFiles.length}`);
  console.log(`mechanical failures: ${failed.length}`);
  process.exit(failed.length === 0 ? 0 : 1);
}

try {
  main();
} catch (err) {
  console.error(err && err.stack ? err.stack : String(err));
  process.exit(2);
}
