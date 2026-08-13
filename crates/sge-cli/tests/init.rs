use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_workspace() -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("sge-cli-init-{nonce}"))
}

#[test]
fn init_json_creates_workspace_without_business_git() {
    let root = temp_workspace();

    let output = Command::new(env!("CARGO_BIN_EXE_sge"))
        .arg("init")
        .arg(&root)
        .arg("--json")
        .output()
        .expect("failed to run sge init");

    assert!(output.status.success());

    let stdout = String::from_utf8(output.stdout).expect("init output was not UTF-8");
    let body: serde_json::Value = serde_json::from_str(&stdout).expect("init output was not JSON");
    assert_eq!(body["ok"], true);
    assert_eq!(body["code"], "OK");
    assert!(
        body["root"]
            .as_str()
            .expect("root should be a string")
            .ends_with(
                root.file_name()
                    .expect("temp workspace should have a file name")
                    .to_str()
                    .expect("temp workspace file name should be UTF-8")
            )
    );

    assert!(root.join(".singularity/repo.git").is_dir());
    assert!(!root.join(".git").exists());

    fs::remove_dir_all(root).expect("failed to clean up temp workspace");
}
