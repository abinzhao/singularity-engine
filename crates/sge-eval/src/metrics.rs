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
                let av = self.get_primary(a);
                let bv = self.get_primary(b);
                if av > bv {
                    ComparisonOutcome::ABetter
                } else if bv > av {
                    ComparisonOutcome::BBetter
                } else {
                    ComparisonOutcome::Tie
                }
            }
        }
    }

    fn respects_gates(&self, m: &MetricVector) -> bool {
        for (metric, gate) in &self.hard_gates {
            let value = self.get_metric(m, metric);
            if let Some(min) = gate.min
                && value < min
            {
                return false;
            }
            if let Some(max) = gate.max
                && value > max
            {
                return false;
            }
        }
        true
    }

    fn get_primary(&self, m: &MetricVector) -> f64 {
        self.get_metric(m, &self.primary)
    }

    fn get_metric(&self, m: &MetricVector, name: &str) -> f64 {
        match name {
            "task_success" => m.task_success,
            "safety" => m.safety,
            "stability" => m.stability,
            "compatibility" => m.compatibility,
            _ => 0.0,
        }
    }
}
