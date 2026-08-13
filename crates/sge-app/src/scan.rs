use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use sge_domain::{ArtifactKind, TargetRef};
use sge_evolution::{
    proposal::Proposal,
    scan::{ScanError, ScanInput, approve_goal, approve_proposal, scan},
};
use sge_protocol::{
    ArtifactDocument, Document, EVIDENCE_V1, EvidenceDocument, MEMORY_V1, MemoryDocument,
    parse_document,
};

use crate::{AppError, Result, validate::validate_workspace};

#[derive(Debug, Clone, Default)]
pub struct ScanOptions {
    pub approve: Option<String>,
    pub goal: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ScanOutcome {
    pub run_id: String,
    pub target: String,
    pub proposals: Vec<Proposal>,
    pub proposals_path: PathBuf,
    pub contract_path: Option<PathBuf>,
}

pub fn scan_workspace(
    workspace: impl AsRef<Path>,
    target: &str,
    options: ScanOptions,
) -> Result<ScanOutcome> {
    let workspace = workspace.as_ref();
    validate_workspace(workspace).map_err(|error| AppError::InvalidScanWorkspace {
        path: workspace.to_path_buf(),
        message: error.to_string(),
    })?;
    let target_ref: TargetRef =
        target
            .parse()
            .map_err(
                |error: sge_domain::TargetRefParseError| AppError::InvalidScanTarget {
                    target: target.to_string(),
                    message: error.to_string(),
                },
            )?;
    if target_ref.kind() != ArtifactKind::Skill {
        return Err(AppError::InvalidScanTarget {
            target: target.to_string(),
            message: "scan currently accepts only skill targets".to_string(),
        });
    }
    if options.approve.is_some() && options.goal.is_some() {
        return Err(AppError::InvalidScanApproval {
            message: "--approve and --goal are mutually exclusive".to_string(),
        });
    }

    let artifact_dir = workspace.join("skills").join(target_ref.name());
    let manifest_path = artifact_dir.join("skill.yaml");
    if !manifest_path.is_file() {
        return Err(AppError::MissingScanArtifact {
            path: manifest_path,
        });
    }
    let artifact = read_artifact(&manifest_path)?;
    if artifact.kind != "skill" || artifact.name != target_ref.name() {
        return Err(AppError::InvalidScanTarget {
            target: target.to_string(),
            message: "target does not match the skill manifest".to_string(),
        });
    }
    let (proposals, contract) = match (&options.approve, &options.goal) {
        (None, Some(goal)) => (
            Vec::new(),
            Some(
                approve_goal(&target_ref, goal)
                    .map_err(map_scan_error)?
                    .contract,
            ),
        ),
        (approve, None) => {
            let declared_files = read_declared_files(&artifact_dir, &artifact)?;
            let evidence = read_evidence_dir(&workspace.join("evals/results"))?;
            let memories = read_memory_dir(&workspace.join("memory/failures"))?;
            let diagnosed = scan(ScanInput {
                target: target_ref.clone(),
                artifact,
                declared_files,
                evidence,
                memories,
            })
            .map_err(map_scan_error)?;
            let contract = approve
                .as_deref()
                .map(|proposal_id| approve_proposal(&diagnosed, proposal_id))
                .transpose()
                .map_err(map_scan_error)?
                .map(|approved| approved.contract);
            (diagnosed.proposals, contract)
        }
        (Some(_), Some(_)) => unreachable!("approval options checked above"),
    };

    let (run_id, run_dir) = create_run_dir(workspace)?;
    let proposals_path = run_dir.join("proposals.json");
    let proposals_json =
        serde_json::to_vec_pretty(&proposals).map_err(|error| AppError::ScanPersist {
            path: proposals_path.clone(),
            message: error.to_string(),
        })?;
    atomic_write(&proposals_path, &proposals_json)?;

    let contract_path = if let Some(contract) = contract {
        let path = run_dir.join("contract.yaml");
        let yaml = serde_yaml::to_string(&contract).map_err(|error| AppError::ScanPersist {
            path: path.clone(),
            message: error.to_string(),
        })?;
        atomic_write(&path, yaml.as_bytes())?;
        Some(path)
    } else {
        None
    };

    Ok(ScanOutcome {
        run_id,
        target: target_ref.to_string(),
        proposals,
        proposals_path,
        contract_path,
    })
}

fn read_artifact(path: &Path) -> Result<ArtifactDocument> {
    let content = fs::read_to_string(path).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    match parse_document(&content) {
        Ok(Document::Artifact(artifact)) => Ok(artifact),
        Ok(_) => Err(AppError::InvalidScanTarget {
            target: path.display().to_string(),
            message: "manifest is not an ArtifactDocument".to_string(),
        }),
        Err(error) => Err(AppError::InvalidScanTarget {
            target: path.display().to_string(),
            message: error.to_string(),
        }),
    }
}

fn read_declared_files(
    artifact_dir: &Path,
    artifact: &ArtifactDocument,
) -> Result<BTreeMap<String, String>> {
    let paths = match artifact.extensions.get("files") {
        Some(Value::Array(entries)) => entries
            .iter()
            .filter_map(|entry| match entry {
                Value::String(path) => Some(path.clone()),
                Value::Object(fields) => fields
                    .get("path")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                _ => None,
            })
            .collect(),
        _ => vec!["instructions.md".to_string()],
    };

    let mut files = BTreeMap::new();
    for relative in paths {
        let relative_path = Path::new(&relative);
        if relative_path.is_absolute()
            || !relative_path
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
        {
            return Err(AppError::InvalidScanTarget {
                target: artifact.name.clone(),
                message: format!("declared file `{relative}` escapes the artifact directory"),
            });
        }
        let path = artifact_dir.join(relative_path);
        let metadata = fs::symlink_metadata(&path)
            .map_err(|_| AppError::MissingScanArtifact { path: path.clone() })?;
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(AppError::InvalidScanTarget {
                target: artifact.name.clone(),
                message: format!("declared file `{relative}` must be a regular file"),
            });
        }
        let content =
            fs::read_to_string(&path).map_err(|_| AppError::MissingScanArtifact { path })?;
        files.insert(relative, content);
    }
    Ok(files)
}

fn read_evidence_dir(path: &Path) -> Result<Vec<EvidenceDocument>> {
    read_yaml_files(path, |file, content| {
        let document: EvidenceDocument =
            serde_yaml::from_str(content).map_err(|error| AppError::InvalidEvidence {
                path: file.to_path_buf(),
                message: error.to_string(),
            })?;
        if document.schema != EVIDENCE_V1 {
            return Err(AppError::InvalidEvidence {
                path: file.to_path_buf(),
                message: format!("unsupported schema `{}`", document.schema),
            });
        }
        Ok(document)
    })
}

fn read_memory_dir(path: &Path) -> Result<Vec<MemoryDocument>> {
    read_yaml_files(path, |file, content| {
        let document: MemoryDocument =
            serde_yaml::from_str(content).map_err(|error| AppError::InvalidMemory {
                path: file.to_path_buf(),
                message: error.to_string(),
            })?;
        if document.schema != MEMORY_V1 {
            return Err(AppError::InvalidMemory {
                path: file.to_path_buf(),
                message: format!("unsupported schema `{}`", document.schema),
            });
        }
        Ok(document)
    })
}

fn read_yaml_files<T>(path: &Path, parse: impl Fn(&Path, &str) -> Result<T>) -> Result<Vec<T>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let entries = fs::read_dir(path).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut paths = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|source| AppError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let file = entry.path();
        let metadata = fs::symlink_metadata(&file).map_err(|source| AppError::Io {
            path: file.clone(),
            source,
        })?;
        if metadata.file_type().is_file()
            && matches!(
                file.extension().and_then(|extension| extension.to_str()),
                Some("yaml" | "yml")
            )
        {
            paths.push(file);
        }
    }
    paths.sort();
    paths
        .into_iter()
        .map(|file| {
            let content = fs::read_to_string(&file).map_err(|source| AppError::Io {
                path: file.clone(),
                source,
            })?;
            parse(&file, &content)
        })
        .collect()
}

fn create_run_dir(workspace: &Path) -> Result<(String, PathBuf)> {
    let root = workspace.join(".singularity/runs");
    for attempt in 0..32_u8 {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or_default();
        let run_id = format!("scan-{nanos:x}-{:x}-{attempt}", std::process::id());
        let path = root.join(&run_id);
        match fs::create_dir(&path) {
            Ok(()) => return Ok((run_id, path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(AppError::ScanPersist {
                    path,
                    message: error.to_string(),
                });
            }
        }
    }
    Err(AppError::ScanPersist {
        path: root,
        message: "failed to allocate a unique run id".to_string(),
    })
}

fn atomic_write(path: &Path, content: &[u8]) -> Result<()> {
    let temp = path.with_extension(format!("tmp-{}", std::process::id()));
    fs::write(&temp, content).map_err(|error| AppError::ScanPersist {
        path: temp.clone(),
        message: error.to_string(),
    })?;
    fs::rename(&temp, path).map_err(|error| AppError::ScanPersist {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

fn map_scan_error(error: ScanError) -> AppError {
    match error {
        ScanError::NoTrustedEvidence => AppError::NoTrustedEvidence,
        ScanError::UnknownProposal(proposal_id) => AppError::UnknownProposal { proposal_id },
        ScanError::EmptyGoal => AppError::InvalidScanApproval {
            message: error.to_string(),
        },
    }
}
