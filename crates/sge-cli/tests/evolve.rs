use std::{fs, path::Path, process::Command};

fn repo_path(relative: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
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
        .arg(repo_path("fixtures/evolution/basic-skill"))
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

#[test]
fn evolve_and_test_commands_run_the_isolated_candidate_flow() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    let standard_source = temp.path().join("skills/code-review/instructions.md");
    let source_before = fs::read(&standard_source).expect("read standard source");

    let evolve = Command::new(env!("CARGO_BIN_EXE_sge"))
        .arg("evolve")
        .arg("skill:code-review")
        .args(["--approve", "prop-sql-injection-guard"])
        .arg("--provider-fixture")
        .arg(repo_path("fixtures/provider/prompt-candidates.json"))
        .arg("--workspace")
        .arg(temp.path())
        .arg("--json")
        .output()
        .expect("run evolve");
    assert!(
        evolve.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&evolve.stderr)
    );
    let body: serde_json::Value =
        serde_json::from_slice(&evolve.stdout).expect("evolve output JSON");
    assert_eq!(body["selected_candidate"], "candidate-2");
    let candidate_path = body["candidates"][1]["worktree_path"]
        .as_str()
        .expect("candidate path");

    for candidate in [None, Some(candidate_path)] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_sge"));
        command
            .arg("test")
            .arg("skill:code-review")
            .arg("--workspace")
            .arg(temp.path())
            .arg("--json");
        if let Some(path) = candidate {
            command.arg("--candidate").arg(path);
        }
        let output = command.output().expect("run test");
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let test_body: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("test output JSON");
        assert_eq!(test_body["ok"], true);
    }

    assert_eq!(
        fs::read(standard_source).expect("read source"),
        source_before
    );
}

#[test]
fn evidence_commands_read_the_persisted_evolution_run() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    let evolve = Command::new(env!("CARGO_BIN_EXE_sge"))
        .arg("evolve")
        .arg("skill:code-review")
        .args(["--approve", "prop-sql-injection-guard"])
        .arg("--provider-fixture")
        .arg(repo_path("fixtures/provider/prompt-candidates.json"))
        .arg("--workspace")
        .arg(temp.path())
        .arg("--json")
        .output()
        .expect("run evolve");
    assert!(evolve.status.success());
    let body: serde_json::Value =
        serde_json::from_slice(&evolve.stdout).expect("evolve output JSON");
    let run_id = body["run_id"].as_str().expect("run id");
    let replay: serde_yaml::Value = serde_yaml::from_str(
        &fs::read_to_string(
            temp.path()
                .join(".singularity/runs")
                .join(run_id)
                .join("replay.yaml"),
        )
        .expect("read replay"),
    )
    .expect("parse replay");
    let baseline_revision = replay["baseline_revision"]
        .as_str()
        .expect("baseline revision");
    let selected_revision = replay["selected_revision"]
        .as_str()
        .expect("selected revision");

    let commands = [
        vec![
            "explain",
            run_id,
            "--workspace",
            temp.path().to_str().unwrap(),
            "--json",
        ],
        vec![
            "history",
            "skill:code-review",
            "--workspace",
            temp.path().to_str().unwrap(),
            "--json",
        ],
        vec![
            "diff",
            baseline_revision,
            selected_revision,
            "--workspace",
            temp.path().to_str().unwrap(),
            "--json",
        ],
        vec![
            "test",
            "--replay",
            run_id,
            "--workspace",
            temp.path().to_str().unwrap(),
            "--json",
        ],
    ];
    for args in commands {
        let output = Command::new(env!("CARGO_BIN_EXE_sge"))
            .args(args)
            .output()
            .expect("run evidence command");
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let result: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("evidence command JSON");
        assert_eq!(result["ok"], true);
    }
}

#[test]
fn apply_and_undo_commands_switch_and_restore_the_standard_skill() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    let standard = temp.path().join("skills/code-review");
    let before = directory_bytes(&standard);
    let evolve = Command::new(env!("CARGO_BIN_EXE_sge"))
        .arg("evolve")
        .arg("skill:code-review")
        .args(["--approve", "prop-sql-injection-guard"])
        .arg("--provider-fixture")
        .arg(repo_path("fixtures/provider/prompt-candidates.json"))
        .arg("--workspace")
        .arg(temp.path())
        .arg("--json")
        .output()
        .expect("run evolve");
    assert!(evolve.status.success());
    let body: serde_json::Value =
        serde_json::from_slice(&evolve.stdout).expect("evolve output JSON");
    let run_id = body["run_id"].as_str().expect("run id");

    let apply = Command::new(env!("CARGO_BIN_EXE_sge"))
        .arg("apply")
        .arg(run_id)
        .arg("--approve")
        .arg("--workspace")
        .arg(temp.path())
        .arg("--json")
        .output()
        .expect("run apply");
    assert!(
        apply.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&apply.stderr)
    );
    assert_ne!(directory_bytes(&standard), before);

    let undo = Command::new(env!("CARGO_BIN_EXE_sge"))
        .arg("undo")
        .arg(run_id)
        .arg("--workspace")
        .arg(temp.path())
        .arg("--json")
        .output()
        .expect("run undo");
    assert!(
        undo.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&undo.stderr)
    );
    assert_eq!(directory_bytes(&standard), before);
}

fn directory_bytes(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = walkdir::WalkDir::new(root)
        .into_iter()
        .map(|entry| entry.expect("walk directory"))
        .filter(|entry| entry.file_type().is_file())
        .map(|entry| {
            (
                entry
                    .path()
                    .strip_prefix(root)
                    .expect("relative path")
                    .display()
                    .to_string(),
                fs::read(entry.path()).expect("read file"),
            )
        })
        .collect::<Vec<_>>();
    files.sort_by(|a, b| a.0.cmp(&b.0));
    files
}
