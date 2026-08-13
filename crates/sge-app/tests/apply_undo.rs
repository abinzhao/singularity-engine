use std::{collections::BTreeMap, fs, path::Path};

use serde_json::json;
use sge_app::{
    apply::{ApplyFault, ApplyOptions, apply_run},
    evolve::{EvolveOptions, evolve_workspace},
    import::import_artifact,
    init,
    undo::{undo_revision, undo_run},
};
use sge_protocol::EvidenceDocument;
use sge_store::{GitLineageRepository, LineageRepository, Revision};
use walkdir::WalkDir;

fn fixture_path(relative: &str) -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

fn prepare_workspace(root: &Path) {
    init::initialize(root).expect("workspace initialization should succeed");
    import_artifact(root, fixture_path("fixtures/evolution/basic-skill"))
        .expect("fixture import should succeed");
    fs::create_dir_all(root.join("evals/results")).expect("create eval results");
    for index in 1..=2 {
        let evidence = EvidenceDocument {
            schema: "sge.dev/evidence/v1".to_string(),
            id: format!("eval:sql-miss-{index}"),
            target: "skill:code-review".to_string(),
            claim: "Missed SQL injection in concatenated query".to_string(),
            source: "evaluation".to_string(),
            status: "confirmed".to_string(),
            details: BTreeMap::from([(
                "estimated_improvement".to_string(),
                json!({"task_success": [0.2, 0.4]}),
            )]),
            extensions: BTreeMap::new(),
        };
        fs::write(
            root.join(format!("evals/results/sql-miss-{index}.yaml")),
            serde_yaml::to_string(&evidence).expect("serialize evidence"),
        )
        .expect("write evidence");
    }
}

async fn evolve(root: &Path) -> sge_app::evolve::EvolveOutcome {
    evolve_workspace(
        root,
        "skill:code-review",
        EvolveOptions {
            approve: "prop-sql-injection-guard".to_string(),
            provider_fixture: fixture_path("fixtures/provider/prompt-candidates.json"),
            candidate_count: 3,
        },
    )
    .await
    .expect("evolution should reach review")
}

fn directory_bytes(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut files = WalkDir::new(root)
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

#[tokio::test]
async fn failure_after_backup_restores_the_exact_standard_skill_directory() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    let outcome = evolve(temp.path()).await;
    let standard = temp.path().join("skills/code-review");
    let before = directory_bytes(&standard);

    let error = apply_run(
        temp.path(),
        &outcome.run_id,
        ApplyOptions {
            approved: true,
            fault: Some(ApplyFault::AfterBackup),
        },
    )
    .expect_err("injected failure must abort apply");

    assert!(error.to_string().contains("injected failure"));
    assert_eq!(directory_bytes(&standard), before);
    assert!(!temp.path().join("skills/.code-review.sge-backup").exists());
    assert!(!temp.path().join("skills/.code-review.sge-stage").exists());
}

#[tokio::test]
async fn apply_switches_the_whole_skill_and_undo_creates_a_restoration_revision() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    let outcome = evolve(temp.path()).await;
    let standard = temp.path().join("skills/code-review");
    let baseline = directory_bytes(&standard);
    let selected = outcome
        .candidates
        .iter()
        .find(|candidate| Some(candidate.id.as_str()) == outcome.selected_candidate.as_deref())
        .expect("selected candidate");
    let selected_tree = directory_bytes(&selected.worktree_path);

    let applied = apply_run(
        temp.path(),
        &outcome.run_id,
        ApplyOptions {
            approved: true,
            fault: None,
        },
    )
    .expect("apply should succeed");

    assert_eq!(directory_bytes(&standard), selected_tree);
    assert_ne!(applied.applied_revision, applied.previous_revision);
    assert!(applied.record_path.is_file());

    let undone = undo_run(temp.path(), &outcome.run_id).expect("undo should succeed");
    assert_eq!(directory_bytes(&standard), baseline);
    assert_ne!(undone.restoration_revision, applied.previous_revision);
    assert_ne!(undone.restoration_revision, applied.applied_revision);

    let repository =
        GitLineageRepository::init_or_open_bare(temp.path().join(".singularity/repo.git"))
            .expect("open lineage repository");
    let restored = tempfile::tempdir().expect("restored tempdir");
    repository
        .restore(
            &Revision::parse(&undone.restoration_revision).expect("restoration revision"),
            restored.path(),
        )
        .expect("restore new revision");
    assert_eq!(directory_bytes(restored.path()), baseline);
}

#[tokio::test]
async fn apply_requires_explicit_approval_and_review_pending_state() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    let outcome = evolve(temp.path()).await;

    let error = apply_run(
        temp.path(),
        &outcome.run_id,
        ApplyOptions {
            approved: false,
            fault: None,
        },
    )
    .expect_err("apply without approval must fail");
    assert!(error.to_string().contains("explicit approval"));

    apply_run(
        temp.path(),
        &outcome.run_id,
        ApplyOptions {
            approved: true,
            fault: None,
        },
    )
    .expect("first approved apply should succeed");
    let repeated = apply_run(
        temp.path(),
        &outcome.run_id,
        ApplyOptions {
            approved: true,
            fault: None,
        },
    )
    .expect_err("completed run must not apply twice");
    assert!(repeated.to_string().contains("ReviewPending"));
}

#[tokio::test]
async fn undo_accepts_an_explicit_revision_without_rewriting_history() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    let outcome = evolve(temp.path()).await;
    let replay: serde_yaml::Value = serde_yaml::from_str(
        &fs::read_to_string(
            outcome
                .journal_path
                .parent()
                .expect("run directory")
                .join("replay.yaml"),
        )
        .expect("read replay"),
    )
    .expect("parse replay");
    let baseline_revision = replay["baseline_revision"]
        .as_str()
        .expect("baseline revision");
    let applied = apply_run(
        temp.path(),
        &outcome.run_id,
        ApplyOptions {
            approved: true,
            fault: None,
        },
    )
    .expect("apply should succeed");

    let undone = undo_revision(temp.path(), "skill:code-review", baseline_revision)
        .expect("explicit revision undo should succeed");

    assert_eq!(undone.restored_revision, baseline_revision);
    assert_ne!(undone.restoration_revision, baseline_revision);
    assert_ne!(undone.restoration_revision, applied.applied_revision);
}

#[tokio::test]
async fn apply_refuses_to_overwrite_a_standard_skill_changed_after_evolution() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    let outcome = evolve(temp.path()).await;
    let instructions = temp.path().join("skills/code-review/instructions.md");
    fs::write(&instructions, "user changed this after review\n").expect("change standard source");

    let error = apply_run(
        temp.path(),
        &outcome.run_id,
        ApplyOptions {
            approved: true,
            fault: None,
        },
    )
    .expect_err("stale standard source must not be overwritten");

    assert!(
        error
            .to_string()
            .contains("changed since the evolution baseline")
    );
    assert_eq!(
        fs::read_to_string(instructions).expect("read user change"),
        "user changed this after review\n"
    );
}
