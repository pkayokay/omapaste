use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_omapaste"))
}

#[test]
fn version_flag() {
    let out = bin().arg("--version").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("omapaste"));
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_flag() {
    let out = bin().arg("-h").output().unwrap();
    assert!(out.status.success());
    let stdout = String::from_utf8_lossy(&out.stdout);
    assert!(stdout.contains("Usage: omapaste"));
    assert!(stdout.contains("daemon"));
}

#[test]
fn unknown_command_exits_2() {
    let out = bin().arg("launch").output().unwrap();
    assert_eq!(out.status.code(), Some(2));
    let err = String::from_utf8_lossy(&out.stderr);
    assert!(err.contains("launch"));
}
