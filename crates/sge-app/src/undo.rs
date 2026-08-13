use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sge_domain::TargetRef;
use sge_store::{GitLineageRepository, LineageRepository, Revision};

use crate::{
    AppError, Result,
    apply::{
        apply_error, read_apply_record, replace_directory_from_revision,
        verify_revision_matches_directory,
    },
    explain::validate_run_id,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoRecord {
    pub source: String,
    pub target: String,
    pub restored_revision: String,
    pub restoration_revision: String,
}

#[derive(Debug, Clone)]
pub struct UndoOutcome {
    pub source: String,
    pub target: String,
    pub restored_revision: String,
    pub restoration_revision: String,
    pub record_path: PathBuf,
}

pub fn undo_run(workspace: impl AsRef<Path>, run_id: &str) -> Result<UndoOutcome> {
    validate_run_id(run_id)?;
    let workspace = workspace.as_ref();
    let run_dir = workspace.join(".singularity/runs").join(run_id);
    let apply = read_apply_record(&run_dir)?;
    restore_as_new_revision(
        workspace,
        run_id,
        &apply.target,
        &apply.previous_revision,
        Some(&apply.applied_revision),
        run_dir.join("undo.json"),
    )
}

pub fn undo_revision(
    workspace: impl AsRef<Path>,
    target: &str,
    revision: &str,
) -> Result<UndoOutcome> {
    let workspace = workspace.as_ref();
    restore_as_new_revision(
        workspace,
        revision,
        target,
        revision,
        None,
        workspace
            .join(".singularity/runs")
            .join(format!("undo-{}", &revision[..revision.len().min(12)]))
            .join("undo.json"),
    )
}

fn restore_as_new_revision(
    workspace: &Path,
    source: &str,
    target: &str,
    revision: &str,
    expected_current: Option<&str>,
    record_path: PathBuf,
) -> Result<UndoOutcome> {
    let target_ref: TargetRef = target
        .parse::<TargetRef>()
        .map_err(|error| apply_error(workspace, error.to_string()))?;
    let standard_dir = workspace.join("skills").join(target_ref.name());
    let repository =
        GitLineageRepository::init_or_open_bare(workspace.join(".singularity/repo.git"))
            .map_err(|error| apply_error(workspace, error.to_string()))?;
    if let Some(expected) = expected_current {
        verify_revision_matches_directory(
            &repository,
            &Revision::parse(expected)
                .map_err(|error| apply_error(&standard_dir, error.to_string()))?,
            &standard_dir,
        )?;
    }
    let restored =
        Revision::parse(revision).map_err(|error| apply_error(&standard_dir, error.to_string()))?;
    replace_directory_from_revision(&repository, &restored, &standard_dir, None)?;
    let restoration = repository
        .snapshot(
            &standard_dir,
            serde_json::json!({
                "op": "undo",
                "source": source,
                "target": target,
                "restored_revision": revision,
            }),
        )
        .map_err(|error| apply_error(&standard_dir, error.to_string()))?;
    let record = UndoRecord {
        source: source.to_string(),
        target: target.to_string(),
        restored_revision: revision.to_string(),
        restoration_revision: restoration.as_str().to_string(),
    };
    if let Some(parent) = record_path.parent() {
        fs::create_dir_all(parent).map_err(|source| AppError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(
        &record_path,
        serde_json::to_vec_pretty(&record)
            .map_err(|error| apply_error(&record_path, error.to_string()))?,
    )
    .map_err(|source| AppError::Io {
        path: record_path.clone(),
        source,
    })?;
    Ok(UndoOutcome {
        source: record.source,
        target: record.target,
        restored_revision: record.restored_revision,
        restoration_revision: record.restoration_revision,
        record_path,
    })
}
