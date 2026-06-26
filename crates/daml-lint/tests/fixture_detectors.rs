#![allow(clippy::unwrap_used)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

const REQUIRED_FIXTURES: [&str; 2] = ["good_patterns.daml", "bad_patterns.daml"];
const EXPECTED_BAD_DETECTORS: [&str; 6] = [
    "missing-ensure-decimal",
    "unguarded-division",
    "unbounded-fields",
    "missing-positive-amount",
    "archive-before-execute",
    "head-of-list-query",
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
    let missing: Vec<_> = REQUIRED_FIXTURES
        .iter()
        .filter(|name| !root.join(name).is_file())
        .copied()
        .collect();

    if missing.is_empty() {
        return Some(root);
    }

    assert!(
        std::env::var_os("CI").is_none(),
        "daml-lint shipped fixture(s) missing under CI from {}: {}",
        root.display(),
        missing.join(", ")
    );
    eprintln!(
        "skipping daml-lint shipped fixture detector smoke; missing from {}: {}",
        root.display(),
        missing.join(", ")
    );
    None
}

fn lint_fixture(root: &Path, name: &str) -> Output {
    cmd()
        .arg(root.join(name))
        .arg("--fail-on")
        .arg("info")
        .output()
        .unwrap()
}

#[test]
fn good_fixture_has_no_builtin_detector_findings() {
    let Some(root) = fixtures_present_or_skip() else {
        return;
    };

    let output = lint_fixture(&root, "good_patterns.daml");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert!(
        output.status.success(),
        "good fixture should lint cleanly\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(stdout.contains("No findings."), "stdout was:\n{stdout}");
    for detector in EXPECTED_BAD_DETECTORS {
        assert!(
            !stdout.contains(detector),
            "good fixture should not report {detector}\nstdout:\n{stdout}"
        );
    }
}

#[test]
fn bad_fixture_exercises_expected_builtin_detectors_without_parse_errors() {
    let Some(root) = fixtures_present_or_skip() else {
        return;
    };

    let output = lint_fixture(&root, "bad_patterns.daml");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    assert_eq!(
        output.status.code(),
        Some(1),
        "bad fixture should produce findings without parser errors\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    assert!(
        !stdout.contains("Parse Errors") && !stderr.contains("parse ["),
        "bad fixture must stay parser-clean\nstdout:\n{stdout}\nstderr:\n{stderr}"
    );
    for detector in EXPECTED_BAD_DETECTORS {
        assert!(
            stdout.contains(detector),
            "bad fixture should exercise {detector}\nstdout:\n{stdout}"
        );
    }
}
