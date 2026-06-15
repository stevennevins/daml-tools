//! daml-fmt CLI — the formatter. (Replaced the retired Node bin/daml-fmt.js.)
//!
//!   daml-fmt <file...>         print formatted source to stdout
//!   daml-fmt -w <file...>      rewrite files in place (only when changed)
//!   daml-fmt --check <file...> exit 1 if any file would change
//!   daml-fmt                   read stdin, write formatted source to stdout
//!
//! Malformed input (unterminated string / block comment) is reported to stderr
//! and exits 2 in every mode; `-w` never rewrites it. Output is unchanged
//! (byte-faithful passthrough) — only the success signal changes.
//!
//! Backend is the AST-driven formatter (`format_source` -> layout_ast).

use daml_fmt::{format_source, lex_diagnostics};
use std::io::Read;
use std::process::exit;

/// Print any lexer diagnostics for `src` to stderr, prefixed with `label`.
/// Returns true when the source is malformed (had diagnostics).
fn report_lex_errors(label: &str, src: &str) -> bool {
    let diags = lex_diagnostics(src);
    for d in &diags {
        eprintln!("daml-fmt: {}: {}", label, d);
    }
    !diags.is_empty()
}

fn usage(code: i32) -> ! {
    eprint!(
        "usage: daml-fmt [options] [file...]\n\
         \n\
         \x20 -w, --write    rewrite files in place\n\
         \x20     --check    exit 1 if any file is not formatted\n\
         \x20 -h, --help     show this help\n\
         \x20 -v, --version  show version\n\
         \n\
         With no files, reads stdin and writes the result to stdout.\n"
    );
    exit(code);
}

fn main() {
    let mut write = false;
    let mut check = false;
    let mut files: Vec<String> = Vec::new();
    for a in std::env::args().skip(1) {
        match a.as_str() {
            "-w" | "--write" => write = true,
            "--check" => check = true,
            "-h" | "--help" => usage(0),
            "-v" | "--version" => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                exit(0);
            }
            s if s.starts_with('-') => {
                eprintln!("daml-fmt: unknown option '{}'", s);
                usage(2);
            }
            _ => files.push(a),
        }
    }
    if write && check {
        eprintln!("daml-fmt: --write and --check are mutually exclusive");
        exit(2);
    }

    if files.is_empty() {
        if write || check {
            eprintln!("daml-fmt: --write/--check need file arguments");
            exit(2);
        }
        let mut text = String::new();
        if std::io::stdin().read_to_string(&mut text).is_err() {
            eprintln!("daml-fmt: failed to read stdin");
            exit(2);
        }
        let malformed = report_lex_errors("<stdin>", &text);
        print!("{}", format_source(&text));
        exit(if malformed { 2 } else { 0 });
    }

    let mut failed = 0;
    let mut unformatted = 0;
    for file in &files {
        let text = match std::fs::read_to_string(file) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("daml-fmt: {}: {}", file, e);
                failed += 1;
                continue;
            }
        };
        // Malformed input (unterminated string / block comment) must not look
        // like a successful format: count it as failed (exit 2) in every mode,
        // and never rewrite it in -w. Output stays byte-faithful passthrough.
        if report_lex_errors(file, &text) {
            failed += 1;
            if !check && !write {
                print!("{}", format_source(&text));
            }
            continue;
        }
        let out = format_source(&text);
        if check {
            if out != text {
                println!("{}", file);
                unformatted += 1;
            }
        } else if write {
            if out != text {
                if let Err(e) = std::fs::write(file, &out) {
                    eprintln!("daml-fmt: {}: {}", file, e);
                    failed += 1;
                }
            }
        } else {
            print!("{}", out);
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
