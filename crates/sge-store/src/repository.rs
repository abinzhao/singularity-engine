use std::{
    fs,
    path::{Path, PathBuf},
};

use git2::{FileMode, ObjectType, Oid, Repository, Signature, Tree};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Revision(String);

impl Revision {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn parse(input: &str) -> Result<Self, StoreError> {
        Oid::from_str(input)?;
        Ok(Self(input.to_string()))
    }
}

pub trait LineageRepository {
    fn snapshot(&self, tree: &Path, metadata: serde_json::Value) -> Result<Revision, StoreError>;

    fn snapshot_candidate(
        &self,
        parent: &Revision,
        tree: &Path,
        metadata: serde_json::Value,
    ) -> Result<Revision, StoreError>;

    fn checkout_candidate(&self, parent: &Revision, target: &Path) -> Result<(), StoreError>;

    fn restore(&self, revision: &Revision, target: &Path) -> Result<(), StoreError>;

    fn verify(&self) -> Result<VerificationReport, StoreError>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerificationReport {
    pub is_bare: bool,
    pub object_database_available: bool,
    pub issues: Vec<String>,
}

impl VerificationReport {
    pub fn is_clean(&self) -> bool {
        self.issues.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct GitLineageRepository {
    path: PathBuf,
}

impl GitLineageRepository {
    pub fn init_or_open_bare(path: impl AsRef<Path>) -> Result<Self, StoreError> {
        let path = path.as_ref().to_path_buf();
        if path.exists() {
            Repository::open_bare(&path)?;
        } else {
            Repository::init_bare(&path)?;
        }

        Ok(Self { path })
    }

    fn open(&self) -> Result<Repository, StoreError> {
        Ok(Repository::open_bare(&self.path)?)
    }
}

impl LineageRepository for GitLineageRepository {
    fn snapshot(&self, tree: &Path, metadata: serde_json::Value) -> Result<Revision, StoreError> {
        let repo = self.open()?;
        let tree_id = write_tree_from_directory(&repo, tree)?;
        let tree = repo.find_tree(tree_id)?;
        let signature = Signature::now("Singularity Engine", "sge@local")?;
        let message = serde_json::to_string(&metadata)?;
        let parent = repo
            .find_reference("refs/heads/main")
            .ok()
            .and_then(|reference| reference.target())
            .and_then(|oid| repo.find_commit(oid).ok());

        let commit_id = match parent.as_ref() {
            Some(parent) => repo.commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                &message,
                &tree,
                &[parent],
            )?,
            None => repo.commit(
                Some("refs/heads/main"),
                &signature,
                &signature,
                &message,
                &tree,
                &[],
            )?,
        };

        Ok(Revision(commit_id.to_string()))
    }

    fn snapshot_candidate(
        &self,
        parent: &Revision,
        tree: &Path,
        metadata: serde_json::Value,
    ) -> Result<Revision, StoreError> {
        let repo = self.open()?;
        let tree_id = write_tree_from_directory(&repo, tree)?;
        let tree = repo.find_tree(tree_id)?;
        let parent_id = Oid::from_str(parent.as_str())?;
        let parent_commit = repo.find_commit(parent_id)?;
        let signature = Signature::now("Singularity Engine", "sge@local")?;
        let message = serde_json::to_string(&metadata)?;
        let commit_id = repo.commit(
            None,
            &signature,
            &signature,
            &message,
            &tree,
            &[&parent_commit],
        )?;
        repo.reference(
            &format!("refs/sge/candidates/{commit_id}"),
            commit_id,
            true,
            "record candidate revision",
        )?;

        Ok(Revision(commit_id.to_string()))
    }

    fn checkout_candidate(&self, parent: &Revision, target: &Path) -> Result<(), StoreError> {
        self.restore(parent, target)
    }

    fn restore(&self, revision: &Revision, target: &Path) -> Result<(), StoreError> {
        let repo = self.open()?;
        let oid = Oid::from_str(revision.as_str())?;
        let commit = repo.find_commit(oid)?;
        let tree = commit.tree()?;

        if target.exists() {
            fs::remove_dir_all(target)?;
        }
        fs::create_dir_all(target)?;
        restore_tree(&repo, &tree, target)?;

        Ok(())
    }

    fn verify(&self) -> Result<VerificationReport, StoreError> {
        let repo = self.open()?;
        let mut issues = Vec::new();
        if !repo.is_bare() {
            issues.push("lineage repository must be bare".to_owned());
        }
        let object_database_available = repo.odb().is_ok();
        if !object_database_available {
            issues.push("object database is unavailable".to_owned());
        }

        Ok(VerificationReport {
            is_bare: repo.is_bare(),
            object_database_available,
            issues,
        })
    }
}

fn write_tree_from_directory(repo: &Repository, directory: &Path) -> Result<Oid, StoreError> {
    let mut builder = repo.treebuilder(None)?;
    let mut entries = fs::read_dir(directory)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name();
        let metadata = entry.metadata()?;

        if metadata.is_dir() {
            let tree_id = write_tree_from_directory(repo, &path)?;
            builder.insert(name, tree_id, FileMode::Tree.into())?;
        } else if metadata.is_file() {
            let contents = fs::read(&path)?;
            let blob_id = repo.blob(&contents)?;
            builder.insert(name, blob_id, FileMode::Blob.into())?;
        }
    }

    Ok(builder.write()?)
}

fn restore_tree(repo: &Repository, tree: &Tree<'_>, target: &Path) -> Result<(), StoreError> {
    for entry in tree {
        let Some(name) = entry.name() else {
            return Err(StoreError::RepositoryInvariant {
                message: "tree entry must be valid UTF-8".to_owned(),
            });
        };
        let output = target.join(name);

        match entry.kind() {
            Some(ObjectType::Blob) => {
                let blob = repo.find_blob(entry.id())?;
                fs::write(output, blob.content())?;
            }
            Some(ObjectType::Tree) => {
                fs::create_dir_all(&output)?;
                let subtree = repo.find_tree(entry.id())?;
                restore_tree(repo, &subtree, &output)?;
            }
            _ => {
                return Err(StoreError::RepositoryInvariant {
                    message: format!("unsupported git object in tree: {:?}", entry.kind()),
                });
            }
        }
    }

    Ok(())
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("I/O error: {source}")]
    Io {
        #[from]
        source: std::io::Error,
    },
    #[error("git error: {source}")]
    Git {
        #[from]
        source: git2::Error,
    },
    #[error("JSON error: {source}")]
    Json {
        #[from]
        source: serde_json::Error,
    },
    #[error("repository invariant failed: {message}")]
    RepositoryInvariant { message: String },
    #[error("journal invariant failed: {message}")]
    JournalInvariant { message: String },
}

impl StoreError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Io { .. } => "SGE-STORE-001",
            Self::Git { .. } => "SGE-STORE-002",
            Self::Json { .. } => "SGE-STORE-003",
            Self::RepositoryInvariant { .. } => "SGE-STORE-004",
            Self::JournalInvariant { .. } => "SGE-STORE-005",
        }
    }
}
