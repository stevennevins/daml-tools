//! Structural candidate metric for the AST formatter (replaces the retired
//! `score` bin, which measured byte-match to `expected/` — no longer the
//! target).
//!
//! Reports structural edit candidates over modeled constructs for the currently
//! modeled construct families (do, if, case, let-in, constructor `with`, and
//! template/interface bodies). This is not a percentage: one construct can
//! produce multiple edits, and on the already-canonical corpus most modeled
//! constructs are no-ops.
//!
//! Usage: `coverage [--list] <dir-or-file>...`

use daml_fmt::coverage;
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
    let mut list = false;
    let mut roots = Vec::new();
    for arg in std::env::args().skip(1) {
        if arg == "--list" {
            list = true;
        } else {
            roots.push(PathBuf::from(arg));
        }
    }
    if roots.is_empty() {
        eprintln!("usage: coverage [--list] <dir-or-file>...");
        exit(2);
    }

    let mut originals = Vec::new();
    for root in &roots {
        collect(root, &mut originals);
    }

    let (mut candidates, mut modeled, mut files_with_candidates) = (0usize, 0usize, 0usize);
    for o in &originals {
        let Ok(src) = std::fs::read_to_string(o) else {
            continue;
        };
        let (r, t) = coverage(&src);
        candidates += r;
        modeled += t;
        if r > 0 {
            files_with_candidates += 1;
            if list {
                println!("CANDIDATE {} ({} structural edit(s))", o.display(), r);
            }
        }
    }
    println!(
        "\nAST layout: {} structural edit candidate(s) / {} modeled construct(s) across {} files ({} files with candidates)",
        candidates,
        modeled,
        originals.len(),
        files_with_candidates
    );
}
