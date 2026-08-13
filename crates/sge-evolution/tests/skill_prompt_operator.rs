use std::{collections::BTreeMap, fs, path::PathBuf};

use serde_json::{Value, json};
use sge_domain::TargetRef;
use sge_evolution::{
    operator::{
        MutationContext, MutationError, MutationOperator, MutationPatch, PromptCandidatesResponse,
    },
    operators::skill_prompt::SkillPromptMutation,
};
use sge_provider::{RecordedModelProvider, RequestBudget};
use sha2::{Digest, Sha256};

const SQL_CLAUSE: &str = "Never approve unescaped SQL string concatenation.";
const SECRETS_CLAUSE: &str = "Flag secrets in source code.";
const SOURCE: &str = "# Code Review\n\nReview changes carefully.\n";
const FIXTURE_SOURCE_HASH: &str =
    "5ad7c8d3a19bde2d5a4c136334a1c2ab0540154a9412f45f625d060bc13b7ae5";

fn target() -> TargetRef {
    "skill:code-review".parse().unwrap()
}

fn non_skill_target() -> TargetRef {
    "agent:code-review".parse().unwrap()
}

fn hash(content: &str) -> String {
    format!("{:x}", Sha256::digest(content.as_bytes()))
}

fn replacement() -> String {
    format!("# Code Review\n\n{SQL_CLAUSE}\n{SECRETS_CLAUSE}\n")
}

fn patch(path: &str) -> MutationPatch {
    MutationPatch {
        path: path.to_string(),
        source_hash_algorithm: "sha256".to_string(),
        source_hash: hash(SOURCE),
        complete_replacement: replacement(),
    }
}

fn declared_files() -> BTreeMap<String, String> {
    BTreeMap::from([("instructions.md".to_string(), SOURCE.to_string())])
}

fn budget() -> RequestBudget {
    RequestBudget {
        max_prompt_tokens: 1_000,
        max_completion_tokens: 1_000,
        timeout_ms: 1_000,
    }
}

fn context<'a>(
    target: &'a TargetRef,
    files: &'a BTreeMap<String, String>,
    clauses: &'a [String],
) -> MutationContext<'a> {
    MutationContext {
        target,
        declared_files: files,
        required_safety_clauses: clauses,
        intent: "Strengthen security review guidance",
        evidence_refs: &[],
        budget: budget(),
    }
}

fn fixture_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/provider/prompt-candidates.json")
}

fn fixture_source_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../fixtures/evolution/basic-skill/instructions.md")
}

fn provider_with_fixture(source_hash: &str) -> (tempfile::TempDir, RecordedModelProvider) {
    let dir = tempfile::tempdir().unwrap();
    let content = fs::read_to_string(fixture_path())
        .unwrap()
        .replace(FIXTURE_SOURCE_HASH, source_hash);
    let path = dir.path().join("prompt-candidates.json");
    fs::write(&path, content).unwrap();
    let provider = RecordedModelProvider::load(path).unwrap();
    (dir, provider)
}

fn provider_with_candidates(
    candidates: Vec<MutationPatch>,
) -> (tempfile::TempDir, RecordedModelProvider) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("response.json");
    fs::write(
        &path,
        serde_json::to_vec_pretty(&json!({
            "version": 1,
            "entries": [{
                "request_signature": "mutate:skill_prompt:code-review:v1",
                "delay_ms": 0,
                "usage": {"prompt_tokens": 10, "completion_tokens": 10},
                "response": PromptCandidatesResponse { candidates }
            }]
        }))
        .unwrap(),
    )
    .unwrap();
    let provider = RecordedModelProvider::load(path).unwrap();
    (dir, provider)
}

#[test]
fn recorded_fixture_source_hash_matches_the_committed_skill() {
    let source = fs::read_to_string(fixture_source_path()).unwrap();
    assert_eq!(hash(&source), FIXTURE_SOURCE_HASH);

    let fixture: Value =
        serde_json::from_str(&fs::read_to_string(fixture_path()).unwrap()).unwrap();
    let candidates = fixture["entries"][0]["response"]["candidates"]
        .as_array()
        .unwrap();
    assert!(
        candidates
            .iter()
            .all(|candidate| candidate["source_hash"] == FIXTURE_SOURCE_HASH)
    );
}

#[tokio::test]
async fn recorded_provider_proposes_valid_patches_and_apply_is_pure() {
    let files = declared_files();
    let clauses = vec![SQL_CLAUSE.to_string(), SECRETS_CLAUSE.to_string()];
    let target = target();
    let context = context(&target, &files, &clauses);
    let (_dir, provider) = provider_with_fixture(&hash(SOURCE));
    let operator = SkillPromptMutation::new(provider);

    let allowed = operator.allowed_paths(&target);
    assert_eq!(allowed.len(), 1);
    assert_eq!(allowed[0].artifact_relative, "instructions.md");
    assert_eq!(
        allowed[0].workspace_relative,
        "skills/code-review/instructions.md"
    );

    let proposed = operator.propose(context.clone()).await.unwrap();
    assert_eq!(proposed.len(), 3);
    assert!(proposed.iter().all(|item| item.path == "instructions.md"));

    for candidate in &proposed {
        operator.validate(candidate, &context).unwrap();
        let applied = operator.apply(candidate, &context).unwrap();
        assert_eq!(
            applied.workspace_relative_path,
            "skills/code-review/instructions.md"
        );
        assert_eq!(applied.complete_replacement, candidate.complete_replacement);
        assert_eq!(applied.source_hash, hash(SOURCE));
        assert_eq!(
            applied.replacement_hash,
            hash(&candidate.complete_replacement)
        );
    }
    assert_eq!(files["instructions.md"], SOURCE);
}

#[test]
fn rejects_paths_outside_the_single_allowed_artifact_path() {
    let files = declared_files();
    let clauses = vec![SQL_CLAUSE.to_string(), SECRETS_CLAUSE.to_string()];
    let target = target();
    let context = context(&target, &files, &clauses);
    let (_dir, provider) = provider_with_fixture(&hash(SOURCE));
    let operator = SkillPromptMutation::new(provider);

    for path in [
        "/tmp/instructions.md",
        "../instructions.md",
        "skill.yaml",
        "scripts/tool.sh",
        "skills/other/instructions.md",
    ] {
        assert_eq!(
            operator.validate(&patch(path), &context),
            Err(MutationError::PathNotAllowed)
        );
    }
}

#[test]
fn rejects_undeclared_instructions_file() {
    let files = BTreeMap::new();
    let clauses = vec![SQL_CLAUSE.to_string(), SECRETS_CLAUSE.to_string()];
    let target = target();
    let context = context(&target, &files, &clauses);
    let (_dir, provider) = provider_with_fixture(&hash(SOURCE));
    let operator = SkillPromptMutation::new(provider);

    assert_eq!(
        operator.validate(&patch("instructions.md"), &context),
        Err(MutationError::UndeclaredPath)
    );
}

#[test]
fn rejects_unsupported_invalid_and_stale_hashes() {
    let files = declared_files();
    let clauses = vec![SQL_CLAUSE.to_string(), SECRETS_CLAUSE.to_string()];
    let target = target();
    let context = context(&target, &files, &clauses);
    let (_dir, provider) = provider_with_fixture(&hash(SOURCE));
    let operator = SkillPromptMutation::new(provider);

    let mut unsupported = patch("instructions.md");
    unsupported.source_hash_algorithm = "sha512".to_string();
    assert_eq!(
        operator.validate(&unsupported, &context),
        Err(MutationError::UnsupportedHashAlgorithm)
    );

    let mut invalid = patch("instructions.md");
    invalid.source_hash = "not-a-sha256".to_string();
    assert_eq!(
        operator.validate(&invalid, &context),
        Err(MutationError::InvalidSourceHash)
    );

    let mut stale = patch("instructions.md");
    stale.source_hash = "a".repeat(64);
    assert_eq!(
        operator.validate(&stale, &context),
        Err(MutationError::StaleSource)
    );
}

#[test]
fn rejects_missing_safety_clauses_empty_and_oversized_replacements() {
    let files = declared_files();
    let clauses = vec![SQL_CLAUSE.to_string(), SECRETS_CLAUSE.to_string()];
    let target = target();
    let context = context(&target, &files, &clauses);
    let (_dir, provider) = provider_with_fixture(&hash(SOURCE));
    let operator = SkillPromptMutation::new(provider);

    for missing in [SQL_CLAUSE, SECRETS_CLAUSE] {
        let mut unsafe_patch = patch("instructions.md");
        unsafe_patch.complete_replacement = replacement().replace(missing, "");
        assert_eq!(
            operator.validate(&unsafe_patch, &context),
            Err(MutationError::MissingSafetyClause(missing.to_string()))
        );
    }

    let mut empty = patch("instructions.md");
    empty.complete_replacement = " \n\t".to_string();
    assert_eq!(
        operator.validate(&empty, &context),
        Err(MutationError::EmptyReplacement)
    );

    let mut oversized = patch("instructions.md");
    oversized.complete_replacement =
        format!("{SQL_CLAUSE}\n{SECRETS_CLAUSE}\n{}", "x".repeat(256 * 1024));
    assert_eq!(
        operator.validate(&oversized, &context),
        Err(MutationError::ReplacementTooLarge)
    );
}

#[tokio::test]
async fn propose_rejects_the_entire_response_when_one_candidate_is_dangerous() {
    let files = declared_files();
    let clauses = vec![SQL_CLAUSE.to_string(), SECRETS_CLAUSE.to_string()];
    let target = target();
    let context = context(&target, &files, &clauses);
    let mut dangerous = patch("instructions.md");
    dangerous.path = "../instructions.md".to_string();
    let (_dir, provider) = provider_with_candidates(vec![patch("instructions.md"), dangerous]);
    let operator = SkillPromptMutation::new(provider);

    assert_eq!(
        operator.propose(context).await,
        Err(MutationError::PathNotAllowed)
    );
}

#[tokio::test]
async fn non_skill_targets_have_no_paths_and_are_rejected() {
    let files = declared_files();
    let clauses = vec![SQL_CLAUSE.to_string(), SECRETS_CLAUSE.to_string()];
    let target = non_skill_target();
    let context = context(&target, &files, &clauses);
    let (_dir, provider) = provider_with_fixture(&hash(SOURCE));
    let operator = SkillPromptMutation::new(provider);

    assert!(operator.allowed_paths(&target).is_empty());
    assert_eq!(
        operator.validate(&patch("instructions.md"), &context),
        Err(MutationError::UnsupportedTarget)
    );
    assert_eq!(
        operator.propose(context).await,
        Err(MutationError::UnsupportedTarget)
    );
}

#[test]
fn prompt_response_schema_is_explicit_and_structured() {
    let schema = serde_json::to_value(schemars::schema_for!(PromptCandidatesResponse)).unwrap();
    let root: &Value = &schema;
    assert_eq!(root["type"], "object");
    assert!(root["properties"]["candidates"].is_object());
    assert_eq!(root["properties"]["candidates"]["type"], "array");
}
