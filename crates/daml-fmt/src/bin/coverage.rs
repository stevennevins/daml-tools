//! Coverage metric for the AST formatter (replaces the retired `score` bin,
//! which measured byte-match to `expected/` — no longer the target).
//!
//! Reports how many AST blocks our rules canonically lay out vs pass through
//! verbatim. Currently the modeled construct is `do`-blocks, so coverage =
//! do-blocks reindented / total do-blocks across the corpus. Higher = more of
//! the corpus is laid out by our own rules. (On the already-canonical corpus
//! most blocks are no-ops, so the count of *reindented* blocks is naturally
//! small; the metric tracks rising modeled-construct coverage as rules land.)
//!
//! Usage: coverage [--list]

use daml_fmt::layout_ast::coverage;
use std::path::{Path, PathBuf};

fn collect(dir: &Path, out: &mut Vec<PathBuf>) {
    let mut entries: Vec<_> = std::fs::read_dir(dir)
        .unwrap_or_else(|e| panic!("read_dir {}: {}", dir.display(), e))
        .map(|e| e.unwrap().path())
        .collect();
    entries.sort();
    for e in entries {
        if e.is_dir() {
            collect(&e, out);
        } else if e.extension().is_some_and(|x| x == "daml") {
            out.push(e);
        }
    }
}

fn main() {
    let list = std::env::args().any(|a| a == "--list");
    let mut originals = Vec::new();
    collect(Path::new("original"), &mut originals);

    let (mut reindented, mut total_blocks, mut files_with_edits) = (0usize, 0usize, 0usize);
    for o in &originals {
        let Ok(src) = std::fs::read_to_string(o) else {
            continue;
        };
        let (r, t) = coverage(&src);
        reindented += r;
        total_blocks += t;
        if r > 0 {
            files_with_edits += 1;
            if list {
                println!("REINDENT {} ({} do-block(s))", o.display(), r);
            }
        }
    }
    println!(
        "\ndo-blocks: {} reindented / {} total across {} files ({} files reindented)",
        reindented,
        total_blocks,
        originals.len(),
        files_with_edits
    );
}
