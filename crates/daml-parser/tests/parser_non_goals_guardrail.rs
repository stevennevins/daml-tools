//! Guardrail: daml-parser stays source-oriented and avoids LF-only constructs.

use std::fs;
use std::path::Path;

const FORBIDDEN_IDENTIFIERS: &[&str] =
    &["NameMap", "FeatureFlags", "EUpdate", "UCreate", "UExercise"];

/// `PackageId` may appear only in comments/docs that explain non-goals.
const PACKAGE_ID: &str = "PackageId";

fn strip_line_comment(line: &str) -> &str {
    line.split("//").next().unwrap_or(line)
}

fn is_doc_comment_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("///") || trimmed.starts_with("//!")
}

#[test]
fn parser_source_avoids_lf_only_identifiers() {
    let src_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let mut violations = Vec::new();

    for entry in fs::read_dir(&src_dir).expect("read parser src dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }

        let contents = fs::read_to_string(&path).expect("read source file");
        for (line_no, line) in contents.lines().enumerate() {
            if is_doc_comment_line(line) {
                continue;
            }

            let code = strip_line_comment(line);
            for ident in FORBIDDEN_IDENTIFIERS {
                if code.contains(ident) {
                    violations.push(format!(
                        "{}:{}: forbidden LF identifier `{ident}`",
                        path.display(),
                        line_no + 1
                    ));
                }
            }

            if code.contains(PACKAGE_ID) {
                violations.push(format!(
                    "{}:{}: `PackageId` outside doc comment (allowed only in non-goal docs)",
                    path.display(),
                    line_no + 1
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "daml-parser must not model LF-only constructs:\n{}",
        violations.join("\n")
    );
}
