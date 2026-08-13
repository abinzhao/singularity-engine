use std::process::Command;

#[test]
fn help_identifies_singularity_engine_cli() {
    let output = Command::new(env!("CARGO_BIN_EXE_sge"))
        .arg("--help")
        .output()
        .expect("failed to run sge --help");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("help output was not UTF-8");
    assert!(stdout.contains("Usage: sge"));
    assert!(stdout.contains("SINGULARITY ENGINE"));
}

#[test]
fn evolve_and_test_commands_expose_isolated_run_options() {
    let evolve = Command::new(env!("CARGO_BIN_EXE_sge"))
        .args(["evolve", "--help"])
        .output()
        .expect("run evolve help");
    assert!(evolve.status.success());
    let evolve_stdout = String::from_utf8(evolve.stdout).expect("evolve help UTF-8");
    for flag in [
        "--approve",
        "--workspace",
        "--provider-fixture",
        "--candidates",
    ] {
        assert!(evolve_stdout.contains(flag), "missing evolve flag {flag}");
    }

    let test = Command::new(env!("CARGO_BIN_EXE_sge"))
        .args(["test", "--help"])
        .output()
        .expect("run test help");
    assert!(test.status.success());
    let test_stdout = String::from_utf8(test.stdout).expect("test help UTF-8");
    for flag in ["--workspace", "--candidate"] {
        assert!(test_stdout.contains(flag), "missing test flag {flag}");
    }
}

#[test]
fn evidence_commands_are_exposed_as_read_only_cli_operations() {
    for command in ["explain", "history", "diff"] {
        let output = Command::new(env!("CARGO_BIN_EXE_sge"))
            .args([command, "--help"])
            .output()
            .expect("run evidence command help");
        assert!(output.status.success(), "{command} help should succeed");
        let stdout = String::from_utf8(output.stdout).expect("help output UTF-8");
        assert!(stdout.contains("--workspace"));
    }

    let test = Command::new(env!("CARGO_BIN_EXE_sge"))
        .args(["test", "--help"])
        .output()
        .expect("run test help");
    assert!(test.status.success());
    assert!(
        String::from_utf8(test.stdout)
            .expect("test help UTF-8")
            .contains("--replay")
    );
}

#[test]
fn apply_and_undo_commands_require_explicit_transaction_inputs() {
    let apply = Command::new(env!("CARGO_BIN_EXE_sge"))
        .args(["apply", "--help"])
        .output()
        .expect("run apply help");
    assert!(apply.status.success());
    let apply_stdout = String::from_utf8(apply.stdout).expect("apply help UTF-8");
    assert!(apply_stdout.contains("--approve"));
    assert!(apply_stdout.contains("--workspace"));

    let undo = Command::new(env!("CARGO_BIN_EXE_sge"))
        .args(["undo", "--help"])
        .output()
        .expect("run undo help");
    assert!(undo.status.success());
    let undo_stdout = String::from_utf8(undo.stdout).expect("undo help UTF-8");
    assert!(undo_stdout.contains("--revision"));
    assert!(undo_stdout.contains("--target"));
    assert!(undo_stdout.contains("--workspace"));
}
