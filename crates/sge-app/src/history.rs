use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sge_store::{GitLineageRepository, LineageRepository, Revision};
use walkdir::WalkDir;

use crate::{AppError, Result, replay::ReplayDocument};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HistoryEntry {
    pub run_id: String,
    pub target: String,
    pub selected_candidate: Option<String>,
    pub selected_revision: Option<String>,
}

pub fn history_target(workspace: impl AsRef<Path>, target: &str) -> Result<Vec<HistoryEntry>> {
    let runs_dir = workspace.as_ref().join(".singularity/runs");
    if !runs_dir.is_dir() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for entry in fs::read_dir(&runs_dir).map_err(|source| AppError::Io {
        path: runs_dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| AppError::Io {
            path: runs_dir.clone(),
            source,
        })?;
        let replay_path = entry.path().join("replay.yaml");
        if !replay_path.is_file() {
            continue;
        }
        let replay: ReplayDocument =
            serde_yaml::from_str(&fs::read_to_string(&replay_path).map_err(|source| {
                AppError::Io {
                    path: replay_path.clone(),
                    source,
                }
            })?)
            .map_err(|error| AppError::Evolution {
                path: replay_path,
                message: error.to_string(),
            })?;
        if replay.target == target {
            entries.push(HistoryEntry {
                run_id: replay.run_id,
                target: replay.target,
                selected_candidate: replay.selected_candidate,
                selected_revision: replay.selected_revision,
            });
        }
    }
    entries.sort_by(|a, b| a.run_id.cmp(&b.run_id));
    Ok(entries)
}

pub fn diff_revisions(
    workspace: impl AsRef<Path>,
    revision_a: &str,
    revision_b: &str,
) -> Result<String> {
    let workspace = workspace.as_ref();
    let repository = GitLineageRepository::init_or_open_bare(
        workspace.join(".singularity/repo.git"),
    )
    .map_err(|error| AppError::Evolution {
        path: workspace.to_path_buf(),
        message: error.to_string(),
    })?;
    let left = tempfile::tempdir().map_err(|source| AppError::Io {
        path: std::env::temp_dir(),
        source,
    })?;
    let right = tempfile::tempdir().map_err(|source| AppError::Io {
        path: std::env::temp_dir(),
        source,
    })?;
    let revision_a = Revision::parse(revision_a).map_err(|error| AppError::Evolution {
        path: workspace.to_path_buf(),
        message: error.to_string(),
    })?;
    let revision_b = Revision::parse(revision_b).map_err(|error| AppError::Evolution {
        path: workspace.to_path_buf(),
        message: error.to_string(),
    })?;
    repository
        .restore(&revision_a, left.path())
        .map_err(|error| AppError::Evolution {
            path: workspace.to_path_buf(),
            message: error.to_string(),
        })?;
    repository
        .restore(&revision_b, right.path())
        .map_err(|error| AppError::Evolution {
            path: workspace.to_path_buf(),
            message: error.to_string(),
        })?;

    render_directory_diff(left.path(), right.path())
}

fn render_directory_diff(left: &Path, right: &Path) -> Result<String> {
    let mut paths = relative_files(left)?;
    paths.extend(relative_files(right)?);
    let mut output = String::new();
    for relative in paths {
        let left_content = read_optional_text(&left.join(&relative))?;
        let right_content = read_optional_text(&right.join(&relative))?;
        if left_content == right_content {
            continue;
        }
        output.push_str(&format!(
            "--- a/{}\n+++ b/{}\n",
            relative.display(),
            relative.display()
        ));
        if let Some(content) = left_content {
            for line in content.lines() {
                output.push_str(&format!("-{line}\n"));
            }
        }
        if let Some(content) = right_content {
            for line in content.lines() {
                output.push_str(&format!("+{line}\n"));
            }
        }
    }
    Ok(output)
}

fn relative_files(root: &Path) -> Result<BTreeSet<PathBuf>> {
    let mut paths = BTreeSet::new();
    for entry in WalkDir::new(root) {
        let entry = entry.map_err(|error| AppError::Evolution {
            path: root.to_path_buf(),
            message: error.to_string(),
        })?;
        if entry.file_type().is_file() {
            paths.insert(
                entry
                    .path()
                    .strip_prefix(root)
                    .expect("walked entry must be under root")
                    .to_path_buf(),
            );
        }
    }
    Ok(paths)
}

fn read_optional_text(path: &Path) -> Result<Option<String>> {
    if !path.is_file() {
        return Ok(None);
    }
    fs::read_to_string(path)
        .map(Some)
        .map_err(|source| AppError::Io {
            path: path.to_path_buf(),
            source,
        })
}
