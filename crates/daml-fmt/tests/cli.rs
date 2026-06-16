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
fn check_reports_unformatted_file_with_exit_one() {
    let path = temp_file("unformatted.daml", "module M where\nfoo : Int\nfoo = 1\n");
    let output = cmd().arg("--check").arg(&path).output().unwrap();
    std::fs::remove_file(&path).ok();

    assert_eq!(output.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&output.stdout).contains(path.to_str().unwrap()));
}
