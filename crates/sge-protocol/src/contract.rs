use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ContractDocument {
    pub schema: String,
    pub id: String,
    pub target: String,
    pub intent: String,
    #[serde(default)]
    pub inputs: Vec<String>,
    #[serde(default)]
    pub outputs: Vec<String>,
    #[serde(default)]
    pub success: Vec<String>,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}
