#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

struct GoldenCase {
    name: &'static str,
    fixture: &'static str,
    expected_stdout: &'static str,
    expected_code: i32,
}

const CASES: &[GoldenCase] = &[
    GoldenCase {
        name: "good fixture markdown report",
        fixture: "good_patterns.daml",
        expected_stdout: "good_patterns.markdown.stdout",
        expected_code: 0,
    },
    GoldenCase {
        name: "bad fixture markdown report",
        fixture: "bad_patterns.daml",
        expected_stdout: "bad_patterns.markdown.stdout",
        expected_code: 1,
    },
];

fn cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_daml-lint"))
}

fn fixture_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
}

fn fixtures_present_or_skip() -> Option<PathBuf> {
    let root = fixture_root();
    let missing: Vec<String> = CASES
        .iter()
        .flat_map(|case| {
            [
                root.join(case.fixture),
                root.join("cli_golden").join(case.expected_stdout),
            ]
        })
        .filter(|path| !path.is_file())
        .map(|path| {
            path.strip_prefix(&root)
                .unwrap_or(&path)
                .display()
                .to_string()
        })
        .collect();

    if missing.is_empty() {
        return Some(root);
    }

    assert!(
        std::env::var_os("CI").is_none(),
        "daml-lint CLI golden fixture(s) missing under CI from {}: {}",
        root.display(),
        missing.join(", ")
    );
    eprintln!(
        "skipping daml-lint CLI golden smoke; missing from {}: {}",
        root.display(),
        missing.join(", ")
    );
    None
}

fn lint_fixture(manifest_dir: &Path, fixture: &str) -> Output {
    cmd()
        .current_dir(manifest_dir)
        .arg(Path::new("tests").join("fixtures").join(fixture))
        .arg("--fail-on")
        .arg("info")
        .output()
        .unwrap()
}

#[test]
fn fixture_markdown_cli_reports_match_goldens() {
    let Some(root) = fixtures_present_or_skip() else {
        return;
    };
    let manifest_dir = root.parent().unwrap().parent().unwrap();

    for case in CASES {
        let output = lint_fixture(manifest_dir, case.fixture);
        let actual_stdout = String::from_utf8_lossy(&output.stdout);
        let actual_stderr = String::from_utf8_lossy(&output.stderr);
        let expected_stdout =
            std::fs::read_to_string(root.join("cli_golden").join(case.expected_stdout)).unwrap();

        assert_eq!(
            output.status.code(),
            Some(case.expected_code),
            "{} should preserve its documented CLI exit code\nstdout:\n{actual_stdout}\nstderr:\n{actual_stderr}",
            case.name
        );
        assert_eq!(
            actual_stderr, "daml-lint: scanning 1 file(s)...\n",
            "{} should keep progress output on stderr and report output on stdout",
            case.name
        );
        assert_eq!(
            actual_stdout, expected_stdout,
            "{} should match the committed markdown CLI golden",
            case.name
        );
    }
}
