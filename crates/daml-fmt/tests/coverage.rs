#![allow(clippy::unwrap_used)]

use std::process::Command;

fn cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_coverage"))
}

#[test]
fn missing_input_exits_nonzero_with_read_err() {
    let missing = std::env::temp_dir().join(format!(
        "daml-fmt-coverage-missing-{}-{}.daml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let output = cmd().arg(&missing).output().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("READ-ERR"),
        "expected READ-ERR diagnostic, got: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn valid_input_reports_metrics() {
    let path = std::env::temp_dir().join(format!(
        "daml-fmt-coverage-valid-{}-{}.daml",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::write(&path, "module M where\nmain = do\n  pass\n").unwrap();
    let output = cmd().arg(&path).output().unwrap();
    std::fs::remove_file(&path).ok();

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("AST layout:"));
}
