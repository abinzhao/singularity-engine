use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExpectedFinding {
    pub severity: Severity,
    pub category: String,
    pub message_contains: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Case {
    pub id: String,
    pub title: Option<String>,
    pub prompt: String,
    pub expected_findings: Vec<ExpectedFinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FindingMatch {
    pub expected_index: usize,
    pub matched: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CaseResult {
    pub case_id: String,
    pub findings: Vec<FindingMatch>,
    pub actual_severity_counts: BTreeMap<Severity, usize>,
    pub latency_ms: u64,
    pub tokens_used: u64,
}

impl CaseResult {
    pub fn fraction_matched(&self) -> f64 {
        if self.findings.is_empty() {
            return 1.0;
        }
        self.findings.iter().filter(|f| f.matched).count() as f64 / self.findings.len() as f64
    }

    pub fn all_matched(&self) -> bool {
        self.findings.iter().all(|f| f.matched)
    }
}
