use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricVector {
    pub task_success: f64,
    pub safety: f64,
    pub latency_p95_ms: u64,
    pub token_cost: u64,
    pub stability: f64,
    pub compatibility: f64,
}

impl Default for MetricVector {
    fn default() -> Self {
        Self {
            task_success: 1.0,
            safety: 1.0,
            stability: 1.0,
            compatibility: 1.0,
            latency_p95_ms: 0,
            token_cost: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContractGates {
    pub min: Option<f64>,
    pub max: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Objective {
    pub primary: String,
    pub protected_metrics: Vec<String>,
    pub hard_gates: std::collections::BTreeMap<String, ContractGates>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ComparisonOutcome {
    ABetter,
    BBetter,
    Tie,
    AHardGateViolated,
    BHardGateViolated,
    BothHardGateViolated,
}

impl Objective {
    pub fn compare(&self, a: &MetricVector, b: &MetricVector) -> ComparisonOutcome {
        let a_ok = self.respects_gates(a);
        let b_ok = self.respects_gates(b);
        match (a_ok, b_ok) {
            (false, false) => ComparisonOutcome::BothHardGateViolated,
            (false, true) => ComparisonOutcome::AHardGateViolated,
            (true, false) => ComparisonOutcome::BHardGateViolated,
            (true, true) => {
                let av = self.metric_value(a, &self.primary);
                let bv = self.metric_value(b, &self.primary);
                if self.is_better(&self.primary, av, bv) {
                    ComparisonOutcome::ABetter
                } else if self.is_better(&self.primary, bv, av) {
                    ComparisonOutcome::BBetter
                } else {
                    ComparisonOutcome::Tie
                }
            }
        }
    }

    pub fn gate_violations(&self, metrics: &MetricVector) -> Vec<String> {
        let mut violations = Vec::new();
        for (metric, gate) in &self.hard_gates {
            let value = self.metric_value(metrics, metric);
            if let Some(min) = gate.min
                && value < min
            {
                violations.push(format!("{metric} below minimum {min}"));
            }
            if let Some(max) = gate.max
                && value > max
            {
                violations.push(format!("{metric} above maximum {max}"));
            }
        }
        violations
    }

    pub fn protected_regressions(
        &self,
        candidate: &MetricVector,
        baseline: &MetricVector,
    ) -> Vec<String> {
        self.protected_metrics
            .iter()
            .filter(|metric| {
                let candidate_value = self.metric_value(candidate, metric);
                let baseline_value = self.metric_value(baseline, metric);
                self.is_better(metric, baseline_value, candidate_value)
            })
            .cloned()
            .collect()
    }

    pub fn primary_value(&self, metrics: &MetricVector) -> f64 {
        self.metric_value(metrics, &self.primary)
    }

    pub fn primary_is_better(&self, candidate: f64, incumbent: f64) -> bool {
        self.is_better(&self.primary, candidate, incumbent)
    }

    fn respects_gates(&self, metrics: &MetricVector) -> bool {
        self.gate_violations(metrics).is_empty()
    }

    fn metric_value(&self, metrics: &MetricVector, name: &str) -> f64 {
        match name {
            "task_success" => metrics.task_success,
            "safety" => metrics.safety,
            "stability" => metrics.stability,
            "compatibility" => metrics.compatibility,
            "latency_p95_ms" => metrics.latency_p95_ms as f64,
            "token_cost" => metrics.token_cost as f64,
            _ => 0.0,
        }
    }

    fn is_better(&self, metric: &str, candidate: f64, incumbent: f64) -> bool {
        match metric {
            "latency_p95_ms" | "token_cost" => candidate < incumbent,
            _ => candidate > incumbent,
        }
    }
}
