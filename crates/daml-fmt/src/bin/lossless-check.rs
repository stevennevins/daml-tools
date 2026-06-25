//! Phase (a) proof: the daml-parser lexer pipeline can reconstruct every corpus
//! file byte-for-byte. No formatting yet — just prove the pipeline is lossless
//! before we re-lay-out anything.
//!
//! Usage: `lossless-check <dir-or-file>...`
//! Exit 0 iff every file round-trips byte-identical through
//! `lex_with_trivia` -> `render_lossless`.

use daml_parser::lexer::render_lossless;
use daml_syntax::SourceTokens;
use std::path::{Path, PathBuf};
use std::process::exit;

fn collect(path: &Path, out: &mut Vec<PathBuf>) -> usize {
    if path.is_dir() {
        let entries = match std::fs::read_dir(path) {
            Ok(entries) => entries,
            Err(e) => {
                println!("READ-ERR {}: {}", path.display(), e);
                return 1;
            }
        };
        let mut entry_errors = 0usize;
        let mut entries: Vec<_> = entries
            .filter_map(|entry| match entry {
                Ok(entry) => Some(entry.path()),
                Err(e) => {
                    println!("READ-ERR {}: {}", path.display(), e);
                    entry_errors += 1;
                    None
                }
            })
            .collect();
        entries.sort();
        for e in entries {
            entry_errors += collect(&e, out);
        }
        entry_errors
    } else if path.extension().is_some_and(|x| x == "daml") {
        out.push(path.to_path_buf());
        0
    } else {
        0
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage: lossless-check <dir-or-file>...");
        exit(2);
    }
    let roots: Vec<PathBuf> = args.iter().map(PathBuf::from).collect();

    let mut files = Vec::new();
    let mut traversal_errors = 0usize;
    for r in &roots {
        traversal_errors += collect(r, &mut files);
    }

    let mut ok = 0usize;
    let mut diff = 0usize;
    let mut err = traversal_errors;
    for f in &files {
        let src = match std::fs::read_to_string(f) {
            Ok(s) => s,
            Err(e) => {
                println!("READ-ERR {}: {}", f.display(), e);
                err += 1;
                continue;
            }
        };
        let source_tokens = SourceTokens::lex(&src);
        match render_lossless(&src, source_tokens.tokens(), source_tokens.trivia()) {
            Ok(rendered) if rendered == src => ok += 1,
            Ok(_) => {
                println!("DIFF {}", f.display());
                diff += 1;
            }
            Err(msg) => {
                println!("ERR  {}: {}", f.display(), msg);
                err += 1;
            }
        }
    }

    println!(
        "\nlossless: {} ok, {} diff, {} err  ({} files)",
        ok,
        diff,
        err,
        files.len()
    );
    if diff + err > 0 {
        exit(1);
    }
}
