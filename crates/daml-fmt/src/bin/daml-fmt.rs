//! daml-fmt CLI — the formatter. (Replaced the retired Node bin/daml-fmt.js.)
//!
//!   daml-fmt <file...>         print formatted source to stdout
//!   daml-fmt -w <file...>      rewrite files in place (only when changed)
//!   daml-fmt --check <file...> exit 1 if any file would change
//!   daml-fmt --preserve-import-order <file...>
//!   daml-fmt                   read stdin, write formatted source to stdout
//!
//! Malformed input (lexical or parser diagnostic) is reported to stderr and exits
//! 2 in every mode; `-w` never rewrites it. Output is unchanged
//! (byte-faithful passthrough) — only the success signal changes.
//!
//! Backend is the AST-driven formatter (`format_source_with_options` ->
//! `layout_ast`).

use daml_fmt::{format_source_with_options, source_diagnostics, FormatOptions, ImportOrder};
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
         \x20 -w, --write    rewrite files in place\n\
         \x20     --check    exit 1 if any file is not formatted\n\
         \x20     --preserve-import-order  do not reorder import declarations\n\
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
    let mut mode = Mode::Print;
    let mut options = FormatOptions::default();
    let mut files: Vec<PathBuf> = Vec::new();
    let mut mode_conflict = false;
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "-w" | "--write" => {
                if mode == Mode::Check {
                    mode_conflict = true;
                    mode = Mode::Print;
                } else {
                    mode = Mode::Write;
                }
            }
            "--check" => {
                if mode == Mode::Write {
                    mode_conflict = true;
                    mode = Mode::Print;
                } else {
                    mode = Mode::Check;
                }
            }
            "--preserve-import-order" => {
                options = options.with_import_order(ImportOrder::Preserve);
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
    if mode_conflict {
        eprintln!("daml-fmt: --write and --check are mutually exclusive");
        exit(2);
    }

    if files.is_empty() {
        if mode != Mode::Print {
            eprintln!("daml-fmt: --write/--check need file arguments");
            exit(2);
        }
        let mut text = String::new();
        if std::io::stdin().read_to_string(&mut text).is_err() {
            eprintln!("daml-fmt: failed to read stdin");
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
        // and never rewrite it in -w. Output stays byte-faithful passthrough.
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
