use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use sge_app::{init, validate};

fn temp_workspace(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("sge-app-{name}-{nonce}"))
}

fn assert_dir(root: &Path, relative: &str) {
    assert!(
        root.join(relative).is_dir(),
        "expected directory {relative} to exist"
    );
}

#[test]
fn init_creates_manifest_internal_repo_and_workspace_paths_without_business_git() {
    let root = temp_workspace("creates");

    init::initialize(&root).expect("workspace initialization should succeed");

    assert!(root.join("singularity.yaml").is_file());
    assert_dir(&root, "agents");
    assert_dir(&root, "skills");
    assert_dir(&root, "rules");
    assert_dir(&root, "memory/facts");
    assert_dir(&root, "memory/preferences");
    assert_dir(&root, "memory/failures");
    assert_dir(&root, "evals/datasets");
    assert_dir(&root, "evals/graders");
    assert_dir(&root, "evals/suites");
    assert_dir(&root, ".singularity/worktrees");
    assert_dir(&root, ".singularity/runs");
    assert_dir(&root, ".singularity/cache");
    assert_dir(&root, ".singularity/installs");
    assert!(root.join(".singularity/repo.git/HEAD").is_file());
    assert!(!root.join(".git").exists());

    validate::validate_workspace(&root).expect("created workspace should validate");

    fs::remove_dir_all(root).expect("failed to clean up temp workspace");
}

#[test]
fn init_is_idempotent_for_generated_workspace() {
    let root = temp_workspace("idempotent");

    init::initialize(&root).expect("first initialization should succeed");
    init::initialize(&root).expect("second initialization should succeed");

    validate::validate_workspace(&root).expect("workspace should remain valid");

    fs::remove_dir_all(root).expect("failed to clean up temp workspace");
}

#[test]
fn init_rejects_existing_non_generated_manifest_with_stable_error_code() {
    let root = temp_workspace("existing-manifest");
    fs::create_dir_all(&root).expect("failed to create temp workspace");
    fs::write(root.join("singularity.yaml"), "name: owned-by-user\n")
        .expect("failed to write existing manifest");

    let error = init::initialize(&root).expect_err("non-generated manifest must be rejected");

    assert_eq!(error.code(), "SGE-APP-001");

    fs::remove_dir_all(root).expect("failed to clean up temp workspace");
}
