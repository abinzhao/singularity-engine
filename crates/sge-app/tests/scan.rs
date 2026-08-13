use std::{collections::BTreeMap, fs, path::Path};

use serde_json::json;
use sge_app::{
    AppError,
    import::import_artifact,
    init,
    scan::{ScanOptions, scan_workspace},
};
use sge_evolution::proposal::Proposal;
use sge_protocol::{EvidenceDocument, MemoryDocument};

fn fixture_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("fixtures/evolution/basic-skill")
}

fn prepare_workspace(root: &Path) {
    init::initialize(root).expect("workspace initialization should succeed");
    import_artifact(root, fixture_path()).expect("fixture import should succeed");
    fs::create_dir_all(root.join("evals/results")).expect("create eval results");
}

fn write_evidence(root: &Path, name: &str, id: &str) {
    let document = EvidenceDocument {
        schema: "sge.dev/evidence/v1".to_string(),
        id: id.to_string(),
        target: "skill:code-review".to_string(),
        claim: "Missed SQL injection in concatenated query".to_string(),
        source: "evaluation".to_string(),
        status: "confirmed".to_string(),
        details: BTreeMap::from([(
            "estimated_improvement".to_string(),
            json!({"safety": [0.1, 0.2]}),
        )]),
        extensions: BTreeMap::new(),
    };
    fs::write(
        root.join("evals/results").join(name),
        serde_yaml::to_string(&document).expect("serialize evidence"),
    )
    .expect("write evidence");
}

fn write_memory(root: &Path) {
    let document = MemoryDocument {
        schema: "sge.dev/memory/v1".to_string(),
        id: "memory:sql-repeat".to_string(),
        target: "skill:code-review".to_string(),
        summary: "Repeated SQL injection misses".to_string(),
        content: "Unsafe concatenation was approved more than once.".to_string(),
        tags: vec!["sql-injection".to_string()],
        extensions: BTreeMap::from([("status".to_string(), json!("confirmed"))]),
    };
    fs::write(
        root.join("memory/failures/sql.yaml"),
        serde_yaml::to_string(&document).expect("serialize memory"),
    )
    .expect("write memory");
}

#[test]
fn scan_persists_deserializable_grounded_proposals() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    write_evidence(temp.path(), "sql-1.yaml", "eval:sql-1");
    write_evidence(temp.path(), "sql-2.yml", "eval:sql-2");
    write_memory(temp.path());

    let result = scan_workspace(temp.path(), "skill:code-review", ScanOptions::default())
        .expect("scan should succeed");

    assert_eq!(result.target, "skill:code-review");
    assert!(result.proposals_path.is_file());
    assert!(result.contract_path.is_none());
    let proposals: Vec<Proposal> =
        serde_json::from_str(&fs::read_to_string(&result.proposals_path).expect("read proposals"))
            .expect("deserialize proposals");
    assert_eq!(proposals, result.proposals);
    assert!(
        proposals[0]
            .title
            .to_ascii_lowercase()
            .contains("sql injection")
    );
}

#[test]
fn explicit_approval_writes_contract_without_changing_source_skill() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    write_evidence(temp.path(), "sql-1.yaml", "eval:sql-1");
    write_evidence(temp.path(), "sql-2.yaml", "eval:sql-2");
    let source = temp.path().join("skills/code-review/instructions.md");
    let before = fs::read(&source).expect("read source before scan");

    let result = scan_workspace(
        temp.path(),
        "skill:code-review",
        ScanOptions {
            approve: Some("prop-sql-injection-guard".to_string()),
            goal: None,
        },
    )
    .expect("approval scan should succeed");

    let contract_path = result.contract_path.expect("contract path");
    let contract: sge_protocol::ContractDocument =
        serde_yaml::from_str(&fs::read_to_string(contract_path).expect("read contract"))
            .expect("deserialize contract");
    assert_eq!(
        contract.extensions["proposal_id"],
        "prop-sql-injection-guard"
    );
    assert_eq!(fs::read(source).expect("read source after scan"), before);
}

#[test]
fn explicit_goal_writes_contract_without_requiring_scan_evidence() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());

    let result = scan_workspace(
        temp.path(),
        "skill:code-review",
        ScanOptions {
            approve: None,
            goal: Some("Eliminate unsafe SQL approvals".to_string()),
        },
    )
    .expect("an explicit goal should not require prior evidence");

    assert!(result.proposals.is_empty());
    let contract_path = result.contract_path.expect("contract path");
    let contract: sge_protocol::ContractDocument =
        serde_yaml::from_str(&fs::read_to_string(contract_path).expect("read contract"))
            .expect("deserialize contract");
    assert_eq!(contract.intent, "Eliminate unsafe SQL approvals");
    assert_eq!(contract.extensions["source"], "explicit_goal");
}

#[test]
fn evidence_outside_results_directory_is_not_read() {
    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    let outside = temp.path().join("untrusted");
    fs::create_dir_all(&outside).expect("create untrusted directory");
    let document = EvidenceDocument {
        schema: "sge.dev/evidence/v1".to_string(),
        id: "eval:outside".to_string(),
        target: "skill:code-review".to_string(),
        claim: "Missed SQL injection".to_string(),
        source: "evaluation".to_string(),
        status: "confirmed".to_string(),
        details: BTreeMap::new(),
        extensions: BTreeMap::new(),
    };
    fs::write(
        outside.join("evidence.yaml"),
        serde_yaml::to_string(&document).expect("serialize outside evidence"),
    )
    .expect("write outside evidence");

    let error = scan_workspace(temp.path(), "skill:code-review", ScanOptions::default())
        .expect_err("outside evidence must not make scan trusted");
    assert!(matches!(error, AppError::NoTrustedEvidence));
    assert_eq!(error.code(), "SGE-SCAN-003");
}

#[cfg(unix)]
#[test]
fn evidence_symlink_cannot_escape_results_directory() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().expect("create tempdir");
    prepare_workspace(temp.path());
    let outside = temp.path().join("outside-evidence.yaml");
    let document = EvidenceDocument {
        schema: "sge.dev/evidence/v1".to_string(),
        id: "eval:outside-link".to_string(),
        target: "skill:code-review".to_string(),
        claim: "Missed SQL injection".to_string(),
        source: "evaluation".to_string(),
        status: "confirmed".to_string(),
        details: BTreeMap::new(),
        extensions: BTreeMap::new(),
    };
    fs::write(
        &outside,
        serde_yaml::to_string(&document).expect("serialize outside evidence"),
    )
    .expect("write outside evidence");
    symlink(
        &outside,
        temp.path().join("evals/results/linked-evidence.yaml"),
    )
    .expect("create evidence symlink");

    let error = scan_workspace(temp.path(), "skill:code-review", ScanOptions::default())
        .expect_err("symlinked evidence must not make scan trusted");
    assert!(matches!(error, AppError::NoTrustedEvidence));
}
