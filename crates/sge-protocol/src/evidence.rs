use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct EvidenceDocument {
    pub schema: String,
    pub id: String,
    pub target: String,
    pub claim: String,
    pub source: String,
    pub status: String,
    #[serde(default)]
    pub details: BTreeMap<String, Value>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}
