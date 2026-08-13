use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Value, json};
use sge_domain::TargetRef;
use sge_protocol::{
    ArtifactDocument, CONTRACT_V1, ContractDocument, EvidenceDocument, MemoryDocument,
};
use thiserror::Error;

use crate::{
    proposal::Proposal,
    state::{Approved, Diagnosed},
};

#[derive(Debug, Clone)]
pub struct ScanInput {
    pub target: TargetRef,
    pub artifact: ArtifactDocument,
    pub declared_files: BTreeMap<String, String>,
    pub evidence: Vec<EvidenceDocument>,
    pub memories: Vec<MemoryDocument>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ScanError {
    #[error("no trusted evidence exists for the target")]
    NoTrustedEvidence,
    #[error("unknown proposal `{0}`")]
    UnknownProposal(String),
    #[error("explicit goal must not be empty")]
    EmptyGoal,
}

pub fn scan(input: ScanInput) -> Result<Diagnosed, ScanError> {
    let target = input.target.to_string();
    let evidence: Vec<_> = input
        .evidence
        .iter()
        .filter(|item| {
            item.target == target && item.source == "evaluation" && item.status == "confirmed"
        })
        .collect();
    let memories: Vec<_> = input
        .memories
        .iter()
        .filter(|item| {
            item.target == target
                && item.extensions.get("status").and_then(Value::as_str) == Some("confirmed")
        })
        .collect();

    if evidence.is_empty() && memories.is_empty() {
        return Err(ScanError::NoTrustedEvidence);
    }

    let declared_manifest_files = manifest_files(&input.artifact);
    let declared_files: Vec<String> = input
        .declared_files
        .keys()
        .filter(|path| declared_manifest_files.contains(*path))
        .cloned()
        .collect();
    let all_refs: Vec<String> = evidence
        .iter()
        .map(|item| item.id.clone())
        .chain(memories.iter().map(|item| item.id.clone()))
        .collect();
    let sql_refs: Vec<String> = evidence
        .iter()
        .filter(|item| contains_sql_injection(&format!("{} {:?}", item.claim, item.details)))
        .map(|item| item.id.clone())
        .chain(
            memories
                .iter()
                .filter(|item| {
                    contains_sql_injection(&format!(
                        "{} {} {:?}",
                        item.summary, item.content, item.tags
                    ))
                })
                .map(|item| item.id.clone()),
        )
        .collect();
    let estimates = evidence_estimates(&evidence);
    let estimate_basis = if estimates.is_empty() {
        "unknown"
    } else {
        "evidence-derived"
    };

    let mut proposals = Vec::new();
    if sql_refs.len() >= 2 {
        proposals.push(Proposal {
            id: "prop-sql-injection-guard".to_string(),
            title: "Strengthen SQL injection detection".to_string(),
            risk: "high".to_string(),
            affected_files: preferred_sql_files(&declared_files),
            confidence: 0.95,
            evidence_refs: sql_refs,
            estimated_improvement: estimates.clone(),
            evaluation_method: "rerun confirmed SQL injection evaluation cases".to_string(),
            estimate_basis: estimate_basis.to_string(),
        });
    }

    proposals.push(Proposal {
        id: "prop-confirmed-failure-guidance".to_string(),
        title: "Address confirmed evaluation failures".to_string(),
        risk: "medium".to_string(),
        affected_files: declared_files.clone(),
        confidence: if evidence.is_empty() { 0.65 } else { 0.8 },
        evidence_refs: all_refs.clone(),
        estimated_improvement: estimates.clone(),
        evaluation_method: "rerun the confirmed evaluation evidence set".to_string(),
        estimate_basis: estimate_basis.to_string(),
    });
    proposals.push(Proposal {
        id: "prop-regression-coverage".to_string(),
        title: "Add regression guidance for confirmed failure patterns".to_string(),
        risk: "low".to_string(),
        affected_files: declared_files,
        confidence: 0.7,
        evidence_refs: all_refs,
        estimated_improvement: estimates,
        evaluation_method: "replay referenced evidence and compare multidimensional metrics"
            .to_string(),
        estimate_basis: estimate_basis.to_string(),
    });

    Ok(Diagnosed {
        target: input.target,
        proposals,
    })
}

pub fn approve_proposal(diagnosed: &Diagnosed, proposal_id: &str) -> Result<Approved, ScanError> {
    let proposal = diagnosed
        .proposals
        .iter()
        .find(|proposal| proposal.id == proposal_id)
        .ok_or_else(|| ScanError::UnknownProposal(proposal_id.to_string()))?;
    let extensions = BTreeMap::from([
        ("source".to_string(), json!("proposal")),
        ("proposal_id".to_string(), json!(proposal.id)),
        ("risk".to_string(), json!(proposal.risk)),
        ("affected_files".to_string(), json!(proposal.affected_files)),
        (
            "evaluation_method".to_string(),
            json!(proposal.evaluation_method),
        ),
        (
            "estimated_improvement".to_string(),
            json!(proposal.estimated_improvement),
        ),
        ("confidence".to_string(), json!(proposal.confidence)),
        ("estimate_basis".to_string(), json!(proposal.estimate_basis)),
    ]);

    Ok(Approved {
        contract: ContractDocument {
            schema: CONTRACT_V1.to_string(),
            id: format!("contract:{}", proposal.id),
            target: diagnosed.target.to_string(),
            intent: proposal.title.clone(),
            inputs: proposal.evidence_refs.clone(),
            outputs: vec!["updated artifact".to_string()],
            success: vec![proposal.evaluation_method.clone()],
            extensions,
        },
    })
}

pub fn approve_goal(target: &TargetRef, goal: &str) -> Result<Approved, ScanError> {
    let goal = goal.trim();
    if goal.is_empty() {
        return Err(ScanError::EmptyGoal);
    }

    Ok(Approved {
        contract: ContractDocument {
            schema: CONTRACT_V1.to_string(),
            id: format!("contract:explicit-goal:{}", target.name()),
            target: target.to_string(),
            intent: goal.to_string(),
            inputs: Vec::new(),
            outputs: vec!["updated artifact".to_string()],
            success: vec![goal.to_string()],
            extensions: BTreeMap::from([("source".to_string(), json!("explicit_goal"))]),
        },
    })
}

fn manifest_files(artifact: &ArtifactDocument) -> BTreeSet<String> {
    let files: BTreeSet<String> = artifact
        .extensions
        .get("files")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|entry| match entry {
            Value::String(path) => Some(path.clone()),
            Value::Object(fields) => fields
                .get("path")
                .and_then(Value::as_str)
                .map(str::to_string),
            _ => None,
        })
        .collect();
    if files.is_empty() {
        BTreeSet::from(["instructions.md".to_string()])
    } else {
        files
    }
}

fn contains_sql_injection(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    text.contains("sql injection") || text.contains("sql-injection")
}

fn preferred_sql_files(files: &[String]) -> Vec<String> {
    let preferred: Vec<_> = files
        .iter()
        .filter(|path| path.to_ascii_lowercase().contains("sql"))
        .cloned()
        .collect();
    if preferred.is_empty() {
        files.to_vec()
    } else {
        preferred
    }
}

fn evidence_estimates(evidence: &[&EvidenceDocument]) -> BTreeMap<String, [f64; 2]> {
    let mut estimates = BTreeMap::new();
    for item in evidence {
        let Some(Value::Object(metrics)) = item.details.get("estimated_improvement") else {
            continue;
        };
        for (metric, value) in metrics {
            let Some(range) = value.as_array() else {
                continue;
            };
            if range.len() != 2 {
                continue;
            }
            let (Some(low), Some(high)) = (range[0].as_f64(), range[1].as_f64()) else {
                continue;
            };
            if low.is_finite() && high.is_finite() && low <= high {
                estimates.entry(metric.clone()).or_insert([low, high]);
            }
        }
    }
    estimates
}
