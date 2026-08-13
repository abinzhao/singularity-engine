use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use sge_protocol::{Document, parse_document};
use sge_store::{GitLineageRepository, LineageRepository};

use crate::{AppError, Result};

#[derive(Debug, Clone)]
pub struct ImportedArtifact {
    pub target: String,
    pub revision: String,
    pub warnings: Vec<String>,
}

struct DeclaredFile {
    path: String,
}

fn parse_declared_files(files_value: Option<&Value>) -> Vec<DeclaredFile> {
    match files_value {
        None => vec![DeclaredFile {
            path: "instructions.md".to_string(),
        }],
        Some(Value::Array(arr)) => arr
            .iter()
            .filter_map(|item| match item {
                Value::String(s) => Some(DeclaredFile { path: s.clone() }),
                Value::Object(obj) => {
                    obj.get("path")
                        .and_then(|p| p.as_str())
                        .map(|p| DeclaredFile {
                            path: p.to_string(),
                        })
                }
                _ => None,
            })
            .collect(),
        Some(_) => vec![DeclaredFile {
            path: "instructions.md".to_string(),
        }],
    }
}

fn io_err(path: PathBuf, source: std::io::Error) -> AppError {
    AppError::Io { path, source }
}

pub fn import_artifact(
    workspace_root: impl AsRef<Path>,
    source_dir: impl AsRef<Path>,
) -> Result<ImportedArtifact> {
    let workspace_root = workspace_root.as_ref();
    let source_dir = source_dir.as_ref();

    let manifest_path = source_dir.join("skill.yaml");
    let manifest_content =
        fs::read_to_string(&manifest_path).map_err(|e| io_err(manifest_path.clone(), e))?;

    let doc = parse_document(&manifest_content).map_err(|e| AppError::InvalidArtifactDoc {
        path: manifest_path.clone(),
        source: Box::new(e),
    })?;

    let artifact = match doc {
        Document::Artifact(a) => a,
        _ => {
            return Err(AppError::InvalidArtifactDoc {
                path: manifest_path,
                source: "document is not an ArtifactDocument".into(),
            });
        }
    };

    if artifact.kind != "skill" {
        return Err(AppError::KindMismatch { got: artifact.kind });
    }

    let name = artifact.name.clone();
    let target_dir = workspace_root.join("skills").join(&name);

    if target_dir.exists() {
        return Err(AppError::DuplicateArtifact {
            name: name.clone(),
            path: target_dir,
        });
    }

    let files_value = artifact.extensions.get("files");
    let declared_files = parse_declared_files(files_value);

    let source_canonical = source_dir
        .canonicalize()
        .map_err(|e| io_err(source_dir.to_path_buf(), e))?;

    for declared in &declared_files {
        let declared_path = source_dir.join(&declared.path);

        let sym_meta = fs::symlink_metadata(&declared_path).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                AppError::MissingDeclaredFile {
                    declared: declared.path.clone(),
                }
            } else {
                io_err(declared_path.clone(), e)
            }
        })?;

        if sym_meta.file_type().is_symlink() {
            return Err(AppError::SymlinkRefused {
                declared: declared.path.clone(),
            });
        }

        let resolved = declared_path
            .canonicalize()
            .map_err(|e| io_err(declared_path.clone(), e))?;

        if !resolved.starts_with(&source_canonical) {
            return Err(AppError::PathTraversal {
                declared: declared.path.clone(),
            });
        }
    }

    let cache_dir = workspace_root.join(".singularity").join("cache");
    fs::create_dir_all(&cache_dir).map_err(|e| io_err(cache_dir.clone(), e))?;

    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos().to_string())
        .unwrap_or_else(|_| "0".to_string());
    let temp_dir = cache_dir.join(format!("import-{nonce}"));
    fs::create_dir_all(&temp_dir).map_err(|e| io_err(temp_dir.clone(), e))?;

    let manifest_dest = temp_dir.join("skill.yaml");
    fs::copy(&manifest_path, &manifest_dest).map_err(|e| io_err(manifest_dest, e))?;

    for declared in &declared_files {
        let src = source_dir.join(&declared.path);
        let dst = temp_dir.join(&declared.path);
        if let Some(parent) = dst.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent).map_err(|e| io_err(parent.to_path_buf(), e))?;
        }
        fs::copy(&src, &dst).map_err(|e| io_err(dst, e))?;
    }

    if let Some(parent) = target_dir.parent() {
        fs::create_dir_all(parent).map_err(|e| io_err(parent.to_path_buf(), e))?;
    }
    fs::rename(&temp_dir, &target_dir).map_err(|e| io_err(temp_dir.clone(), e))?;

    let store_path = workspace_root.join(".singularity").join("repo.git");
    let repo =
        GitLineageRepository::init_or_open_bare(&store_path).map_err(|e| AppError::StoreInit {
            path: store_path.clone(),
            message: e.to_string(),
        })?;

    let metadata = serde_json::json!({
        "op": "import",
        "target": format!("skill:{name}"),
    });

    let revision = repo
        .snapshot(&target_dir, metadata)
        .map_err(|e| AppError::StoreInit {
            path: target_dir.clone(),
            message: e.to_string(),
        })?;

    Ok(ImportedArtifact {
        target: format!("skill:{name}"),
        revision: revision.as_str().to_string(),
        warnings: Vec::new(),
    })
}
