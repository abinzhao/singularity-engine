use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ArtifactDocument {
    pub schema: String,
    pub id: String,
    pub kind: String,
    pub name: String,
    pub title: String,
    pub summary: String,
    pub body: String,
    #[serde(flatten)]
    pub extensions: BTreeMap<String, Value>,
}
