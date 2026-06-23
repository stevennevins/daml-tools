use clap::Parser;
use daml_lint::detector::{self, parse_severity};
use daml_lint::reporter::{self, OutputFormat};
use daml_lint::{detectors, parser};
use std::path::PathBuf;

#[cfg(feature = "custom-rules")]
mod config;

#[derive(Parser)]
#[command(name = "daml-lint")]
#[command(about = "Static analysis scanner for DAML smart contracts")]
#[command(version)]
struct Cli {
    /// DAML files or directories to scan
    #[arg(required = true)]
    paths: Vec<PathBuf>,

    /// Output format: sarif, markdown, json
    #[arg(short, long, default_value = "markdown")]
    format: String,

    /// Output file (default: stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Minimum severity to cause non-zero exit: critical, high, medium, low, info
    #[arg(long, default_value = "high")]
    fail_on: String,

    /// JSON config file with plugins and rule settings (default: .daml-lint.json)
    #[cfg(feature = "custom-rules")]
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Custom AST rule scripts (JavaScript), repeatable. Write in TypeScript
    /// against examples/daml-lint.d.ts and compile; see examples/
    #[cfg(feature = "custom-rules")]
    #[arg(long)]
    rules: Vec<PathBuf>,
}

fn main() {
    let cli = Cli::parse();

    let format = cli.format.parse::<OutputFormat>().unwrap_or_else(|()| {
        eprintln!(
            "Unknown format '{}'. Use sarif, markdown, or json.",
            cli.format
        );
        std::process::exit(2);
    });

    let fail_on = parse_severity(&cli.fail_on).unwrap_or_else(|| {
        eprintln!(
            "Unknown severity '{}'. Use critical, high, medium, low, or info.",
            cli.fail_on
        );
        std::process::exit(2);
    });

    // Load detectors first so rule-file errors surface before scanning
    #[cfg(feature = "custom-rules")]
    let detectors = {
        let lint_config = config::LintConfig::load(cli.config.as_deref()).unwrap_or_else(|e| {
            eprintln!("Error: {e}");
            std::process::exit(2);
        });
        let mut detectors = detectors::create_builtin_detectors();
        match lint_config.load_plugin_detectors() {
            Ok(plugin_detectors) => detectors.extend(plugin_detectors),
            Err(e) => {
                eprintln!("Error: {e}");
                std::process::exit(2);
            }
        }
        for rules_path in &cli.rules {
            match detectors::script::load_script(rules_path) {
                Ok(rule) => detectors.push(rule),
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(2);
                }
            }
        }
        let detectors = lint_config.apply_rule_settings(detectors);
        if let Err(e) = lint_config.validate_rule_settings(&detectors) {
            eprintln!("Error: {e}");
            std::process::exit(2);
        }
        detectors
    };
    #[cfg(not(feature = "custom-rules"))]
    let detectors = detectors::create_builtin_detectors();
    if let Some(duplicate_detector_name) = detector::find_duplicate_detector_name(&detectors) {
        eprintln!(
            "Error: rule '{duplicate_detector_name}': name collides with a built-in detector or another rule"
        );
        std::process::exit(2);
    }

    // Discover .daml files
    let files = discover_daml_files(&cli.paths);
    if files.is_empty() {
        eprintln!("No .daml files found.");
        std::process::exit(2);
    }

    eprintln!("daml-lint: scanning {} file(s)...", files.len());
    let mut all_findings = Vec::new();
    let mut parse_errors = Vec::new();

    for file in &files {
        let source = match std::fs::read_to_string(file) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Warning: could not read {}: {e}", file.display());
                continue;
            }
        };

        let (module, diagnostics) = parser::parse_daml_with_diagnostics(&source, file);
        for d in &diagnostics {
            eprintln!(
                "daml-lint: parse [{}]: {}:{}:{}: {}",
                d.category.as_str(),
                file.display(),
                d.line,
                d.column,
                d.message
            );
            parse_errors.push(reporter::ParseError {
                file: file.display().to_string(),
                line: d.line,
                column: d.column,
                end_column: d.end_column,
                message: d.message.clone(),
                category: d.category,
            });
        }

        for det in &detectors {
            match det.try_detect(&module) {
                Ok(findings) => all_findings.extend(findings),
                Err(e) => {
                    eprintln!("Error: {e}");
                    std::process::exit(2);
                }
            }
        }
    }

    // Sort findings by severity, then file, then line
    all_findings.sort_by(|a, b| {
        a.severity
            .cmp(&b.severity)
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
    });

    // Format output
    let output = reporter::format_findings(&all_findings, &parse_errors, format);

    if let Some(output_path) = &cli.output {
        std::fs::write(output_path, &output).unwrap_or_else(|e| {
            eprintln!("Error writing to {}: {e}", output_path.display());
            std::process::exit(2);
        });
        eprintln!(
            "daml-lint: {} finding(s) written to {}",
            all_findings.len(),
            output_path.display()
        );
    } else {
        println!("{output}");
    }

    // Parse failures mean the scan is not authoritative: signal that with a
    // distinct exit code (3) so callers can tell it apart from a clean scan (0),
    // a findings-over-threshold scan (1), and usage/IO errors (2). This is
    // independent of --fail-on, which only governs findings severity.
    let code = if parse_errors.is_empty() {
        reporter::exit_code(&all_findings, fail_on)
    } else {
        3
    };
    std::process::exit(code);
}

fn discover_daml_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut daml_files = Vec::new();
    for path in paths {
        if path.is_file() {
            if path.extension().is_some_and(|e| e == "daml") {
                daml_files.push(path.clone());
            }
        } else if path.is_dir() {
            collect_daml_files(path, &mut daml_files);
        } else {
            eprintln!("Warning: scan path {} does not exist.", path.display());
        }
    }
    daml_files.sort();
    daml_files
}

fn collect_daml_files(dir: &PathBuf, daml_files: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_daml_files(&path, daml_files);
            } else if path.extension().is_some_and(|e| e == "daml") {
                daml_files.push(path);
            }
        }
    }
}
