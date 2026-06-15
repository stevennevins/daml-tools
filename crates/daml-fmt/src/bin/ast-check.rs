//! Verify daml-parser's AST-level losslessness oracle over the corpus.
//!
//! Parses every file with `parse_module` and runs `render_from_ast`, asserting
//! the byte spans reconstruct the source byte-identical (and that the V3
//! coverage check finds no dropped tokens). This is the gate for the
//! structural formatter: if it passes 924/924, the AST is faithful enough to
//! format from.
//!
//! Usage: ast-check <dir-or-file>...   (default: original/)

use daml_parser::ast_span::render_from_ast;
use daml_parser::lexer::lex_with_trivia;
use daml_parser::parse::parse_module;
use std::path::{Path, PathBuf};
use std::process::exit;

fn collect(path: &Path, out: &mut Vec<PathBuf>) {
    if path.is_dir() {
        let mut entries: Vec<_> = std::fs::read_dir(path)
            .unwrap_or_else(|e| panic!("read_dir {}: {}", path.display(), e))
            .map(|e| e.unwrap().path())
            .collect();
        entries.sort();
        for e in entries {
            collect(&e, out);
        }
    } else if path.extension().is_some_and(|x| x == "daml") {
        out.push(path.to_path_buf());
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let roots: Vec<PathBuf> = if args.is_empty() {
        vec![PathBuf::from("original")]
    } else {
        args.iter().map(PathBuf::from).collect()
    };

    let mut files = Vec::new();
    for r in &roots {
        collect(r, &mut files);
    }

    let mut ok = 0usize;
    let mut err = 0usize;
    for f in &files {
        let src = match std::fs::read_to_string(f) {
            Ok(s) => s,
            Err(e) => {
                println!("READ-ERR {}: {}", f.display(), e);
                err += 1;
                continue;
            }
        };
        let (module, _diags) = parse_module(&src);
        let (_tokens, trivia, _lex_errors) = lex_with_trivia(&src);
        match render_from_ast(&src, &module, &trivia) {
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
