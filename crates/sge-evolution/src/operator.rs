use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sge_domain::TargetRef;
use sge_provider::RequestBudget;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OperatorDescriptor {
    pub id: String,
    pub risk: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PathPolicy {
    pub artifact_relative: String,
    pub workspace_relative: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct MutationPatch {
    pub path: String,
    pub source_hash_algorithm: String,
    pub source_hash: String,
    pub complete_replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PromptCandidatesResponse {
    pub candidates: Vec<MutationPatch>,
}

#[derive(Debug, Clone)]
pub struct MutationContext<'a> {
    pub target: &'a TargetRef,
    pub declared_files: &'a BTreeMap<String, String>,
    pub required_safety_clauses: &'a [String],
    pub intent: &'a str,
    pub evidence_refs: &'a [String],
    pub budget: RequestBudget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMutation {
    pub workspace_relative_path: String,
    pub complete_replacement: String,
    pub source_hash: String,
    pub replacement_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MutationError {
    #[error("target is not supported by this operator")]
    UnsupportedTarget,
    #[error("mutation path is not allowed")]
    PathNotAllowed,
    #[error("mutation path is not declared")]
    UndeclaredPath,
    #[error("source hash algorithm is not supported")]
    UnsupportedHashAlgorithm,
    #[error("source hash is not a valid SHA-256 digest")]
    InvalidSourceHash,
    #[error("source content has changed")]
    StaleSource,
    #[error("replacement content is empty")]
    EmptyReplacement,
    #[error("replacement content exceeds 256 KiB")]
    ReplacementTooLarge,
    #[error("replacement is missing required safety clause: {0}")]
    MissingSafetyClause(String),
    #[error("provider failed: {0}")]
    Provider(String),
    #[error("provider returned no candidates")]
    NoCandidates,
}

#[allow(async_fn_in_trait)]
pub trait MutationOperator {
    fn descriptor(&self) -> OperatorDescriptor;
    fn allowed_paths(&self, target: &TargetRef) -> Vec<PathPolicy>;
    async fn propose(
        &self,
        context: MutationContext<'_>,
    ) -> Result<Vec<MutationPatch>, MutationError>;
    fn validate(
        &self,
        patch: &MutationPatch,
        context: &MutationContext<'_>,
    ) -> Result<(), MutationError>;
    fn apply(
        &self,
        patch: &MutationPatch,
        context: &MutationContext<'_>,
    ) -> Result<AppliedMutation, MutationError>;
}
