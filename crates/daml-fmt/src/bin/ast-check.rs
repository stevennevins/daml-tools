//! Verify daml-parser's AST-level losslessness oracle over the corpus.
//!
//! Parses every file with `parse_module` and runs `render_from_ast`, asserting
//! the byte spans reconstruct the source byte-identical (and that the V3
//! coverage check finds no dropped tokens). This is the gate for the
//! structural formatter: if it passes 924/924, the AST is faithful enough to
//! format from.
//!
//! Usage: `ast-check <dir-or-file>...`

use daml_parser::ast_span::render_from_ast;
use daml_syntax::SourceFile;
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
        eprintln!("usage: ast-check <dir-or-file>...");
        exit(2);
    }
    let roots: Vec<PathBuf> = args.iter().map(PathBuf::from).collect();

    let mut files = Vec::new();
    let mut traversal_errors = 0usize;
    for r in &roots {
        traversal_errors += collect(r, &mut files);
    }

    let mut ok = 0usize;
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
        let source_file = SourceFile::parse(&src);
        match render_from_ast(&src, source_file.module(), source_file.trivia()) {
            Ok(rendered) if rendered == src => ok += 1,
            Ok(_) => {
                println!("DIFF {}", f.display());
                err += 1;
            }
            Err(msg) => {
                println!("ERR  {}: {}", f.display(), msg);
                err += 1;
            }
        }
    }

    println!(
        "\nast-lossless: {} ok, {} err  ({} files)",
        ok,
        err,
        files.len()
    );
    if err > 0 {
        exit(1);
    }
}
