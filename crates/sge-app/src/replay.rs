use std::{
    fs,
    path::{Component, Path},
};

use serde::{Deserialize, Serialize};
use sge_store::{GitLineageRepository, LineageRepository, Revision};
use sha2::{Digest, Sha256};

use crate::{
    AppError, Result,
    evolve::{evaluate_instructions, read_suite},
    explain::validate_run_id,
};

pub const REPLAY_V1: &str = "sge.dev/replay/v1";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayCandidate {
    pub id: String,
    pub revision: String,
    pub evidence_path: String,
    pub evidence_hash: String,
    pub normalized_replay_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayDocument {
    pub schema: String,
    pub run_id: String,
    pub target: String,
    pub baseline_revision: String,
    pub baseline_evidence_path: String,
    pub baseline_evidence_hash: String,
    pub baseline_replay_hash: String,
    pub candidates: Vec<ReplayCandidate>,
    pub selected_candidate: Option<String>,
    pub selected_revision: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayOutcome {
    pub run_id: String,
    pub matches: bool,
    pub checked_evidence: usize,
    pub mismatches: Vec<String>,
}

pub fn replay_run(workspace: impl AsRef<Path>, run_id: &str) -> Result<ReplayOutcome> {
    validate_run_id(run_id)?;
    let workspace = workspace.as_ref();
    let replay_path = workspace
        .join(".singularity/runs")
        .join(run_id)
        .join("replay.yaml");
    let document: ReplayDocument = serde_yaml::from_str(
        &fs::read_to_string(&replay_path).map_err(|source| AppError::Io {
            path: replay_path.clone(),
            source,
        })?,
    )
    .map_err(|error| AppError::Evolution {
        path: replay_path,
        message: error.to_string(),
    })?;
    if document.schema != REPLAY_V1 {
        return Err(AppError::Evolution {
            path: workspace.to_path_buf(),
            message: format!("unsupported replay schema {}", document.schema),
        });
    }

    let repository = GitLineageRepository::init_or_open_bare(
        workspace.join(".singularity/repo.git"),
    )
    .map_err(|error| AppError::Evolution {
        path: workspace.to_path_buf(),
        message: error.to_string(),
    })?;
    let baseline_dir = tempfile::tempdir().map_err(|source| AppError::Io {
        path: std::env::temp_dir(),
        source,
    })?;
    repository
        .restore(
            &Revision::parse(&document.baseline_revision).map_err(|error| AppError::Evolution {
                path: workspace.to_path_buf(),
                message: error.to_string(),
            })?,
            baseline_dir.path(),
        )
        .map_err(|error| AppError::Evolution {
            path: workspace.to_path_buf(),
            message: error.to_string(),
        })?;
    let suite = read_suite(&baseline_dir.path().join("evals/code-review.yaml"))?;
    let baseline_instructions = fs::read_to_string(baseline_dir.path().join("instructions.md"))
        .map_err(|source| AppError::Io {
            path: baseline_dir.path().join("instructions.md"),
            source,
        })?;
    let baseline = evaluate_instructions(&suite, &baseline_instructions, baseline_dir.path())?;
    let mut checked_evidence = 1;
    let mut mismatches = Vec::new();
    verify_evidence_hash(
        workspace,
        run_id,
        &document.baseline_evidence_path,
        &document.baseline_evidence_hash,
        "baseline",
        &mut mismatches,
    )?;
    if baseline.normalized_replay_hash != document.baseline_replay_hash {
        mismatches.push("baseline replay hash changed".to_string());
    }

    for candidate in &document.candidates {
        verify_evidence_hash(
            workspace,
            run_id,
            &candidate.evidence_path,
            &candidate.evidence_hash,
            &candidate.id,
            &mut mismatches,
        )?;
        let Some(expected_hash) = candidate.normalized_replay_hash.as_ref() else {
            continue;
        };
        checked_evidence += 1;
        let candidate_dir = tempfile::tempdir().map_err(|source| AppError::Io {
            path: std::env::temp_dir(),
            source,
        })?;
        repository
            .restore(
                &Revision::parse(&candidate.revision).map_err(|error| AppError::Evolution {
                    path: workspace.to_path_buf(),
                    message: error.to_string(),
                })?,
                candidate_dir.path(),
            )
            .map_err(|error| AppError::Evolution {
                path: workspace.to_path_buf(),
                message: error.to_string(),
            })?;
        let instructions = fs::read_to_string(candidate_dir.path().join("instructions.md"))
            .map_err(|source| AppError::Io {
                path: candidate_dir.path().join("instructions.md"),
                source,
            })?;
        let report = evaluate_instructions(&suite, &instructions, candidate_dir.path())?;
        if &report.normalized_replay_hash != expected_hash {
            mismatches.push(format!("{} replay hash changed", candidate.id));
        }
    }

    Ok(ReplayOutcome {
        run_id: run_id.to_string(),
        matches: mismatches.is_empty(),
        checked_evidence,
        mismatches,
    })
}

fn verify_evidence_hash(
    workspace: &Path,
    run_id: &str,
    relative_path: &str,
    expected_hash: &str,
    label: &str,
    mismatches: &mut Vec<String>,
) -> Result<()> {
    let run_dir = workspace.join(".singularity/runs").join(run_id);
    let relative = Path::new(relative_path);
    if relative.is_absolute()
        || relative
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(AppError::Evolution {
            path: relative.to_path_buf(),
            message: "replay evidence path escapes the run directory".to_string(),
        });
    }
    let path = run_dir.join(relative);
    let bytes = fs::read(&path).map_err(|source| AppError::Io {
        path: path.clone(),
        source,
    })?;
    let actual_hash = format!("{:x}", Sha256::digest(bytes));
    if actual_hash != expected_hash {
        mismatches.push(format!("{label} evidence hash changed"));
    }
    Ok(())
}
