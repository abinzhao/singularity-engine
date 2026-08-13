use thiserror::Error;

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("provider timeout: budget {budget_ms}ms, observed {observed_ms}ms")]
    Timeout { budget_ms: u64, observed_ms: u64 },
    #[error("budget exceeded: {kind} max={max}, used={used}")]
    BudgetExceeded {
        kind: &'static str,
        max: u64,
        used: u64,
    },
    #[error("provider response failed schema validation: {message}")]
    SchemaValidationFailed { message: String },
    #[error("failed to deserialize provider response: {message}")]
    DeserializationFailed { message: String },
    #[error("no recorded entry for signature `{signature}`")]
    NoRecordedEntry { signature: String },
    #[error("provider I/O error: {message}")]
    Io { message: String },
}

pub type TransportResult<T> = Result<T, ProviderError>;

impl ProviderError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::Timeout { .. } => "SGE-PROVIDER-001",
            Self::BudgetExceeded { .. } => "SGE-PROVIDER-002",
            Self::SchemaValidationFailed { .. } => "SGE-PROVIDER-003",
            Self::DeserializationFailed { .. } => "SGE-PROVIDER-004",
            Self::NoRecordedEntry { .. } => "SGE-PROVIDER-005",
            Self::Io { .. } => "SGE-PROVIDER-006",
        }
    }
}
