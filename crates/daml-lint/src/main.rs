use clap::Parser;
use daml_lint::detector::{self, Severity};
use daml_lint::reporter::{self, OutputFormat};
use daml_lint::{detectors, parser};
use daml_syntax::{CharColumn, LineNumber};
use std::io::Write as _;
use std::path::{Path, PathBuf};

#[cfg(feature = "custom-rules")]
mod config;
#[cfg(feature = "custom-rules")]
mod rule_registry;

#[derive(Parser)]
#[command(name = "daml-lint")]
#[command(about = "Static analysis scanner for DAML smart contracts")]
#[command(version)]
struct Cli {
    /// DAML files or directories to scan
    #[arg(required = true)]
    paths: Vec<PathBuf>,

    /// Output format: sarif, markdown, json
    #[arg(short, long, default_value = "markdown", value_parser = parse_output_format)]
    format: OutputFormat,

    /// Output file (default: stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Minimum severity to cause non-zero exit: critical, high, medium, low, info
    #[arg(long, default_value = "high", value_parser = parse_fail_on)]
    fail_on: Severity,

    /// YAML config file with daml-tools.lint settings (default: ./daml.yaml)
    #[cfg(feature = "custom-rules")]
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Built-in or plugin rule id to run (repeatable). Replaces config rule/group selection.
    #[cfg(feature = "custom-rules")]
    #[arg(long)]
    rule: Vec<String>,

    /// Built-in or plugin rule group to run (repeatable). Replaces config rule/group selection.
    #[cfg(feature = "custom-rules")]
    #[arg(long)]
    group: Vec<String>,

    /// Custom AST rule scripts (JavaScript), repeatable. Write in TypeScript
    /// against examples/daml-lint.d.ts and compile; see examples/
    #[cfg(feature = "custom-rules")]
    #[arg(long)]
    rules: Vec<PathBuf>,
}

fn main() {
    let cli = Cli::parse();

    // Load detectors first so rule-file errors surface before scanning
    #[cfg(feature = "custom-rules")]
    let detectors = {
        let lint_config = config::LintConfig::load(cli.config.as_deref()).unwrap_or_else(|e| {
            eprintln!("Error: {e}");
            std::process::exit(2);
        });
        let selection = lint_config
            .resolve_effective_rules(&cli.group, &cli.rule)
            .unwrap_or_else(|e| {
                eprintln!("Error: {e}");
                std::process::exit(2);
            });
        let mut detectors = detectors::create_builtin_detectors();
        match lint_config.load_plugin_detectors(&selection) {
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
        if let Err(e) =
            config::LintConfig::validate_selection_against_detectors(&selection, &detectors)
        {
            eprintln!("Error: {e}");
            std::process::exit(2);
        }
        let respect_disabled = selection.source != config::RuleSelectionSource::Cli;
        let detectors = lint_config.filter_by_selection(detectors, &selection);
        let detectors = lint_config.apply_rule_settings(detectors, respect_disabled);
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
    let (files, mut input_errors) = discover_daml_files(&cli.paths);
    if !input_errors.is_empty() {
        for error in &input_errors {
            eprintln!("{error}");
        }
    }
    if files.is_empty() && input_errors.is_empty() {
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
                eprintln!("Error: could not read {}: {e}", file.display());
                input_errors.push(InputDiscoveryError::ReadFailure {
                    path: file.clone(),
                    error: e,
                });
                continue;
            }
        };

        let parse_result = parser::parse_daml_with_diagnostics(&source, file);
        let module = parse_result.module;
        for d in &parse_result.diagnostics {
            eprintln!(
                "daml-lint: parse [{}]: {}:{}:{}: {}",
                d.category.as_str(),
                file.display(),
                d.line,
                d.column,
                d.message
            );
            parse_errors.push(reporter::ParseError::new(
                file.display().to_string(),
                d.line,
                d.column,
                d.end_column,
                d.message.clone(),
                d.category,
            ));
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

    // Sort findings by explicit severity rank, then file, then line.
    all_findings.sort_by(|a, b| {
        b.severity
            .rank()
            .cmp(&a.severity.rank())
            .then_with(|| a.file.cmp(&b.file))
            .then_with(|| a.line.cmp(&b.line))
    });

    // Include input-path failures in the report channel so the output never
    // reads as clean when input could not be discovered/read.
    let mut reportable_parse_errors = parse_errors.clone();
    reportable_parse_errors.extend(input_errors.iter().map(input_error_to_parse_error));

    // Format output
    let output = reporter::format_findings(&all_findings, &reportable_parse_errors, cli.format);

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
        print!("{output}");
        let _ = std::io::stdout().flush();
    }

    // Parse failures mean the scan is not authoritative: signal that with a
    // distinct exit code (3) so callers can tell it apart from a clean scan (0),
    // a findings-over-threshold scan (1), and usage/IO errors (2). This is
    // independent of --fail-on, which only governs findings severity.
    let code = if input_errors.is_empty() {
        if parse_errors.is_empty() {
            reporter::exit_code(&all_findings, cli.fail_on)
        } else {
            3
        }
    } else {
        2
    };
    std::process::exit(code);
}

fn input_error_to_parse_error(error: &InputDiscoveryError) -> reporter::ParseError {
    reporter::ParseError::new(
        match error {
            InputDiscoveryError::NotFound(path)
            | InputDiscoveryError::NotADirectory(path)
            | InputDiscoveryError::ReadDir { path, .. }
            | InputDiscoveryError::ReadFailure { path, .. } => path.display().to_string(),
        },
        LineNumber::new(1),
        CharColumn::new(1),
        None,
        error.to_string(),
        parser::ParseDiagnosticCategory::UnsupportedSyntax,
    )
}

fn parse_output_format(value: &str) -> Result<OutputFormat, String> {
    value
        .parse::<OutputFormat>()
        .map_err(|_| format!("Unknown format '{value}'. Use sarif, markdown, or json."))
}

fn parse_fail_on(value: &str) -> Result<Severity, String> {
    value.parse::<Severity>().map_err(|e| e.to_string())
}

#[derive(Debug)]
enum InputDiscoveryError {
    NotFound(PathBuf),
    NotADirectory(PathBuf),
    ReadDir {
        path: PathBuf,
        error: std::io::Error,
    },
    ReadFailure {
        path: PathBuf,
        error: std::io::Error,
    },
}

impl std::fmt::Display for InputDiscoveryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(path) => {
                write!(f, "Error: scan path {} does not exist", path.display())
            }
            Self::NotADirectory(path) => {
                write!(
                    f,
                    "Error: scan path {} is not a file or directory",
                    path.display()
                )
            }
            Self::ReadDir { path, error } => {
                write!(
                    f,
                    "Error: could not read directory {}: {error}",
                    path.display()
                )
            }
            Self::ReadFailure { path, error } => {
                write!(f, "Error: could not read {}: {error}", path.display())
            }
        }
    }
}

fn discover_daml_files(paths: &[PathBuf]) -> (Vec<PathBuf>, Vec<InputDiscoveryError>) {
    let mut daml_files = Vec::new();
    let mut errors = Vec::new();
    for path in paths {
        collect_input_path(path, &mut daml_files, &mut errors);
    }
    daml_files.sort();
    (daml_files, errors)
}

fn collect_input_path(
    path: &Path,
    daml_files: &mut Vec<PathBuf>,
    errors: &mut Vec<InputDiscoveryError>,
) {
    match path.metadata() {
        Ok(_md) if _md.is_file() => {
            if path.extension().is_some_and(|e| e == "daml") {
                daml_files.push(path.to_path_buf());
            }
        }
        Ok(md) if md.is_dir() => {
            let entries = match std::fs::read_dir(path) {
                Ok(entries) => entries,
                Err(error) => {
                    errors.push(InputDiscoveryError::ReadDir {
                        path: path.to_path_buf(),
                        error,
                    });
                    return;
                }
            };
            for entry in entries {
                match entry {
                    Ok(entry) => collect_input_path(&entry.path(), daml_files, errors),
                    Err(error) => errors.push(InputDiscoveryError::ReadDir {
                        path: path.to_path_buf(),
                        error,
                    }),
                }
            }
        }
        Ok(_) => {
            errors.push(InputDiscoveryError::NotADirectory(path.to_path_buf()));
        }
        Err(error) => {
            if error.kind() == std::io::ErrorKind::NotFound {
                errors.push(InputDiscoveryError::NotFound(path.to_path_buf()));
            } else {
                errors.push(InputDiscoveryError::ReadDir {
                    path: path.to_path_buf(),
                    error,
                });
            }
        }
    };
}
