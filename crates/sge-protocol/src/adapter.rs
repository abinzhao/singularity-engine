use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct AdapterDocument {
    pub schema: String,
    pub id: String,
    pub target: String,
    pub runtime: String,
    pub command: String,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}
