use assert_cmd::Command;

#[test]
fn cli_help_exits_successfully() {
    let mut cmd = Command::cargo_bin("latch").unwrap();
    cmd.arg("--help").assert().success();
}

#[test]
fn cli_version_exits_successfully() {
    let mut cmd = Command::cargo_bin("latch").unwrap();
    cmd.arg("--version").assert().success();
}

#[test]
fn cli_help_lists_subcommands() {
    let mut cmd = Command::cargo_bin("latch").unwrap();
    let output = cmd.arg("--help").output().unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();

    // All subcommands from constitution must be present
    assert!(stdout.contains("new"), "missing 'new' subcommand");
    assert!(stdout.contains("attach"), "missing 'attach' subcommand");
    assert!(stdout.contains("detach"), "missing 'detach' subcommand");
    assert!(stdout.contains("list"), "missing 'list' subcommand");
    assert!(stdout.contains("kill"), "missing 'kill' subcommand");
    assert!(stdout.contains("history"), "missing 'history' subcommand");
    assert!(stdout.contains("rename"), "missing 'rename' subcommand");
}

#[test]
fn cli_unknown_subcommand_fails() {
    let mut cmd = Command::cargo_bin("latch").unwrap();
    cmd.arg("nonexistent-command").assert().failure();
}
