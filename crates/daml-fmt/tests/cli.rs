#![allow(clippy::unwrap_used)]

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

fn cmd() -> Command {
    Command::new(env!("CARGO_BIN_EXE_daml-fmt"))
}

fn temp_file(name: &str, contents: &str) -> std::path::PathBuf {
    let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "daml-fmt-cli-{}-{}-{}",
        std::process::id(),
        id,
        name
    ));
    std::fs::write(&path, contents).unwrap();
    path
}

#[test]
fn help_exits_successfully() {
    let output = cmd().arg("--help").output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage: daml-fmt"));
}

#[test]
fn stdin_formats_source_to_stdout() {
    let mut child = cmd()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"module M where\nfoo : Int\nfoo = 1\n")
        .unwrap();

    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "module M where\nfoo: Int\nfoo = 1\n"
    );
}

#[test]
fn file_arguments_rewrite_in_place_by_default() {
    let path = temp_file("default-write.daml", "module M where\nfoo : Int\nfoo = 1\n");
    let output = cmd().arg(&path).output().unwrap();
    let formatted = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).ok();

    assert!(output.status.success());
    assert_eq!(formatted, "module M where\nfoo: Int\nfoo = 1\n");
    assert!(output.stdout.is_empty());
}

#[test]
fn check_reports_unformatted_file_with_exit_one() {
    let path = temp_file("unformatted.daml", "module M where\nfoo : Int\nfoo = 1\n");
    let output = cmd().arg("--check").arg(&path).output().unwrap();
    std::fs::remove_file(&path).ok();

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains(path.to_str().unwrap()));
}

#[test]
fn preserve_import_order_disables_import_organization() {
    let input = "module M where\n\nimport DA.Optional\nimport Daml.Script\nimport DA.List\n\nx: Optional Text\nx = Some \"ok\"\n";
    let output = cmd()
        .arg("--preserve-import-order")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.as_mut().unwrap().write_all(input.as_bytes())?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(output.status.success());
    assert_eq!(String::from_utf8_lossy(&output.stdout), input);
}

#[test]
fn check_without_files_exits_two() {
    let output = cmd()
        .arg("--check")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|child| child.wait_with_output())
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("need file arguments"));
}

#[test]
fn write_without_files_exits_two() {
    let output = cmd()
        .arg("--write")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .and_then(|child| child.wait_with_output())
        .unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("need file arguments"));
}

#[test]
fn stdin_reports_parser_diagnostics_and_exits_two() {
    let input = "module M where\nfoo = if x then 1\n";
    let mut child = cmd()
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();

    let output = child.wait_with_output().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(String::from_utf8_lossy(&output.stdout), input);
    assert!(String::from_utf8_lossy(&output.stderr).contains("daml-fmt: <stdin>"));
}

#[test]
fn check_reports_parser_diagnostics_and_exits_two() {
    let path = temp_file(
        "parser-diagnostic-check.daml",
        "module M where\nfoo = if x then 1\n",
    );
    let output = cmd().arg("--check").arg(&path).output().unwrap();
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).ok();

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(&format!("daml-fmt: {}:", path.display()))
    );
    assert_eq!(source, "module M where\nfoo = if x then 1\n");
}

#[test]
fn write_reports_parser_diagnostics_and_does_not_modify_input() {
    let path = temp_file(
        "parser-diagnostic-write.daml",
        "module M where\nfoo = if x then 1\n",
    );
    let output = cmd().arg("--write").arg(&path).output().unwrap();
    let source = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).ok();

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(&format!("daml-fmt: {}:", path.display()))
    );
    assert_eq!(source, "module M where\nfoo = if x then 1\n");
}

#[test]
fn rule_gap_normalization_only_changes_spacing() {
    let path = temp_file("gap-only.daml", "module M where\nfoo : Int\nfoo = 1\n");
    let output = cmd()
        .arg("--rule")
        .arg("gap-normalization")
        .arg(&path)
        .output()
        .unwrap();
    let formatted = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).ok();

    assert!(output.status.success());
    assert_eq!(formatted, "module M where\nfoo: Int\nfoo = 1\n");
}

#[test]
fn rule_import_order_only_reorders_imports() {
    let input =
        "module M where\n\nimport DA.Optional\nimport Daml.Script\nimport DA.List\n\nx = []\n";
    let want =
        "module M where\n\nimport Daml.Script\n\nimport DA.List\nimport DA.Optional\n\nx = []\n";
    let path = temp_file("import-only.daml", input);
    let output = cmd()
        .arg("--rule")
        .arg("import-order")
        .arg(&path)
        .output()
        .unwrap();
    let formatted = std::fs::read_to_string(&path).unwrap();
    std::fs::remove_file(&path).ok();

    assert!(output.status.success());
    assert_eq!(formatted, want);
}

#[test]
fn preserve_import_order_conflicts_with_explicit_import_order_rule() {
    let path = temp_file("import-conflict.daml", "module M where\nx = 1\n");
    let output = cmd()
        .arg("--preserve-import-order")
        .arg("--rule")
        .arg("import-order")
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();

    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("import-order")
            && String::from_utf8_lossy(&output.stderr).contains("preserve-import-order")
    );
}

#[test]
fn unknown_rule_exits_two() {
    let path = temp_file("unknown-rule.daml", "module M where\nx = 1\n");
    let output = cmd()
        .arg("--rule")
        .arg("not-a-rule")
        .arg(&path)
        .output()
        .unwrap();
    std::fs::remove_file(&path).ok();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown rule 'not-a-rule'"));
}

#[test]
fn config_rules_load_from_daml_yaml() {
    let dir = std::env::temp_dir().join(format!(
        "daml-fmt-config-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("daml.yaml"),
        "daml-tools:\n  fmt:\n    rules: [gap-normalization]\n",
    )
    .unwrap();
    let path = dir.join("spacing.daml");
    std::fs::write(&path, "module M where\nfoo : Int\nfoo = 1\n").unwrap();

    let output = cmd()
        .current_dir(&dir)
        .arg(path.file_name().unwrap())
        .output()
        .unwrap();
    let formatted = std::fs::read_to_string(&path).unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    assert!(output.status.success());
    assert_eq!(formatted, "module M where\nfoo: Int\nfoo = 1\n");
}

#[test]
fn explicit_config_must_include_fmt_section() {
    let dir = std::env::temp_dir().join(format!(
        "daml-fmt-config-missing-{}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let config = dir.join("config.yaml");
    std::fs::write(&config, "daml-tools:\n  lint:\n    groups: [recommended]\n").unwrap();
    let path = dir.join("file.daml");
    std::fs::write(&path, "module M where\nx = 1\n").unwrap();

    let output = cmd()
        .arg("--config")
        .arg(&config)
        .arg(&path)
        .output()
        .unwrap();
    let _ = std::fs::remove_dir_all(&dir);

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("missing daml-tools.fmt section"));
}
