use std::{collections::BTreeMap, fs, path::Path};

use serde_json::json;
use sge_app::{
    evolve::{EvolveOptions, evolve_workspace},
    import::import_artifact,
    init,
};
use sge_protocol::EvidenceDocument;
use sge_store::{JournalEntry, JournalState};

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

    let evidence = EvidenceDocument {
        schema: "sge.dev/evidence/v1".to_string(),
        id: "eval:sql-miss".to_string(),
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
    for (name, id) in [
        ("sql-miss-1.yaml", "eval:sql-miss-1"),
        ("sql-miss-2.yaml", "eval:sql-miss-2"),
    ] {
        let mut evidence = evidence.clone();
        evidence.id = id.to_string();
        fs::write(
            root.join("evals/results").join(name),
            serde_yaml::to_string(&evidence).expect("serialize evidence"),
        )
        .expect("write evidence");
    }
}

#[tokio::test]
async fn evolution_isolates_three_candidates_and_stops_for_review() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    let standard_source = temp.path().join("skills/code-review/instructions.md");
    let source_before = fs::read(&standard_source).expect("read standard source");

    let outcome = evolve_workspace(
        temp.path(),
        "skill:code-review",
        EvolveOptions {
            approve: "prop-sql-injection-guard".to_string(),
            provider_fixture: fixture_path("fixtures/provider/prompt-candidates.json"),
            candidate_count: 3,
        },
    )
    .await
    .expect("evolution should reach review");

    assert_eq!(outcome.candidates.len(), 3);
    assert_eq!(
        outcome.selected_candidate.as_deref(),
        Some("candidate-2"),
        "only the fully capable candidate should satisfy the contract"
    );
    assert_eq!(
        fs::read(&standard_source).expect("read source"),
        source_before
    );

    for candidate in &outcome.candidates {
        assert!(candidate.worktree_path.is_dir());
        assert!(candidate.evidence_path.is_file());
        assert!(
            candidate
                .worktree_path
                .starts_with(temp.path().join(".singularity/worktrees"))
        );
    }
    assert_ne!(
        fs::read(outcome.candidates[0].worktree_path.join("instructions.md"))
            .expect("read first candidate"),
        fs::read(outcome.candidates[1].worktree_path.join("instructions.md"))
            .expect("read second candidate")
    );

    let entries = fs::read_to_string(&outcome.journal_path)
        .expect("read journal")
        .lines()
        .map(|line| serde_json::from_str::<JournalEntry>(line).expect("parse journal entry"))
        .collect::<Vec<_>>();
    assert_eq!(
        entries.iter().map(|entry| entry.state).collect::<Vec<_>>(),
        vec![
            JournalState::Prepared,
            JournalState::Baseline,
            JournalState::Diagnosed,
            JournalState::Approved,
            JournalState::Mutating,
            JournalState::Evaluating,
            JournalState::ReviewPending,
        ]
    );
    assert!(
        outcome
            .candidates
            .iter()
            .filter(|candidate| candidate.rejection_reason.is_some())
            .count()
            >= 2,
        "every losing candidate should record a rejection reason"
    );
}

#[tokio::test]
async fn one_candidate_evaluation_failure_does_not_abort_other_candidates() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());

    let outcome = evolve_workspace(
        temp.path(),
        "skill:code-review",
        EvolveOptions {
            approve: "prop-sql-injection-guard".to_string(),
            provider_fixture: fixture_path("fixtures/provider/prompt-candidates.json"),
            candidate_count: 3,
        },
    )
    .await
    .expect("isolated candidate failure should not abort the run");

    assert!(
        outcome
            .candidates
            .iter()
            .any(|candidate| candidate.evaluation.is_none())
    );
    assert!(outcome.selected_candidate.is_some());
}
