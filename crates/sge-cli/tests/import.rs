use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_workspace() -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("sge-cli-import-{nonce}"))
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("fixtures")
        .join("evolution")
        .join("basic-skill")
}

#[test]
fn import_json_imports_skill() {
    let root = temp_workspace();
    let fixture = fixture_path();

    let init_output = Command::new(env!("CARGO_BIN_EXE_sge"))
        .arg("init")
        .arg(&root)
        .arg("--json")
        .output()
        .expect("failed to run sge init");
    assert!(init_output.status.success(), "sge init should succeed");

    let output = Command::new(env!("CARGO_BIN_EXE_sge"))
        .arg("import")
        .arg(&fixture)
        .arg("--workspace")
        .arg(&root)
        .arg("--json")
        .output()
        .expect("failed to run sge import");

    assert!(
        output.status.success(),
        "sge import should succeed, stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8(output.stdout).expect("import output was not UTF-8");
    let body: serde_json::Value =
        serde_json::from_str(&stdout).expect("import output was not JSON");
    assert_eq!(body["ok"], true);
    assert_eq!(body["code"], "OK");
    assert_eq!(body["target"], "skill:code-review");
    assert!(
        body["revision"]
            .as_str()
            .expect("revision should be string")
            .len()
            >= 4
    );

    assert!(
        root.join("skills")
            .join("code-review")
            .join("skill.yaml")
            .is_file()
    );

    fs::remove_dir_all(root).expect("failed to clean up temp workspace");
}
