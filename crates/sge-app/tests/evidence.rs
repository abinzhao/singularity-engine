use std::{collections::BTreeMap, fs, path::Path};

use serde_json::json;
use sge_app::{
    evolve::{EvolveOptions, evolve_workspace},
    explain::explain_run,
    history::{diff_revisions, history_target},
    import::import_artifact,
    init,
    replay::replay_run,
};
use sge_protocol::EvidenceDocument;

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

#[tokio::test]
async fn evolution_run_contains_complete_typed_evidence() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    let outcome = evolve(temp.path()).await;
    let run_dir = outcome.journal_path.parent().expect("run directory");

    for relative in [
        "contract.yaml",
        "baseline.json",
        "proposals.json",
        "decision.md",
        "mutation.patch",
        "replay.yaml",
    ] {
        assert!(run_dir.join(relative).is_file(), "missing {relative}");
    }
    for candidate in &outcome.candidates {
        assert!(candidate.evidence_path.is_file());
    }

    let selected = outcome
        .candidates
        .iter()
        .find(|candidate| Some(candidate.id.as_str()) == outcome.selected_candidate.as_deref())
        .expect("selected candidate");
    let selected_report = selected.evaluation.as_ref().expect("selected evaluation");
    let relative_evidence = selected
        .evidence_path
        .strip_prefix(run_dir)
        .expect("relative evidence path")
        .display()
        .to_string();
    let decision = fs::read_to_string(run_dir.join("decision.md")).expect("read decision");
    assert!(decision.contains(&relative_evidence));
    assert!(decision.contains(&selected_report.normalized_replay_hash));
    assert!(
        !decision.contains("# Code Review V2"),
        "raw model output must not be presented as trusted rationale"
    );

    let replay: serde_yaml::Value = serde_yaml::from_str(
        &fs::read_to_string(run_dir.join("replay.yaml")).expect("read replay"),
    )
    .expect("parse replay");
    assert_eq!(replay["schema"], "sge.dev/replay/v1");
    assert_eq!(replay["run_id"], outcome.run_id);
    assert_eq!(replay["target"], outcome.target);
}

#[tokio::test]
async fn explain_history_diff_and_replay_use_persisted_evidence() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    let outcome = evolve(temp.path()).await;

    let explanation = explain_run(temp.path(), &outcome.run_id).expect("explain run");
    assert!(explanation.markdown.contains("candidate-2"));

    let history = history_target(temp.path(), "skill:code-review").expect("target history");
    assert!(history.iter().any(|entry| entry.run_id == outcome.run_id));

    let replay = replay_run(temp.path(), &outcome.run_id).expect("replay run");
    assert!(replay.matches);
    assert!(replay.checked_evidence >= 2);

    let replay_doc: serde_yaml::Value = serde_yaml::from_str(
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
    let baseline_revision = replay_doc["baseline_revision"]
        .as_str()
        .expect("baseline revision");
    let selected_revision = replay_doc["selected_revision"]
        .as_str()
        .expect("selected revision");
    let diff =
        diff_revisions(temp.path(), baseline_revision, selected_revision).expect("diff revisions");
    assert!(diff.contains("instructions.md"));
    assert!(diff.contains("-You are a code review assistant."));
    assert!(diff.contains("+## Required Rules"));
}

#[tokio::test]
async fn replay_detects_tampered_metric_evidence() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    let outcome = evolve(temp.path()).await;
    let selected = outcome
        .candidates
        .iter()
        .find(|candidate| Some(candidate.id.as_str()) == outcome.selected_candidate.as_deref())
        .expect("selected candidate");
    fs::write(&selected.evidence_path, b"{\"tampered\":true}\n")
        .expect("tamper candidate evidence");

    let replay = replay_run(temp.path(), &outcome.run_id).expect("replay should complete");
    assert!(!replay.matches);
    assert!(
        replay
            .mismatches
            .iter()
            .any(|message| message.contains("evidence hash changed"))
    );
}

#[tokio::test]
async fn replay_rejects_evidence_paths_that_escape_the_run_directory() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    let outcome = evolve(temp.path()).await;
    let replay_path = outcome
        .journal_path
        .parent()
        .expect("run directory")
        .join("replay.yaml");
    let mut replay: serde_yaml::Value =
        serde_yaml::from_str(&fs::read_to_string(&replay_path).expect("read replay"))
            .expect("parse replay");
    replay["baseline_evidence_path"] = serde_yaml::Value::String("../outside.json".to_string());
    fs::write(
        &replay_path,
        serde_yaml::to_string(&replay).expect("serialize replay"),
    )
    .expect("write malicious replay");

    let error = replay_run(temp.path(), &outcome.run_id)
        .expect_err("escaping evidence path must be rejected");
    assert!(error.to_string().contains("escapes the run directory"));
}
