use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sge_domain::TargetRef;
use sge_evolution::{
    scan::{ScanError, ScanInput, approve_goal, approve_proposal, scan},
    state::Diagnosed,
};
use sge_protocol::{ArtifactDocument, CONTRACT_V1, EvidenceDocument, MemoryDocument};

fn target() -> TargetRef {
    "skill:code-review".parse().expect("valid target")
}

fn artifact() -> ArtifactDocument {
    ArtifactDocument {
        schema: "sge.dev/artifact/v1".to_string(),
        id: "code-review-skill-v1".to_string(),
        kind: "skill".to_string(),
        name: "code-review".to_string(),
        title: "Code Review".to_string(),
        summary: "Review changes".to_string(),
        body: "see instructions.md".to_string(),
        extensions: BTreeMap::from([(
            "files".to_string(),
            json!([{"path": "instructions.md"}, {"path": "checks/sql.md"}]),
        )]),
    }
}

fn evidence(
    id: &str,
    target: &str,
    status: &str,
    claim: &str,
    details: BTreeMap<String, Value>,
) -> EvidenceDocument {
    EvidenceDocument {
        schema: "sge.dev/evidence/v1".to_string(),
        id: id.to_string(),
        target: target.to_string(),
        claim: claim.to_string(),
        source: "evaluation".to_string(),
        status: status.to_string(),
        details,
        extensions: BTreeMap::new(),
    }
}

fn memory(id: &str, target: &str, status: &str, summary: &str) -> MemoryDocument {
    MemoryDocument {
        schema: "sge.dev/memory/v1".to_string(),
        id: id.to_string(),
        target: target.to_string(),
        summary: summary.to_string(),
        content: "The reviewer repeatedly missed unsafe SQL concatenation.".to_string(),
        tags: vec!["sql-injection".to_string()],
        extensions: BTreeMap::from([("status".to_string(), json!(status))]),
    }
}

fn input(evidence: Vec<EvidenceDocument>, memories: Vec<MemoryDocument>) -> ScanInput {
    ScanInput {
        target: target(),
        artifact: artifact(),
        declared_files: BTreeMap::from([
            (
                "instructions.md".to_string(),
                "Never approve unsafe SQL.".to_string(),
            ),
            (
                "checks/sql.md".to_string(),
                "Check parameterization.".to_string(),
            ),
        ]),
        evidence,
        memories,
    }
}

#[test]
fn repeated_confirmed_sql_injection_misses_rank_first_with_grounded_fields() {
    let range = BTreeMap::from([(
        "estimated_improvement".to_string(),
        json!({"safety": [0.1, 0.25]}),
    )]);
    let evidence = vec![
        evidence(
            "eval:sql-1",
            "skill:code-review",
            "confirmed",
            "Missed SQL injection in string concatenation",
            range,
        ),
        evidence(
            "eval:sql-2",
            "skill:code-review",
            "confirmed",
            "Repeated SQL injection miss",
            BTreeMap::new(),
        ),
    ];
    let memories = vec![memory(
        "memory:sql-pattern",
        "skill:code-review",
        "confirmed",
        "SQL injection misses recur",
    )];
    let diagnosed = scan(input(evidence, memories)).expect("trusted scan should succeed");

    assert!((2..=5).contains(&diagnosed.proposals.len()));
    let first = &diagnosed.proposals[0];
    assert!(first.title.to_ascii_lowercase().contains("sql injection"));
    assert!(!first.id.is_empty());
    assert!(!first.risk.is_empty());
    assert!(!first.evaluation_method.is_empty());
    assert_eq!(first.estimate_basis, "evidence-derived");
    assert_eq!(
        first.estimated_improvement.get("safety"),
        Some(&[0.1, 0.25])
    );

    let real_refs = BTreeSet::from([
        "eval:sql-1".to_string(),
        "eval:sql-2".to_string(),
        "memory:sql-pattern".to_string(),
    ]);
    let declared = BTreeSet::from(["instructions.md".to_string(), "checks/sql.md".to_string()]);
    for proposal in &diagnosed.proposals {
        assert!((0.0..=1.0).contains(&proposal.confidence));
        assert!(!proposal.risk.is_empty());
        assert!(!proposal.evidence_refs.is_empty());
        assert!(
            proposal
                .evidence_refs
                .iter()
                .all(|reference| real_refs.contains(reference))
        );
        assert!(
            proposal
                .affected_files
                .iter()
                .all(|path| declared.contains(path))
        );
        assert!(!proposal.evaluation_method.is_empty());
        assert!(
            proposal.estimate_basis == "unknown" || proposal.estimate_basis == "evidence-derived"
        );
    }
}

#[test]
fn ignores_unconfirmed_and_target_mismatched_records() {
    let untrusted = input(
        vec![
            evidence(
                "eval:pending",
                "skill:code-review",
                "pending",
                "SQL injection miss",
                BTreeMap::new(),
            ),
            evidence(
                "eval:other",
                "skill:other",
                "confirmed",
                "SQL injection miss",
                BTreeMap::new(),
            ),
        ],
        vec![
            memory(
                "memory:pending",
                "skill:code-review",
                "pending",
                "SQL issue",
            ),
            memory("memory:other", "skill:other", "confirmed", "SQL issue"),
        ],
    );

    assert_eq!(scan(untrusted), Err(ScanError::NoTrustedEvidence));
}

#[test]
fn ignores_confirmed_evidence_that_is_not_an_evaluation_result() {
    let mut untrusted = evidence(
        "model:sql-guess",
        "skill:code-review",
        "confirmed",
        "SQL injection miss",
        BTreeMap::new(),
    );
    untrusted.source = "model_analysis".to_string();

    assert_eq!(
        scan(input(vec![untrusted], Vec::new())),
        Err(ScanError::NoTrustedEvidence)
    );
}

#[test]
fn approvals_create_v1_contracts_and_reject_unknown_proposals() {
    let diagnosed: Diagnosed = scan(input(
        vec![evidence(
            "eval:sql-1",
            "skill:code-review",
            "confirmed",
            "SQL injection miss",
            BTreeMap::new(),
        )],
        vec![],
    ))
    .expect("scan should succeed");
    let proposal = &diagnosed.proposals[0];

    let approved =
        approve_proposal(&diagnosed, &proposal.id).expect("known proposal should be approved");
    assert_eq!(approved.contract.schema, CONTRACT_V1);
    assert_eq!(approved.contract.target, "skill:code-review");
    assert_eq!(approved.contract.extensions["proposal_id"], proposal.id);
    assert_eq!(approved.contract.extensions["risk"], proposal.risk);
    assert_eq!(
        approved.contract.extensions["affected_files"],
        json!(proposal.affected_files)
    );
    assert_eq!(
        approved.contract.extensions["evaluation_method"],
        proposal.evaluation_method
    );
    assert_eq!(
        approved.contract.extensions["estimated_improvement"],
        json!(proposal.estimated_improvement)
    );
    assert_eq!(
        approved.contract.extensions["confidence"],
        proposal.confidence
    );

    assert_eq!(
        approve_proposal(&diagnosed, "prop-does-not-exist"),
        Err(ScanError::UnknownProposal(
            "prop-does-not-exist".to_string()
        ))
    );

    let goal = approve_goal(&target(), "Eliminate unsafe SQL approvals")
        .expect("explicit goal should be approved");
    assert_eq!(goal.contract.schema, CONTRACT_V1);
    assert_eq!(goal.contract.extensions["source"], "explicit_goal");
    assert_eq!(goal.contract.intent, "Eliminate unsafe SQL approvals");
}
