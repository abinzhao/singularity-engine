pub mod adapter;
pub mod artifact;
pub mod contract;
pub mod evidence;
pub mod memory;
pub mod schemas;
pub mod version;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub use adapter::AdapterDocument;
pub use artifact::ArtifactDocument;
pub use contract::ContractDocument;
pub use evidence::EvidenceDocument;
pub use memory::MemoryDocument;
pub use version::{
    ADAPTER_SCHEMA_V1, ADAPTER_V1, ARTIFACT_SCHEMA_V1, ARTIFACT_V1, CONTRACT_SCHEMA_V1,
    CONTRACT_V1, EVIDENCE_SCHEMA_V1, EVIDENCE_V1, MEMORY_SCHEMA_V1, MEMORY_V1,
};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Document {
    Artifact(ArtifactDocument),
    Contract(ContractDocument),
    Evidence(EvidenceDocument),
    Memory(MemoryDocument),
    Adapter(AdapterDocument),
}

pub fn parse_document(input: &str) -> Result<Document, ProtocolError> {
    let envelope: SchemaEnvelope = serde_yaml::from_str(input)?;

    match envelope.schema.as_str() {
        ARTIFACT_SCHEMA_V1 => Ok(Document::Artifact(serde_yaml::from_str(input)?)),
        CONTRACT_SCHEMA_V1 => Ok(Document::Contract(serde_yaml::from_str(input)?)),
        EVIDENCE_SCHEMA_V1 => Ok(Document::Evidence(serde_yaml::from_str(input)?)),
        MEMORY_SCHEMA_V1 => Ok(Document::Memory(serde_yaml::from_str(input)?)),
        ADAPTER_SCHEMA_V1 => Ok(Document::Adapter(serde_yaml::from_str(input)?)),
        schema => Err(ProtocolError::UnsupportedSchema {
            schema: schema.to_owned(),
        }),
    }
}

#[derive(Debug, Deserialize)]
struct SchemaEnvelope {
    schema: String,
}

#[derive(Debug, Error)]
pub enum ProtocolError {
    #[error("unsupported protocol schema `{schema}`")]
    UnsupportedSchema { schema: String },
    #[error(transparent)]
    Yaml(#[from] serde_yaml::Error),
}

impl PartialEq for ProtocolError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (
                Self::UnsupportedSchema { schema: left },
                Self::UnsupportedSchema { schema: right },
            ) => left == right,
            (Self::Yaml(left), Self::Yaml(right)) => left.to_string() == right.to_string(),
            _ => false,
        }
    }
}
