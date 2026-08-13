use std::path::{Component, Path};

use schemars::schema_for;
use serde_json::json;
use sge_domain::{ArtifactKind, TargetRef};
use sge_provider::{DataManifest, ModelProvider, ModelRequest};
use sha2::{Digest, Sha256};

use crate::operator::{
    AppliedMutation, MutationContext, MutationError, MutationOperator, MutationPatch,
    OperatorDescriptor, PathPolicy, PromptCandidatesResponse,
};

const INSTRUCTIONS_PATH: &str = "instructions.md";
const MAX_REPLACEMENT_BYTES: usize = 256 * 1024;

pub struct SkillPromptMutation<P> {
    provider: P,
}

impl<P> SkillPromptMutation<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }
}

impl<P: ModelProvider + Sync> MutationOperator for SkillPromptMutation<P> {
    fn descriptor(&self) -> OperatorDescriptor {
        OperatorDescriptor {
            id: "skill_prompt_mutation".to_string(),
            risk: "low".to_string(),
            description: "Replace a Skill instructions.md prompt".to_string(),
        }
    }

    fn allowed_paths(&self, target: &TargetRef) -> Vec<PathPolicy> {
        if target.kind() != ArtifactKind::Skill {
            return Vec::new();
        }

        vec![PathPolicy {
            artifact_relative: INSTRUCTIONS_PATH.to_string(),
            workspace_relative: format!("skills/{}/instructions.md", target.name()),
        }]
    }

    async fn propose(
        &self,
        context: MutationContext<'_>,
    ) -> Result<Vec<MutationPatch>, MutationError> {
        ensure_skill_target(context.target)?;
        let source_content = context
            .declared_files
            .get(INSTRUCTIONS_PATH)
            .ok_or(MutationError::UndeclaredPath)?;
        let source_hash = sha256(source_content);
        let allowed_paths = self.allowed_paths(context.target);
        let schema = serde_json::to_value(schema_for!(PromptCandidatesResponse))
            .map_err(|error| MutationError::Provider(error.to_string()))?;
        let request = ModelRequest {
            signature: format!("mutate:skill_prompt:{}:v1", context.target.name()),
            data_manifest: DataManifest(json!({
                "operator": self.descriptor().id,
                "target": context.target.to_string(),
                "intent": context.intent,
                "evidence_refs": context.evidence_refs,
                "allowed_paths": allowed_paths,
                "source_hash": {
                    "algorithm": "sha256",
                    "value": source_hash,
                },
                "source_content": source_content,
                "required_safety_clauses": context.required_safety_clauses,
            })),
            response_json_schema: Some(schema),
        };

        let response = self
            .provider
            .generate::<PromptCandidatesResponse>(request, context.budget.clone())
            .await
            .map_err(|error| MutationError::Provider(error.to_string()))?;
        if response.data.candidates.is_empty() {
            return Err(MutationError::NoCandidates);
        }
        for patch in &response.data.candidates {
            self.validate(patch, &context)?;
        }
        Ok(response.data.candidates)
    }

    fn validate(
        &self,
        patch: &MutationPatch,
        context: &MutationContext<'_>,
    ) -> Result<(), MutationError> {
        ensure_skill_target(context.target)?;
        ensure_allowed_path(&patch.path)?;
        let current_source = context
            .declared_files
            .get(&patch.path)
            .ok_or(MutationError::UndeclaredPath)?;
        if patch.source_hash_algorithm != "sha256" {
            return Err(MutationError::UnsupportedHashAlgorithm);
        }
        if patch.source_hash.len() != 64
            || !patch
                .source_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit())
        {
            return Err(MutationError::InvalidSourceHash);
        }
        if sha256(current_source) != patch.source_hash.to_ascii_lowercase() {
            return Err(MutationError::StaleSource);
        }
        if patch.complete_replacement.trim().is_empty() {
            return Err(MutationError::EmptyReplacement);
        }
        if patch.complete_replacement.len() > MAX_REPLACEMENT_BYTES {
            return Err(MutationError::ReplacementTooLarge);
        }
        for clause in context.required_safety_clauses {
            if !patch.complete_replacement.contains(clause) {
                return Err(MutationError::MissingSafetyClause(clause.clone()));
            }
        }
        Ok(())
    }

    fn apply(
        &self,
        patch: &MutationPatch,
        context: &MutationContext<'_>,
    ) -> Result<AppliedMutation, MutationError> {
        self.validate(patch, context)?;
        Ok(AppliedMutation {
            workspace_relative_path: format!("skills/{}/instructions.md", context.target.name()),
            complete_replacement: patch.complete_replacement.clone(),
            source_hash: patch.source_hash.to_ascii_lowercase(),
            replacement_hash: sha256(&patch.complete_replacement),
        })
    }
}

fn ensure_skill_target(target: &TargetRef) -> Result<(), MutationError> {
    if target.kind() == ArtifactKind::Skill {
        Ok(())
    } else {
        Err(MutationError::UnsupportedTarget)
    }
}

fn ensure_allowed_path(path: &str) -> Result<(), MutationError> {
    let parsed = Path::new(path);
    if parsed.is_absolute()
        || parsed.components().count() != 1
        || !parsed
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
        || path != INSTRUCTIONS_PATH
    {
        return Err(MutationError::PathNotAllowed);
    }
    Ok(())
}

fn sha256(content: &str) -> String {
    format!("{:x}", Sha256::digest(content.as_bytes()))
}
