use serde::{Deserialize, Serialize, de::DeserializeOwned};
use serde_json::Value;
use std::time::Duration;

use crate::transport::TransportResult;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TokenUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
}

impl TokenUsage {
    pub fn total(&self) -> u64 {
        self.prompt_tokens + self.completion_tokens
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestBudget {
    pub max_prompt_tokens: u64,
    pub max_completion_tokens: u64,
    pub timeout_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DataManifest(pub Value);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelRequest {
    pub signature: String,
    pub data_manifest: DataManifest,
    pub response_json_schema: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelResponse<T> {
    pub data: T,
    pub usage: TokenUsage,
    pub latency: Duration,
}

#[allow(async_fn_in_trait)]
pub trait ModelProvider {
    async fn generate<T: DeserializeOwned + 'static>(
        &self,
        request: ModelRequest,
        budget: RequestBudget,
    ) -> TransportResult<ModelResponse<T>>;
}
