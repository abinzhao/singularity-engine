use std::{path::Path, time::Instant};

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{ModelProvider, ModelRequest, ModelResponse, RequestBudget, TokenUsage};
use crate::transport::{ProviderError, TransportResult};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RecordedEntry {
    request_signature: String,
    delay_ms: u64,
    usage: TokenUsage,
    response: Value,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RecordedFile {
    version: u32,
    entries: Vec<RecordedEntry>,
}

pub struct RecordedModelProvider {
    entries: std::collections::BTreeMap<String, RecordedEntry>,
    time_fn: fn() -> Instant,
    sleep_fn: fn(u64) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>>,
}

impl RecordedModelProvider {
    pub fn load(path: impl AsRef<Path>) -> TransportResult<Self> {
        let content = std::fs::read_to_string(path.as_ref()).map_err(|e| ProviderError::Io {
            message: e.to_string(),
        })?;
        let file: RecordedFile =
            serde_json::from_str(&content).map_err(|e| ProviderError::DeserializationFailed {
                message: e.to_string(),
            })?;
        let mut entries = std::collections::BTreeMap::new();
        for e in file.entries {
            entries.insert(e.request_signature.clone(), e);
        }
        Ok(Self {
            entries,
            time_fn: Instant::now,
            sleep_fn: |ms| {
                Box::pin(async move {
                    let start = Instant::now();
                    let dur = std::time::Duration::from_millis(ms);
                    while start.elapsed() < dur { /* spin */ }
                })
            },
        })
    }
}

impl ModelProvider for RecordedModelProvider {
    async fn generate<T: serde::de::DeserializeOwned + 'static>(
        &self,
        request: ModelRequest,
        budget: RequestBudget,
    ) -> TransportResult<ModelResponse<T>> {
        let start = (self.time_fn)();
        let entry = self
            .entries
            .get(&request.signature)
            .ok_or_else(|| ProviderError::NoRecordedEntry {
                signature: request.signature.clone(),
            })?
            .clone();

        if entry.usage.prompt_tokens > budget.max_prompt_tokens {
            return Err(ProviderError::BudgetExceeded {
                kind: "prompt_tokens",
                max: budget.max_prompt_tokens,
                used: entry.usage.prompt_tokens,
            });
        }
        if entry.usage.completion_tokens > budget.max_completion_tokens {
            return Err(ProviderError::BudgetExceeded {
                kind: "completion_tokens",
                max: budget.max_completion_tokens,
                used: entry.usage.completion_tokens,
            });
        }

        if let Some(schema) = &request.response_json_schema {
            validate_json_schema(&entry.response, schema)?;
        }

        (self.sleep_fn)(entry.delay_ms).await;
        let elapsed = start.elapsed();

        if elapsed.as_millis() as u64 > budget.timeout_ms {
            return Err(ProviderError::Timeout {
                budget_ms: budget.timeout_ms,
                observed_ms: elapsed.as_millis() as u64,
            });
        }

        let data: T = serde_json::from_value(entry.response.clone()).map_err(|e| {
            ProviderError::DeserializationFailed {
                message: e.to_string(),
            }
        })?;

        Ok(ModelResponse {
            data,
            usage: entry.usage,
            latency: elapsed,
        })
    }
}

fn validate_json_schema(instance: &Value, schema: &Value) -> TransportResult<()> {
    fn check(value: &Value, schema: &Value, path: &str) -> Result<(), String> {
        let expected_type = schema.get("type").and_then(|t| t.as_str());
        if let Some(t) = expected_type {
            let ok = match t {
                "object" => value.is_object(),
                "array" => value.is_array(),
                "string" => value.is_string(),
                "number" | "integer" => value.is_number(),
                "boolean" => value.is_boolean(),
                "null" => value.is_null(),
                _ => true,
            };
            if !ok {
                return Err(format!("type mismatch at {path}: expected {t}"));
            }
        }
        if let Some(props) = schema.get("properties").and_then(|p| p.as_object())
            && let Some(obj) = value.as_object()
        {
            for (k, subschema) in props {
                let child_path = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{path}.{k}")
                };
                if let Some(child) = obj.get(k) {
                    check(child, subschema, &child_path)?;
                }
            }
        }
        if let Some(required) = schema.get("required").and_then(|r| r.as_array()) {
            let obj = value
                .as_object()
                .ok_or_else(|| format!("{path}: required needs object"))?;
            for r in required {
                let key = r
                    .as_str()
                    .ok_or_else(|| format!("{path}: required not string"))?;
                if !obj.contains_key(key) {
                    return Err(format!("{path}: missing required field `{key}`"));
                }
            }
        }
        if let (Some(items_schema), Some(arr)) = (schema.get("items"), value.as_array()) {
            for (i, item) in arr.iter().enumerate() {
                check(item, items_schema, &format!("{path}[{i}]"))?;
            }
        }
        Ok(())
    }

    check(instance, schema, "").map_err(|message| ProviderError::SchemaValidationFailed { message })
}
