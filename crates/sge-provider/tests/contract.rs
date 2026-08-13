use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;

use sge_provider::{
    DataManifest, ModelProvider, ModelRequest, ProviderError, RecordedModelProvider, RequestBudget,
};

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ScanProposal {
    pub id: String,
    pub title: String,
    pub risk: String,
    pub affected_files: Vec<String>,
    pub confidence: f64,
    pub evidence_refs: Vec<String>,
    pub estimated_improvement: BTreeMap<String, [f64; 2]>,
    pub evaluation_method: String,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct ScanProposalsResponse {
    pub proposals: Vec<ScanProposal>,
}

fn scan_proposals_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(ScanProposalsResponse)).unwrap()
}

fn fixture_path(name: &str) -> String {
    format!(
        "{}/fixtures/provider/{name}",
        env!("CARGO_MANIFEST_DIR").replace("/crates/sge-provider", "")
    )
}

#[tokio::test]
async fn valid_structured_response_passes_schema_validation_and_respects_budget() {
    let provider = RecordedModelProvider::load(fixture_path("scan-proposals.json")).unwrap();

    let req = ModelRequest {
        signature: "scan:code-review:v1".to_string(),
        data_manifest: DataManifest(json!({"target":"skill:code-review"})),
        response_json_schema: Some(scan_proposals_schema()),
    };

    let budget = RequestBudget {
        max_prompt_tokens: 1000,
        max_completion_tokens: 1000,
        timeout_ms: 1000,
    };

    let result = provider
        .generate::<ScanProposalsResponse>(req, budget)
        .await;
    assert!(result.is_ok(), "expected Ok, got {result:?}");

    let resp = result.unwrap();
    assert!(
        !resp.data.proposals.is_empty(),
        "expected at least 1 proposal"
    );
    assert_eq!(resp.data.proposals[0].id, "prop-1");
    assert_eq!(resp.usage.prompt_tokens, 200);
    assert_eq!(resp.usage.completion_tokens, 410);
    assert!(resp.latency.as_millis() as u64 <= 300);
}

#[tokio::test]
async fn over_budget_response_returns_budget_exceeded() {
    let provider = RecordedModelProvider::load(fixture_path("scan-proposals.json")).unwrap();

    let req = ModelRequest {
        signature: "scan:code-review:v1".to_string(),
        data_manifest: DataManifest(json!({"target":"skill:code-review"})),
        response_json_schema: Some(scan_proposals_schema()),
    };

    let budget = RequestBudget {
        max_prompt_tokens: 1000,
        max_completion_tokens: 100,
        timeout_ms: 1000,
    };

    let result = provider
        .generate::<ScanProposalsResponse>(req, budget)
        .await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    match &err {
        ProviderError::BudgetExceeded { kind, max, used } => {
            assert_eq!(*kind, "completion_tokens");
            assert_eq!(*max, 100);
            assert_eq!(*used, 410);
        }
        other => panic!("expected BudgetExceeded, got {other:?}"),
    }
}

#[tokio::test]
async fn malformed_json_returns_protocol_error() {
    let dir = tempfile::tempdir().unwrap();
    let fixture_path = dir.path().join("malformed.json");

    let content = json!({
        "version": 1,
        "entries": [{
            "request_signature": "scan:code-review:v1",
            "delay_ms": 15,
            "usage": {"prompt_tokens": 200, "completion_tokens": 410},
            "response": {
                "proposals": "not-an-array"
            }
        }]
    });

    std::fs::write(
        &fixture_path,
        serde_json::to_string_pretty(&content).unwrap(),
    )
    .unwrap();

    let provider = RecordedModelProvider::load(&fixture_path).unwrap();

    let req = ModelRequest {
        signature: "scan:code-review:v1".to_string(),
        data_manifest: DataManifest(json!({"target":"skill:code-review"})),
        response_json_schema: Some(scan_proposals_schema()),
    };

    let budget = RequestBudget {
        max_prompt_tokens: 1000,
        max_completion_tokens: 1000,
        timeout_ms: 1000,
    };

    let result = provider
        .generate::<ScanProposalsResponse>(req, budget)
        .await;
    assert!(result.is_err(), "expected Err, got {result:?}");

    let err = result.unwrap_err();
    match &err {
        ProviderError::SchemaValidationFailed { .. } => {}
        ProviderError::DeserializationFailed { .. } => {}
        other => panic!("expected SchemaValidationFailed or DeserializationFailed, got {other:?}"),
    }
}

#[tokio::test]
async fn timeout_returns_timeout_error() {
    let dir = tempfile::tempdir().unwrap();
    let fixture_path = dir.path().join("slow.json");

    let content = json!({
        "version": 1,
        "entries": [{
            "request_signature": "scan:code-review:v1",
            "delay_ms": 200,
            "usage": {"prompt_tokens": 200, "completion_tokens": 410},
            "response": {
                "proposals": [{
                    "id": "prop-1",
                    "title": "Add SQL injection guard clauses",
                    "risk": "high",
                    "affected_files": ["skills/code-review/instructions.md"],
                    "confidence": 0.92,
                    "evidence_refs": ["eval::sql-injection-1"],
                    "estimated_improvement": {"task_success": [0.1, 0.3]},
                    "evaluation_method": "rerun code-review suite"
                }]
            }
        }]
    });

    std::fs::write(
        &fixture_path,
        serde_json::to_string_pretty(&content).unwrap(),
    )
    .unwrap();

    let provider = RecordedModelProvider::load(&fixture_path).unwrap();

    let req = ModelRequest {
        signature: "scan:code-review:v1".to_string(),
        data_manifest: DataManifest(json!({"target":"skill:code-review"})),
        response_json_schema: Some(scan_proposals_schema()),
    };

    let budget = RequestBudget {
        max_prompt_tokens: 1000,
        max_completion_tokens: 1000,
        timeout_ms: 10,
    };

    let result = provider
        .generate::<ScanProposalsResponse>(req, budget)
        .await;
    assert!(result.is_err(), "expected Err, got {result:?}");

    let err = result.unwrap_err();
    match &err {
        ProviderError::Timeout {
            budget_ms,
            observed_ms,
        } => {
            assert_eq!(*budget_ms, 10);
            assert!(*observed_ms > 10);
        }
        other => panic!("expected Timeout, got {other:?}"),
    }
}
