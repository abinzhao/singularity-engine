use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Proposal {
    pub id: String,
    pub title: String,
    pub risk: String,
    pub affected_files: Vec<String>,
    pub confidence: f64,
    pub evidence_refs: Vec<String>,
    pub estimated_improvement: BTreeMap<String, [f64; 2]>,
    pub evaluation_method: String,
    pub estimate_basis: String,
}
