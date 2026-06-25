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

fn collect(path: &Path, out: &mut Vec<PathBuf>) -> usize {
    if path.is_dir() {
        let entries = match std::fs::read_dir(path) {
            Ok(entries) => entries,
            Err(e) => {
                eprintln!("READ-ERR {}: {}", path.display(), e);
                return 1;
            }
        };
        let mut entry_errors = 0usize;
        let mut entries: Vec<_> = entries
            .filter_map(|entry| match entry {
                Ok(entry) => Some(entry.path()),
                Err(e) => {
                    eprintln!("READ-ERR {}: {}", path.display(), e);
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
    let mut traversal_errors = 0usize;
    for root in &roots {
        traversal_errors += collect(root, &mut originals);
    }
    if originals.is_empty() {
        eprintln!("coverage: no .daml files found under supplied path(s)");
        exit(1);
    }

    let (mut candidates, mut modeled, mut files_with_candidates, mut errors) =
        (0usize, 0usize, 0usize, 0usize);
    errors += traversal_errors;
    for o in &originals {
        let src = match std::fs::read_to_string(o) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("READ-ERR {}: {}", o.display(), e);
                errors += 1;
                continue;
            }
        };
        let coverage = match coverage(&src) {
            Ok(coverage) => coverage,
            Err(e) => {
                eprintln!("DIAG-ERR {}: {}", o.display(), e);
                errors += 1;
                continue;
            }
        };
        candidates += coverage.edit_candidates();
        modeled += coverage.modeled_constructs();
        if coverage.edit_candidates() > 0 {
            files_with_candidates += 1;
            if list {
                println!(
                    "CANDIDATE {} ({} structural edit(s))",
                    o.display(),
                    coverage.edit_candidates()
                );
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
    if errors > 0 {
        exit(1);
    }
}
