pub mod import;
pub mod init;
pub mod scan;
pub mod validate;

use std::path::PathBuf;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("refusing to overwrite existing non-generated singularity.yaml at {path}")]
    NonGeneratedManifest { path: PathBuf },
    #[error("I/O operation failed at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to initialize bare git repository at {path}: {message}")]
    StoreInit { path: PathBuf, message: String },
    #[error("workspace validation failed at {path}: {message}")]
    InvalidWorkspace { path: PathBuf, message: String },
    #[error("failed to parse artifact document at {path}: {source}")]
    InvalidArtifactDoc {
        path: PathBuf,
        source: Box<dyn std::error::Error + Send + Sync>,
    },
    #[error("artifact kind mismatch: expected skill, got {got}")]
    KindMismatch { got: String },
    #[error("artifact name `{name}` already exists in workspace at {path}")]
    DuplicateArtifact { name: String, path: PathBuf },
    #[error("declared file `{declared}` escapes source root")]
    PathTraversal { declared: String },
    #[error("declared file `{declared}` in manifest is missing on disk")]
    MissingDeclaredFile { declared: String },
    #[error("symlink or hardlink detected in declared file `{declared}`; refusing insecure import")]
    SymlinkRefused { declared: String },
    #[error("invalid scan target `{target}`: {message}")]
    InvalidScanTarget { target: String, message: String },
    #[error("scan artifact is missing at {path}")]
    MissingScanArtifact { path: PathBuf },
    #[error("no trusted evidence exists for the scan target")]
    NoTrustedEvidence,
    #[error("invalid evidence document at {path}: {message}")]
    InvalidEvidence { path: PathBuf, message: String },
    #[error("invalid memory document at {path}: {message}")]
    InvalidMemory { path: PathBuf, message: String },
    #[error("unknown proposal `{proposal_id}`")]
    UnknownProposal { proposal_id: String },
    #[error("failed to persist scan output at {path}: {message}")]
    ScanPersist { path: PathBuf, message: String },
    #[error("invalid workspace for scan at {path}: {message}")]
    InvalidScanWorkspace { path: PathBuf, message: String },
    #[error("invalid scan approval options: {message}")]
    InvalidScanApproval { message: String },
}

impl AppError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NonGeneratedManifest { .. } => "SGE-APP-001",
            Self::Io { .. } => "SGE-APP-002",
            Self::StoreInit { .. } => "SGE-STORE-001",
            Self::InvalidWorkspace { .. } => "SGE-APP-003",
            Self::InvalidArtifactDoc { .. } => "SGE-IMPORT-001",
            Self::DuplicateArtifact { .. } => "SGE-IMPORT-002",
            Self::KindMismatch { .. } => "SGE-IMPORT-003",
            Self::PathTraversal { .. } => "SGE-IMPORT-004",
            Self::MissingDeclaredFile { .. } => "SGE-IMPORT-005",
            Self::SymlinkRefused { .. } => "SGE-IMPORT-006",
            Self::InvalidScanTarget { .. } => "SGE-SCAN-001",
            Self::MissingScanArtifact { .. } => "SGE-SCAN-002",
            Self::NoTrustedEvidence => "SGE-SCAN-003",
            Self::InvalidEvidence { .. } => "SGE-SCAN-004",
            Self::InvalidMemory { .. } => "SGE-SCAN-005",
            Self::UnknownProposal { .. } => "SGE-SCAN-006",
            Self::ScanPersist { .. } => "SGE-SCAN-007",
            Self::InvalidScanWorkspace { .. } => "SGE-SCAN-008",
            Self::InvalidScanApproval { .. } => "SGE-SCAN-009",
        }
    }
}

pub type Result<T> = std::result::Result<T, AppError>;
