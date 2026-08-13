use crate::case::{Case, CaseResult, ExpectedFinding, FindingMatch, Severity};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum GraderError {
    #[error("grader error: {0}")]
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActualFinding {
    pub category: String,
    pub severity: Severity,
    pub message: Option<String>,
}

pub trait DeterministicGraderLike {
    fn grade_case(&self, case: &Case, actual_findings: &[ActualFinding]) -> CaseResult;
}

#[derive(Debug, Clone, Copy)]
pub struct DeterministicGrader;

impl DeterministicGraderLike for DeterministicGrader {
    fn grade_case(&self, case: &Case, actual_findings: &[ActualFinding]) -> CaseResult {
        DeterministicGrader::grade_case(case, actual_findings)
    }
}

impl DeterministicGrader {
    pub fn grade_case(case: &Case, actual_findings: &[ActualFinding]) -> CaseResult {
        let mut findings = Vec::with_capacity(case.expected_findings.len());
        for (idx, expected) in case.expected_findings.iter().enumerate() {
            let matched = actual_findings
                .iter()
                .any(|actual| Self::finding_matches(expected, actual));
            findings.push(FindingMatch {
                expected_index: idx,
                matched,
            });
        }

        let mut actual_severity_counts = BTreeMap::new();
        for actual in actual_findings {
            *actual_severity_counts.entry(actual.severity).or_insert(0) += 1;
        }

        CaseResult {
            case_id: case.id.clone(),
            findings,
            actual_severity_counts,
            latency_ms: 0,
            tokens_used: 0,
        }
    }

    fn finding_matches(expected: &ExpectedFinding, actual: &ActualFinding) -> bool {
        if expected.category != actual.category {
            return false;
        }

        if actual.severity < expected.severity {
            return false;
        }

        if let Some(needle) = &expected.message_contains {
            match &actual.message {
                Some(msg) if msg.contains(needle) => {}
                _ => return false,
            }
        }

        true
    }
}
