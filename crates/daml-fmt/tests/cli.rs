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

fn temp_dir(name: &str) -> std::path::PathBuf {
    let id = NEXT_TEMP.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "daml-fmt-cli-{}-{}-{}",
        std::process::id(),
        id,
        name
    ));
    std::fs::create_dir_all(&path).unwrap();
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
fn rule_flag_runs_only_selected_formatter_rule() {
    let input = "module M where\n\nimport DA.Optional\nimport DA.List\n\nfoo : Int\nfoo = 1\n";
    let output = cmd()
        .arg("--rule")
        .arg("imports")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.as_mut().unwrap().write_all(input.as_bytes())?;
            child.wait_with_output()
        })
        .unwrap();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "module M where\n\nimport DA.List\nimport DA.Optional\n\nfoo : Int\nfoo = 1\n"
    );
}

#[test]
fn daml_yaml_can_disable_formatter_rule() {
    let project = temp_dir("config-project");
    std::fs::write(
        project.join("daml.yaml"),
        r#"daml-tools:
  fmt:
    groups: [all]
    rules:
      imports: off
"#,
    )
    .unwrap();
    let input = "module M where\n\nimport DA.Optional\nimport DA.List\n\nfoo : Int\nfoo = 1\n";

    let output = cmd()
        .current_dir(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.as_mut().unwrap().write_all(input.as_bytes())?;
            child.wait_with_output()
        })
        .unwrap();
    std::fs::remove_dir_all(&project).ok();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "module M where\n\nimport DA.Optional\nimport DA.List\n\nfoo: Int\nfoo = 1\n"
    );
}

#[test]
fn cli_rule_selection_overrides_daml_yaml_selection() {
    let project = temp_dir("cli-over-config-project");
    std::fs::write(
        project.join("daml.yaml"),
        r#"daml-tools:
  fmt:
    rules:
      imports: off
"#,
    )
    .unwrap();
    let input = "module M where\n\nimport DA.Optional\nimport DA.List\n\nfoo : Int\nfoo = 1\n";

    let output = cmd()
        .current_dir(&project)
        .arg("--rule")
        .arg("imports")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .and_then(|mut child| {
            child.stdin.as_mut().unwrap().write_all(input.as_bytes())?;
            child.wait_with_output()
        })
        .unwrap();
    std::fs::remove_dir_all(&project).ok();

    assert!(output.status.success());
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "module M where\n\nimport DA.List\nimport DA.Optional\n\nfoo : Int\nfoo = 1\n"
    );
}

#[test]
fn unknown_formatter_rule_exits_two() {
    let output = cmd().arg("--rule").arg("unknown-rule").output().unwrap();

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unknown-rule"));
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
