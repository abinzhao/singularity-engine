use std::{
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sge_domain::TargetRef;
use sge_store::{
    AppendOnlyJournal, GitLineageRepository, JournalEntry, JournalState, LineageRepository,
    Revision,
};
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::{
    AppError, Result,
    explain::validate_run_id,
    replay::{ReplayDocument, replay_run},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyFault {
    AfterBackup,
}

#[derive(Debug, Clone)]
pub struct ApplyOptions {
    pub approved: bool,
    pub fault: Option<ApplyFault>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyRecord {
    pub run_id: String,
    pub target: String,
    pub previous_revision: String,
    pub selected_revision: String,
    pub applied_revision: String,
}

#[derive(Debug, Clone)]
pub struct ApplyOutcome {
    pub run_id: String,
    pub target: String,
    pub previous_revision: String,
    pub applied_revision: String,
    pub record_path: PathBuf,
}

pub fn apply_run(
    workspace: impl AsRef<Path>,
    run_id: &str,
    options: ApplyOptions,
) -> Result<ApplyOutcome> {
    if !options.approved {
        return Err(apply_error(
            Path::new(run_id),
            "explicit approval is required",
        ));
    }
    validate_run_id(run_id)?;
    let workspace = workspace.as_ref();
    let run_dir = workspace.join(".singularity/runs").join(run_id);
    let journal_path = run_dir.join("journal.ndjson");
    require_last_state(&journal_path, JournalState::ReviewPending)?;
    let replay = read_replay(&run_dir)?;
    let replay_outcome = replay_run(workspace, run_id)?;
    if !replay_outcome.matches {
        return Err(apply_error(
            &run_dir,
            format!(
                "replay verification failed: {}",
                replay_outcome.mismatches.join(", ")
            ),
        ));
    }
    let selected_revision = replay
        .selected_revision
        .clone()
        .ok_or_else(|| apply_error(&run_dir, "run has no selected revision"))?;
    let target: TargetRef = replay
        .target
        .parse::<TargetRef>()
        .map_err(|error| apply_error(&run_dir, error.to_string()))?;
    let standard_dir = workspace.join("skills").join(target.name());
    let repository =
        GitLineageRepository::init_or_open_bare(workspace.join(".singularity/repo.git"))
            .map_err(|error| apply_error(workspace, error.to_string()))?;

    verify_revision_matches_directory(
        &repository,
        &Revision::parse(&replay.baseline_revision)
            .map_err(|error| apply_error(&run_dir, error.to_string()))?,
        &standard_dir,
    )?;
    let journal = AppendOnlyJournal::open(&journal_path)
        .map_err(|error| apply_error(&journal_path, error.to_string()))?;
    journal
        .append(
            JournalState::Applying,
            serde_json::json!({
                "operation": "apply",
                "selected_revision": selected_revision,
            }),
        )
        .map_err(|error| apply_error(&journal_path, error.to_string()))?;

    let selected = Revision::parse(&selected_revision)
        .map_err(|error| apply_error(&run_dir, error.to_string()))?;
    if let Err(error) =
        replace_directory_from_revision(&repository, &selected, &standard_dir, options.fault)
    {
        journal
            .append(
                JournalState::ReviewPending,
                serde_json::json!({"operation": "apply", "rolled_back": true}),
            )
            .map_err(|journal_error| apply_error(&journal_path, journal_error.to_string()))?;
        return Err(error);
    }

    let applied_revision = match repository.snapshot(
        &standard_dir,
        serde_json::json!({
            "op": "apply",
            "run_id": run_id,
            "target": replay.target,
            "source_revision": selected_revision,
        }),
    ) {
        Ok(revision) => revision,
        Err(error) => {
            let baseline = Revision::parse(&replay.baseline_revision)
                .map_err(|parse_error| apply_error(&run_dir, parse_error.to_string()))?;
            replace_directory_from_revision(&repository, &baseline, &standard_dir, None)?;
            journal
                .append(
                    JournalState::ReviewPending,
                    serde_json::json!({"operation": "apply", "rolled_back": true}),
                )
                .map_err(|journal_error| apply_error(&journal_path, journal_error.to_string()))?;
            return Err(apply_error(&standard_dir, error.to_string()));
        }
    };

    let record = ApplyRecord {
        run_id: run_id.to_string(),
        target: replay.target,
        previous_revision: replay.baseline_revision,
        selected_revision,
        applied_revision: applied_revision.as_str().to_string(),
    };
    let record_path = run_dir.join("apply.json");
    write_json_atomic(&record_path, &record)?;
    journal
        .append(
            JournalState::Completed,
            serde_json::json!({
                "operation": "apply",
                "applied_revision": applied_revision.as_str(),
                "record": record_path,
            }),
        )
        .map_err(|error| apply_error(&journal_path, error.to_string()))?;

    Ok(ApplyOutcome {
        run_id: run_id.to_string(),
        target: record.target,
        previous_revision: record.previous_revision,
        applied_revision: record.applied_revision,
        record_path,
    })
}

pub(crate) fn replace_directory_from_revision(
    repository: &GitLineageRepository,
    revision: &Revision,
    standard_dir: &Path,
    fault: Option<ApplyFault>,
) -> Result<()> {
    let parent = standard_dir
        .parent()
        .ok_or_else(|| apply_error(standard_dir, "standard directory has no parent"))?;
    let name = standard_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| apply_error(standard_dir, "standard directory name is not UTF-8"))?;
    let stage = parent.join(format!(".{name}.sge-stage"));
    let backup = parent.join(format!(".{name}.sge-backup"));
    if stage.exists() || backup.exists() {
        return Err(apply_error(
            parent,
            "stale apply staging or backup directory exists",
        ));
    }

    repository
        .restore(revision, &stage)
        .map_err(|error| apply_error(&stage, error.to_string()))?;
    sync_tree(&stage)?;
    fs::rename(standard_dir, &backup).map_err(|source| AppError::Io {
        path: standard_dir.to_path_buf(),
        source,
    })?;
    sync_directory(parent)?;

    if fault == Some(ApplyFault::AfterBackup) {
        rollback_backup(&stage, &backup, standard_dir, parent)?;
        return Err(apply_error(standard_dir, "injected failure after backup"));
    }

    if let Err(source) = fs::rename(&stage, standard_dir) {
        rollback_backup(&stage, &backup, standard_dir, parent)?;
        return Err(AppError::Io {
            path: standard_dir.to_path_buf(),
            source,
        });
    }
    sync_directory(parent)?;
    fs::remove_dir_all(&backup).map_err(|source| AppError::Io {
        path: backup.clone(),
        source,
    })?;
    sync_directory(parent)?;
    Ok(())
}

pub(crate) fn read_apply_record(run_dir: &Path) -> Result<ApplyRecord> {
    let path = run_dir.join("apply.json");
    serde_json::from_str(&fs::read_to_string(&path).map_err(|source| AppError::Io {
        path: path.clone(),
        source,
    })?)
    .map_err(|error| apply_error(path, error.to_string()))
}

pub(crate) fn read_replay(run_dir: &Path) -> Result<ReplayDocument> {
    let path = run_dir.join("replay.yaml");
    serde_yaml::from_str(&fs::read_to_string(&path).map_err(|source| AppError::Io {
        path: path.clone(),
        source,
    })?)
    .map_err(|error| apply_error(path, error.to_string()))
}

pub(crate) fn verify_revision_matches_directory(
    repository: &GitLineageRepository,
    revision: &Revision,
    directory: &Path,
) -> Result<()> {
    let expected = tempfile::tempdir().map_err(|source| AppError::Io {
        path: std::env::temp_dir(),
        source,
    })?;
    repository
        .restore(revision, expected.path())
        .map_err(|error| apply_error(directory, error.to_string()))?;
    if directory_digest(expected.path())? != directory_digest(directory)? {
        return Err(apply_error(
            directory,
            "standard Skill changed since the evolution baseline",
        ));
    }
    Ok(())
}

fn require_last_state(path: &Path, expected: JournalState) -> Result<()> {
    let file = File::open(path).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    let mut last = None;
    for line in BufReader::new(file).lines() {
        let line = line.map_err(|source| AppError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        if !line.trim().is_empty() {
            last = Some(line);
        }
    }
    let last = last.ok_or_else(|| apply_error(path, "journal is empty"))?;
    let entry: JournalEntry =
        serde_json::from_str(&last).map_err(|error| apply_error(path, error.to_string()))?;
    if entry.state != expected {
        return Err(apply_error(
            path,
            format!("run must be in {expected:?}, found {:?}", entry.state),
        ));
    }
    Ok(())
}

fn rollback_backup(stage: &Path, backup: &Path, standard: &Path, parent: &Path) -> Result<()> {
    if standard.exists() {
        fs::remove_dir_all(standard).map_err(|source| AppError::Io {
            path: standard.to_path_buf(),
            source,
        })?;
    }
    fs::rename(backup, standard).map_err(|source| AppError::Io {
        path: backup.to_path_buf(),
        source,
    })?;
    if stage.exists() {
        fs::remove_dir_all(stage).map_err(|source| AppError::Io {
            path: stage.to_path_buf(),
            source,
        })?;
    }
    sync_directory(parent)
}

fn directory_digest(root: &Path) -> Result<String> {
    let mut entries = WalkDir::new(root)
        .into_iter()
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|error| apply_error(root, error.to_string()))?;
    entries.sort_by_key(|entry| entry.path().to_path_buf());
    let mut hasher = Sha256::new();
    for entry in entries {
        let relative = entry
            .path()
            .strip_prefix(root)
            .map_err(|error| apply_error(root, error.to_string()))?;
        if relative.as_os_str().is_empty() {
            continue;
        }
        let metadata = fs::symlink_metadata(entry.path()).map_err(|source| AppError::Io {
            path: entry.path().to_path_buf(),
            source,
        })?;
        if metadata.file_type().is_symlink() {
            return Err(apply_error(entry.path(), "symlink is not allowed"));
        }
        hasher.update(relative.to_string_lossy().as_bytes());
        if metadata.is_file() {
            hasher.update([0x01]);
            hasher.update(fs::read(entry.path()).map_err(|source| AppError::Io {
                path: entry.path().to_path_buf(),
                source,
            })?);
        } else if metadata.is_dir() {
            hasher.update([0x02]);
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn sync_tree(root: &Path) -> Result<()> {
    let mut directories = Vec::new();
    for entry in WalkDir::new(root) {
        let entry = entry.map_err(|error| apply_error(root, error.to_string()))?;
        if entry.file_type().is_file() {
            File::open(entry.path())
                .and_then(|file| file.sync_all())
                .map_err(|source| AppError::Io {
                    path: entry.path().to_path_buf(),
                    source,
                })?;
        } else if entry.file_type().is_dir() {
            directories.push(entry.path().to_path_buf());
        }
    }
    directories.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    for directory in directories {
        sync_directory(&directory)?;
    }
    Ok(())
}

fn sync_directory(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|file| file.sync_all())
        .map_err(|source| AppError::Io {
            path: path.to_path_buf(),
            source,
        })
}

fn write_json_atomic(path: &Path, value: &impl Serialize) -> Result<()> {
    let temporary = path.with_extension("json.tmp");
    let bytes =
        serde_json::to_vec_pretty(value).map_err(|error| apply_error(path, error.to_string()))?;
    fs::write(&temporary, bytes).map_err(|source| AppError::Io {
        path: temporary.clone(),
        source,
    })?;
    File::open(&temporary)
        .and_then(|file| file.sync_all())
        .map_err(|source| AppError::Io {
            path: temporary.clone(),
            source,
        })?;
    fs::rename(&temporary, path).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    if let Some(parent) = path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

pub(crate) fn apply_error(path: impl AsRef<Path>, message: impl Into<String>) -> AppError {
    AppError::Evolution {
        path: path.as_ref().to_path_buf(),
        message: message.into(),
    }
}
