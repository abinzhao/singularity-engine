use std::collections::BTreeMap;

use sge_eval::{
    ActualFinding, Case, ComparisonOutcome, ContractGates, DeterministicGrader, EvaluationReport,
    MetricVector, Objective, RunMeta, Severity, Suite, SuiteRunner,
};

fn load_fixture_suite() -> Suite {
    let yaml = include_str!("../../../fixtures/evolution/basic-skill/evals/code-review.yaml");
    Suite::from_yaml(yaml).expect("fixture yaml must parse")
}

fn make_actuals(
    has_sql_injection: bool,
    has_secret_leak: bool,
    has_style: bool,
) -> Vec<ActualFinding> {
    let mut v = Vec::new();
    if has_sql_injection {
        v.push(ActualFinding {
            category: "sql_injection".to_string(),
            severity: Severity::High,
            message: Some("avoid string concatenation in SQL queries".to_string()),
        });
    }
    if has_secret_leak {
        v.push(ActualFinding {
            category: "secret_leak".to_string(),
            severity: Severity::High,
            message: Some("hardcoded API key detected".to_string()),
        });
    }
    if has_style {
        v.push(ActualFinding {
            category: "style".to_string(),
            severity: Severity::Low,
            message: Some("spacing around operators".to_string()),
        });
    }
    v
}

fn run_for_actuals_fn<F>(
    suite: &Suite,
    case_order: &[&str],
    actuals_fn: F,
    timestamp_secs: u64,
    workspace_path: &str,
) -> EvaluationReport
where
    F: Fn(&str) -> Vec<ActualFinding>,
{
    let ordered_cases: Vec<&Case> = case_order
        .iter()
        .map(|id| {
            suite
                .cases
                .iter()
                .find(|c| c.id == *id)
                .expect("case must exist")
        })
        .collect();

    let grader = DeterministicGrader;
    let runner = SuiteRunner::new(grader);

    let run_meta = RunMeta {
        timestamp_secs,
        workspace_path: workspace_path.to_string(),
        env_vars: BTreeMap::new(),
    };

    runner.evaluate(
        suite,
        &ordered_cases,
        |case| {
            let actuals = actuals_fn(&case.id);
            let latency_ms: u64 = match case.id.as_str() {
                "sql-injection-1" => 10,
                "secrets-1" => 20,
                "style-only" => 5,
                _ => 1,
            };
            let tokens_used: u64 = match case.id.as_str() {
                "sql-injection-1" => 100,
                "secrets-1" => 200,
                "style-only" => 50,
                _ => 0,
            };
            (actuals, latency_ms, tokens_used)
        },
        &run_meta,
    )
}

#[test]
fn grader_detects_missing_sql_injection_finding_lowers_task_success() {
    let suite = load_fixture_suite();
    let order = ["sql-injection-1", "secrets-1", "style-only"];

    let report_a = run_for_actuals_fn(
        &suite,
        &order,
        |case_id| match case_id {
            "sql-injection-1" => make_actuals(true, false, false),
            "secrets-1" => make_actuals(false, true, false),
            "style-only" => make_actuals(false, false, true),
            _ => vec![],
        },
        1700000000,
        "/tmp/a",
    );

    let report_b = run_for_actuals_fn(
        &suite,
        &order,
        |case_id| match case_id {
            "sql-injection-1" => make_actuals(false, false, false),
            "secrets-1" => make_actuals(false, true, false),
            "style-only" => make_actuals(false, false, true),
            _ => vec![],
        },
        1700000000,
        "/tmp/a",
    );

    assert!(
        (report_a.metrics.task_success - 1.0).abs() < 1e-9,
        "A task_success should be 1.0, got {}",
        report_a.metrics.task_success
    );

    assert!(
        report_b.metrics.task_success < report_a.metrics.task_success,
        "B task_success ({}) must be < A task_success ({})",
        report_b.metrics.task_success,
        report_a.metrics.task_success
    );

    assert!(
        (report_a.metrics.safety - 1.0).abs() < 1e-9,
        "A safety should be 1.0"
    );
    assert!(
        (report_b.metrics.safety - 1.0).abs() < 1e-9,
        "B safety should be 1.0 (secrets not leaked)"
    );

    assert!(
        (report_a.metrics.compatibility - 1.0).abs() < 1e-9,
        "A compatibility default should be 1.0"
    );

    assert_eq!(
        report_a.metrics.latency_p95_ms, report_b.metrics.latency_p95_ms,
        "latency_p95_ms must be independent of findings"
    );
    assert_eq!(
        report_a.metrics.token_cost, report_b.metrics.token_cost,
        "token_cost must be independent of findings"
    );

    assert_ne!(
        report_a.normalized_replay_hash, report_b.normalized_replay_hash,
        "normalized_replay_hash must differ when findings match flags differ (sql-injection-1 finding: A matched, B unmatched)"
    );
}

#[test]
fn metric_comparison_respects_hard_gates_not_universal_score() {
    let objective = Objective {
        primary: "task_success".to_string(),
        protected_metrics: vec!["safety".to_string(), "stability".to_string()],
        hard_gates: {
            let mut m = BTreeMap::new();
            m.insert(
                "safety".to_string(),
                ContractGates {
                    min: Some(0.9),
                    max: None,
                },
            );
            m
        },
    };

    let x = MetricVector {
        task_success: 0.99,
        safety: 0.85,
        latency_p95_ms: 0,
        token_cost: 0,
        stability: 1.0,
        compatibility: 1.0,
    };
    let y = MetricVector {
        task_success: 0.80,
        safety: 0.95,
        latency_p95_ms: 0,
        token_cost: 0,
        stability: 1.0,
        compatibility: 1.0,
    };
    let z = MetricVector {
        task_success: 0.95,
        safety: 0.95,
        latency_p95_ms: 0,
        token_cost: 0,
        stability: 1.0,
        compatibility: 1.0,
    };

    assert_eq!(
        objective.compare(&x, &y),
        ComparisonOutcome::AHardGateViolated,
        "X has safety 0.85 < 0.9, so X should lose"
    );
    assert_eq!(
        objective.compare(&y, &z),
        ComparisonOutcome::BBetter,
        "Z has higher task_success (0.95>0.80) and both pass gates, so Z wins"
    );
}

#[test]
fn normalized_replay_ignores_case_order_timestamps_and_paths() {
    let suite = load_fixture_suite();

    let run1 = run_for_actuals_fn(
        &suite,
        &["sql-injection-1", "secrets-1", "style-only"],
        |case_id| match case_id {
            "sql-injection-1" => make_actuals(true, false, false),
            "secrets-1" => make_actuals(false, true, false),
            "style-only" => make_actuals(false, false, true),
            _ => vec![],
        },
        1700000000,
        "/tmp/a",
    );

    let run2 = run_for_actuals_fn(
        &suite,
        &["style-only", "sql-injection-1", "secrets-1"],
        |case_id| match case_id {
            "sql-injection-1" => make_actuals(true, false, false),
            "secrets-1" => make_actuals(false, true, false),
            "style-only" => make_actuals(false, false, true),
            _ => vec![],
        },
        1900000000,
        "/Users/foo/tmp",
    );

    assert_eq!(
        run1.normalized_replay_hash, run2.normalized_replay_hash,
        "normalized_replay_hash must be identical regardless of case order, timestamps, or paths"
    );

    use sha2::{Digest, Sha256};
    let h1 = format!("{:x}", Sha256::digest(&run1.normalized_snapshot_bytes));
    let h2 = format!("{:x}", Sha256::digest(&run2.normalized_snapshot_bytes));
    assert_eq!(
        h1, h2,
        "byte-level sha256 of normalized snapshots must match"
    );
    assert_eq!(
        h1, run1.normalized_replay_hash,
        "hash method must use sha256 hex of normalized_snapshot_bytes"
    );
}
