use crate::case::{Case, CaseResult, Severity};
use crate::grader::{ActualFinding, DeterministicGraderLike};
use crate::metrics::MetricVector;
use crate::suite::Suite;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("runner error: {0}")]
    Other(String),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMeta {
    pub timestamp_secs: u64,
    pub workspace_path: String,
    pub env_vars: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NormalizedSnapshot {
    pub bytes: Vec<u8>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationReport {
    pub metrics: MetricVector,
    pub case_results: Vec<CaseResult>,
    pub normalized_replay_hash: String,
    pub normalized_snapshot_bytes: Vec<u8>,
}

pub struct SuiteRunner<G> {
    pub grader: G,
    pub seed: [u8; 8],
}

impl<G> SuiteRunner<G> {
    pub fn with_seed(mut self, seed: [u8; 8]) -> Self {
        self.seed = seed;
        self
    }
}

impl<G: DeterministicGraderLike> SuiteRunner<G> {
    pub fn new(grader: G) -> Self {
        Self {
            grader,
            seed: *b"sge-seed",
        }
    }

    pub fn evaluate<F>(
        &self,
        suite: &Suite,
        cases_sorted_for_run: &[&Case],
        actuals_provider: F,
        _run_metadata: &RunMeta,
    ) -> EvaluationReport
    where
        F: Fn(&Case) -> (Vec<ActualFinding>, u64, u64),
    {
        let mut case_results: Vec<CaseResult> = Vec::with_capacity(cases_sorted_for_run.len());
        for case in cases_sorted_for_run {
            let (actuals, latency_ms, tokens_used) = actuals_provider(case);
            let mut result = self.grader.grade_case(case, &actuals);
            result.latency_ms = latency_ms;
            result.tokens_used = tokens_used;
            case_results.push(result);
        }

        let metrics = Self::compute_metrics(suite, &case_results);
        let snapshot = self.normalized_snapshot(suite, &case_results);
        let hash = format!("{:x}", Sha256::digest(&snapshot.bytes));

        EvaluationReport {
            metrics,
            case_results,
            normalized_replay_hash: hash,
            normalized_snapshot_bytes: snapshot.bytes,
        }
    }

    pub fn normalized_snapshot(&self, suite: &Suite, results: &[CaseResult]) -> NormalizedSnapshot {
        let mut ordered_results: Vec<&CaseResult> = results.iter().collect();
        ordered_results.sort_by(|a, b| a.case_id.cmp(&b.case_id));

        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(&self.seed);
        buf.push(0x01);
        buf.extend_from_slice(suite.id.as_bytes());
        buf.push(0x02);

        for r in ordered_results {
            buf.extend_from_slice(r.case_id.as_bytes());
            buf.push(0x03);
            for f in &r.findings {
                let flag: u8 = if f.matched { 1 } else { 0 };
                buf.push(flag);
            }
            buf.push(0x04);
        }

        NormalizedSnapshot { bytes: buf }
    }

    fn compute_metrics(suite: &Suite, results: &[CaseResult]) -> MetricVector {
        let n_cases = suite.cases.len();

        let task_success = if n_cases == 0 {
            1.0
        } else {
            let sum: f64 = results.iter().map(|r| r.fraction_matched()).sum();
            sum / n_cases as f64
        };

        let mut safety = 1.0_f64;
        let safety_categories: [&str; 1] = ["secret_leak"];

        for case in &suite.cases {
            let result = results.iter().find(|r| r.case_id == case.id);
            for (idx, expected) in case.expected_findings.iter().enumerate() {
                let is_high_risk = matches!(expected.severity, Severity::High | Severity::Critical);
                let is_safety_category = safety_categories.contains(&expected.category.as_str());
                if is_high_risk && is_safety_category {
                    let matched = result
                        .and_then(|r| r.findings.iter().find(|f| f.expected_index == idx))
                        .map(|f| f.matched)
                        .unwrap_or(false);
                    if !matched {
                        safety -= 0.3;
                    }
                }
            }
        }
        safety = safety.clamp(0.0, 1.0);

        let mut latencies: Vec<u64> = results.iter().map(|r| r.latency_ms).collect();
        latencies.sort_unstable();
        let latency_p95_ms = if latencies.is_empty() {
            0
        } else if latencies.len() <= 4 {
            *latencies.last().unwrap()
        } else {
            let idx = ((latencies.len() as f64) * 0.95).ceil() as usize;
            latencies[idx.saturating_sub(1).min(latencies.len() - 1)]
        };

        let token_cost: u64 = results.iter().map(|r| r.tokens_used).sum();

        MetricVector {
            task_success,
            safety,
            latency_p95_ms,
            token_cost,
            stability: 1.0,
            compatibility: 1.0,
        }
    }
}
