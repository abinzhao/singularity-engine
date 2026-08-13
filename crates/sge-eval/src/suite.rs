use crate::case::Case;
use crate::metrics::Objective;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SuiteParseError {
    #[error("yaml parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("unsupported schema: {0}")]
    UnsupportedSchema(String),
    #[error("missing required field: {0}")]
    MissingField(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Suite {
    pub id: String,
    pub target: String,
    pub objective: Objective,
    pub cases: Vec<Case>,
}

#[derive(Debug, Deserialize)]
struct ContractV1Doc {
    schema: Option<String>,
    id: Option<String>,
    target: Option<String>,
    objective: Option<Objective>,
    cases: Option<Vec<Case>>,
    #[serde(flatten)]
    #[allow(dead_code)]
    extra: BTreeMap<String, serde_yaml::Value>,
}

impl Suite {
    pub fn from_yaml(s: &str) -> Result<Suite, SuiteParseError> {
        let doc: ContractV1Doc = serde_yaml::from_str(s)?;
        if let Some(schema) = &doc.schema {
            if schema == "sge.dev/contract/v1" {
                return Self::from_contract_v1(doc);
            }
            return Err(SuiteParseError::UnsupportedSchema(schema.clone()));
        }
        Self::from_plain(doc)
    }

    fn from_contract_v1(doc: ContractV1Doc) -> Result<Suite, SuiteParseError> {
        let id = doc
            .id
            .ok_or_else(|| SuiteParseError::MissingField("id".to_string()))?;
        let target = doc
            .target
            .ok_or_else(|| SuiteParseError::MissingField("target".to_string()))?;
        let objective = doc
            .objective
            .ok_or_else(|| SuiteParseError::MissingField("objective".to_string()))?;
        let cases = doc
            .cases
            .ok_or_else(|| SuiteParseError::MissingField("cases".to_string()))?;
        Ok(Suite {
            id,
            target,
            objective,
            cases,
        })
    }

    fn from_plain(doc: ContractV1Doc) -> Result<Suite, SuiteParseError> {
        let id = doc
            .id
            .ok_or_else(|| SuiteParseError::MissingField("id".to_string()))?;
        let target = doc
            .target
            .ok_or_else(|| SuiteParseError::MissingField("target".to_string()))?;
        let objective = doc
            .objective
            .ok_or_else(|| SuiteParseError::MissingField("objective".to_string()))?;
        let cases = doc
            .cases
            .ok_or_else(|| SuiteParseError::MissingField("cases".to_string()))?;
        Ok(Suite {
            id,
            target,
            objective,
            cases,
        })
    }
}
