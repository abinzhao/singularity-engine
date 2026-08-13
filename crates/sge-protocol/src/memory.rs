use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct MemoryDocument {
    pub schema: String,
    pub id: String,
    pub target: String,
    pub summary: String,
    pub content: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}
