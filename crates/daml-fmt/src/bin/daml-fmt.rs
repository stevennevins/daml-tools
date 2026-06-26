//! daml-fmt CLI — the formatter. (Replaced the retired Node bin/daml-fmt.js.)
//!
//!   daml-fmt <file...>         rewrite files in place with selected rules
//!   daml-fmt -w <file...>      same as default file mode
//!   daml-fmt --check <file...> exit 1 if any file would change
//!   daml-fmt --preserve-import-order <file...>
//!   daml-fmt                   read stdin, write formatted source to stdout
//!
//! Malformed input (lexical or parser diagnostic) is reported to stderr and exits
//! 2 in every mode; write mode never rewrites it. Output is unchanged
//! (byte-faithful passthrough) — only the success signal changes.
//!
//! Backend is the AST-driven formatter (`format_source_with_options` ->
//! `layout_ast`).

use daml_fmt::{
    format_source_with_options, source_diagnostics, FmtConfig, FormatOptions, ImportOrder,
};
use std::io::Read;
use std::path::PathBuf;
use std::process::exit;

/// Print source diagnostics for `src` to stderr, prefixed with `label`.
/// Returns `true` when diagnostics are present.
fn report_diagnostics(label: &str, src: &str) -> bool {
    let diags = source_diagnostics(src);
    for d in &diags {
        eprintln!("daml-fmt: {label}: {d}");
    }
    !diags.is_empty()
}

fn usage(code: i32) -> ! {
    eprint!(
        "usage: daml-fmt [options] [file...]\n\
         \n\
         \x20 -w, --write    rewrite files in place (default for file arguments)\n\
         \x20     --check    exit 1 if any file is not formatted\n\
         \x20     --config <FILE>  YAML config with daml-tools.fmt settings\n\
         \x20     --rule <ID>  formatter rule to apply (repeatable; replaces config rules)\n\
         \x20     --preserve-import-order  remove import-order from the default rule set\n\
         \x20 -h, --help     show this help\n\
         \x20 -v, --version  show version\n\
         \n\
         With no files, reads stdin and writes the result to stdout.\n"
    );
    exit(code);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Print,
    Write,
    Check,
}

fn main() {
    let mut explicit_write = false;
    let mut explicit_check = false;
    let mut preserve_import_order = false;
    let mut config_path: Option<PathBuf> = None;
    let mut cli_rules: Vec<String> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "-w" | "--write" => explicit_write = true,
            "--check" => explicit_check = true,
            "--preserve-import-order" => preserve_import_order = true,
            "--config" => {
                let Some(path) = args.next() else {
                    eprintln!("daml-fmt: --config requires a file argument");
                    usage(2);
                };
                config_path = Some(path.into());
            }
            "--rule" => {
                let Some(rule) = args.next() else {
                    eprintln!("daml-fmt: --rule requires a rule id");
                    usage(2);
                };
                cli_rules.push(rule);
            }
            "-h" | "--help" => usage(0),
            "-v" | "--version" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                exit(0);
            }
            s if s.starts_with('-') => {
                eprintln!("daml-fmt: unknown option '{s}'");
                usage(2);
            }
            s => files.push(s.into()),
        }
    }
    if explicit_write && explicit_check {
        eprintln!("daml-fmt: --write and --check are mutually exclusive");
        exit(2);
    }
    if files.is_empty() && (explicit_write || explicit_check) {
        eprintln!("daml-fmt: --write/--check need file arguments");
        exit(2);
    }
    let mode = if files.is_empty() {
        Mode::Print
    } else if explicit_check {
        Mode::Check
    } else {
        Mode::Write
    };

    let fmt_config = match FmtConfig::load(config_path.as_deref()) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("daml-fmt: {err}");
            exit(2);
        }
    };
    let (rules, _) = match fmt_config.resolve_effective_rules(&cli_rules, preserve_import_order) {
        Ok(selection) => selection,
        Err(err) => {
            eprintln!("daml-fmt: {err}");
            exit(2);
        }
    };
    let import_order = if preserve_import_order {
        ImportOrder::Preserve
    } else {
        ImportOrder::Organize
    };
    let options = FormatOptions::new()
        .with_import_order(import_order)
        .with_rules(rules);

    if files.is_empty() {
        let mut text = String::new();
        if let Err(e) = std::io::stdin().read_to_string(&mut text) {
            eprintln!("daml-fmt: failed to read stdin: {e}");
            exit(2);
        }
        let malformed = report_diagnostics("<stdin>", &text);
        if malformed {
            print!("{}", &text);
            exit(2);
        }
        print!("{}", format_source_with_options(&text, options));
        exit(0);
    }

    let mut failed = 0;
    let mut unformatted = 0;
    for file in &files {
        let text = match std::fs::read_to_string(file) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("daml-fmt: {}: {e}", file.display());
                failed += 1;
                continue;
            }
        };
        // Malformed input (unterminated string / block comment) must not look
        // like a successful format: count it as failed (exit 2) in every mode,
        // and never rewrite it in write mode. Output stays byte-faithful passthrough.
        if report_diagnostics(&file.display().to_string(), &text) {
            failed += 1;
            if mode == Mode::Print {
                print!("{}", &text);
            }
            continue;
        }
        let out = format_source_with_options(&text, options);
        if mode == Mode::Check {
            if out != text {
                println!("{}", file.display());
                unformatted += 1;
            }
        } else if mode == Mode::Write {
            if out != text {
                if let Err(e) = std::fs::write(file, &out) {
                    eprintln!("daml-fmt: {}: {e}", file.display());
                    failed += 1;
                }
            }
        } else {
            print!("{out}");
        }
    }
    exit(if failed > 0 {
        2
    } else if unformatted > 0 {
        1
    } else {
        0
    });
}
