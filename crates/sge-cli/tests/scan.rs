use std::{fs, path::Path, process::Command};

fn fixture_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/evolution/basic-skill")
}

fn prepare_workspace(root: &Path) {
    let init = Command::new(env!("CARGO_BIN_EXE_sge"))
        .args(["init"])
        .arg(root)
        .arg("--json")
        .output()
        .expect("run init");
    assert!(init.status.success());

    let import = Command::new(env!("CARGO_BIN_EXE_sge"))
        .arg("import")
        .arg(fixture_path())
        .arg("--workspace")
        .arg(root)
        .arg("--json")
        .output()
        .expect("run import");
    assert!(import.status.success());

    fs::create_dir_all(root.join("evals/results")).expect("create eval results");
    for index in 1..=2 {
        fs::write(
            root.join(format!("evals/results/sql-{index}.yaml")),
            format!(
                "schema: sge.dev/evidence/v1\n\
                 id: eval:sql-{index}\n\
                 target: skill:code-review\n\
                 claim: Missed SQL injection in concatenated query\n\
                 source: evaluation\n\
                 status: confirmed\n\
                 details: {{}}\n"
            ),
        )
        .expect("write evidence");
    }
}

fn run_scan(root: &Path, extra: &[&str]) -> std::process::Output {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sge"));
    command
        .arg("scan")
        .arg("skill:code-review")
        .arg("--workspace")
        .arg(root)
        .arg("--json")
        .args(extra)
        .output()
        .expect("run scan")
}

#[test]
fn scan_json_returns_grounded_proposals_path() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());

    let output = run_scan(temp.path(), &[]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("scan output should be JSON");
    assert_eq!(body["ok"], true);
    assert_eq!(body["code"], "OK");
    assert_eq!(body["target"], "skill:code-review");
    assert!(body["run_id"].as_str().is_some_and(|id| !id.is_empty()));
    assert!(
        body["proposals"]
            .as_array()
            .is_some_and(|items| !items.is_empty())
    );
    let proposals_path = body["proposals_path"]
        .as_str()
        .expect("proposals_path string");
    assert!(Path::new(proposals_path).is_file());
    assert!(body["contract_path"].is_null());
}

#[test]
fn explicit_approve_json_writes_contract() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());

    let output = run_scan(temp.path(), &["--approve", "prop-sql-injection-guard"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("approval output should be JSON");
    assert_eq!(body["ok"], true);
    let contract_path = body["contract_path"]
        .as_str()
        .expect("contract_path string");
    assert!(Path::new(contract_path).is_file());
}

#[test]
fn explicit_goal_json_does_not_require_existing_evidence() {
    let temp = tempfile::tempdir().expect("create tempdir");
    let init = Command::new(env!("CARGO_BIN_EXE_sge"))
        .args(["init"])
        .arg(temp.path())
        .arg("--json")
        .output()
        .expect("run init");
    assert!(init.status.success());
    let import = Command::new(env!("CARGO_BIN_EXE_sge"))
        .arg("import")
        .arg(fixture_path())
        .arg("--workspace")
        .arg(temp.path())
        .arg("--json")
        .output()
        .expect("run import");
    assert!(import.status.success());

    let output = run_scan(temp.path(), &["--goal", "Eliminate unsafe SQL approvals"]);
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("goal output should be JSON");
    assert_eq!(body["ok"], true);
    assert_eq!(body["proposals"], serde_json::json!([]));
    let contract_path = body["contract_path"]
        .as_str()
        .expect("contract_path string");
    assert!(Path::new(contract_path).is_file());
}
