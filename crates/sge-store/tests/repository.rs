use std::fs;

use serde_json::json;
use sge_store::{GitLineageRepository, LineageRepository};
use tempfile::TempDir;

#[test]
fn snapshot_commits_tree_into_bare_repo_and_restores_it() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().join("lineage.git");
    let source = temp.path().join("source");
    let checkout = temp.path().join("checkout");
    let restore = temp.path().join("restore");

    fs::create_dir_all(source.join("nested")).unwrap();
    fs::write(source.join("README.md"), "first revision\n").unwrap();
    fs::write(source.join("nested/data.txt"), "payload\n").unwrap();

    let repo = GitLineageRepository::init_or_open_bare(&repo_path).unwrap();

    let revision = repo
        .snapshot(&source, json!({ "task": "P0 Task 6", "phase": "red" }))
        .unwrap();

    assert!(!revision.as_str().is_empty());
    let report = repo.verify().unwrap();
    assert!(report.is_clean());
    assert!(report.is_bare);
    assert!(report.object_database_available);

    repo.checkout_candidate(&revision, &checkout).unwrap();
    assert_eq!(
        fs::read_to_string(checkout.join("nested/data.txt")).unwrap(),
        "payload\n"
    );

    fs::remove_file(source.join("nested/data.txt")).unwrap();
    fs::write(source.join("README.md"), "mutated\n").unwrap();

    repo.restore(&revision, &restore).unwrap();
    assert_eq!(
        fs::read_to_string(restore.join("README.md")).unwrap(),
        "first revision\n"
    );
    assert_eq!(
        fs::read_to_string(restore.join("nested/data.txt")).unwrap(),
        "payload\n"
    );
}

#[test]
fn candidate_revisions_share_the_requested_baseline_parent() {
    let temp = tempfile::tempdir().unwrap();
    let store_path = temp.path().join("repo.git");
    let source = temp.path().join("source");
    std::fs::create_dir_all(&source).unwrap();
    std::fs::write(source.join("instructions.md"), "baseline").unwrap();
    let repository = GitLineageRepository::init_or_open_bare(&store_path).unwrap();
    let baseline = repository
        .snapshot(&source, serde_json::json!({"op": "baseline"}))
        .unwrap();

    let first = temp.path().join("first");
    let second = temp.path().join("second");
    repository.checkout_candidate(&baseline, &first).unwrap();
    repository.checkout_candidate(&baseline, &second).unwrap();
    std::fs::write(first.join("instructions.md"), "candidate one").unwrap();
    std::fs::write(second.join("instructions.md"), "candidate two").unwrap();
    let first_revision = repository
        .snapshot_candidate(&baseline, &first, serde_json::json!({"candidate": 1}))
        .unwrap();
    let second_revision = repository
        .snapshot_candidate(&baseline, &second, serde_json::json!({"candidate": 2}))
        .unwrap();

    let git = git2::Repository::open_bare(&store_path).unwrap();
    for revision in [first_revision, second_revision] {
        let commit = git
            .find_commit(git2::Oid::from_str(revision.as_str()).unwrap())
            .unwrap();
        assert_eq!(commit.parent_count(), 1);
        assert_eq!(commit.parent_id(0).unwrap().to_string(), baseline.as_str());
    }
}

#[test]
fn init_or_open_bare_reopens_existing_repository() {
    let temp = TempDir::new().unwrap();
    let repo_path = temp.path().join("lineage.git");

    GitLineageRepository::init_or_open_bare(&repo_path).unwrap();
    let repo = GitLineageRepository::init_or_open_bare(&repo_path).unwrap();

    assert!(repo.verify().unwrap().is_clean());
}
