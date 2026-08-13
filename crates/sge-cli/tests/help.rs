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
