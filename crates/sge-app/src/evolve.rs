use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use serde_json::json;
use sge_domain::TargetRef;
use sge_eval::{
    ActualFinding, Case, DeterministicGrader, EvaluationReport, RunMeta, Suite, SuiteRunner,
};
use sge_evolution::{
    operator::{MutationContext, MutationOperator},
    operators::skill_prompt::SkillPromptMutation,
};
use sge_provider::{RecordedModelProvider, RequestBudget};
use sge_store::{
    AppendOnlyJournal, GitLineageRepository, JournalState, LineageRepository, Revision,
};
use sha2::{Digest, Sha256};

use crate::{
    AppError, Result,
    replay::{REPLAY_V1, ReplayCandidate, ReplayDocument},
    scan::{ScanOptions, scan_workspace},
};

#[derive(Debug, Clone)]
pub struct EvolveOptions {
    pub approve: String,
    pub provider_fixture: PathBuf,
    pub candidate_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct CandidateOutcome {
    pub id: String,
    pub worktree_path: PathBuf,
    pub evidence_path: PathBuf,
    pub revision: Option<String>,
    pub evaluation: Option<EvaluationReport>,
    pub rejection_reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct EvolveOutcome {
    pub run_id: String,
    pub target: String,
    pub baseline_path: PathBuf,
    pub contract_path: PathBuf,
    pub journal_path: PathBuf,
    pub candidates: Vec<CandidateOutcome>,
    pub selected_candidate: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct TestOutcome {
    pub target: String,
    pub source_path: PathBuf,
    pub evaluation: EvaluationReport,
}

pub async fn evolve_workspace(
    workspace: impl AsRef<Path>,
    target: &str,
    options: EvolveOptions,
) -> Result<EvolveOutcome> {
    let workspace = workspace.as_ref();
    if options.candidate_count == 0 || options.candidate_count > 5 {
        return Err(evolution_error(
            workspace,
            "candidate count must be between 1 and 5",
        ));
    }

    let target_ref = TargetRef::from_str(target)
        .map_err(|error| evolution_error(workspace, error.to_string()))?;
    let standard_dir = workspace.join("skills").join(target_ref.name());
    let standard_source = standard_dir.join("instructions.md");
    let source_before = fs::read(&standard_source).map_err(|error| AppError::Io {
        path: standard_source.clone(),
        source: error,
    })?;
    let source_content = String::from_utf8(source_before.clone())
        .map_err(|error| evolution_error(&standard_source, error.to_string()))?;
    let suite_path = standard_dir.join("evals/code-review.yaml");
    let suite = read_suite(&suite_path)?;

    let (run_id, run_dir) = create_run_dir(workspace)?;
    let journal_path = run_dir.join("journal.ndjson");
    let journal = AppendOnlyJournal::open(&journal_path)
        .map_err(|error| evolution_error(&journal_path, error.to_string()))?;
    journal_state(&journal, JournalState::Prepared, json!({"target": target}))?;

    let repo_path = workspace.join(".singularity/repo.git");
    let repo = GitLineageRepository::init_or_open_bare(&repo_path)
        .map_err(|error| evolution_error(&repo_path, error.to_string()))?;
    let baseline_revision = repo
        .snapshot(
            &standard_dir,
            json!({"op": "evolve-baseline", "run_id": run_id, "target": target}),
        )
        .map_err(|error| evolution_error(&standard_dir, error.to_string()))?;
    let baseline = evaluate_instructions(&suite, &source_content, &standard_dir)?;
    let baseline_path = run_dir.join("baseline.json");
    write_json(&baseline_path, &baseline)?;
    journal_state(
        &journal,
        JournalState::Baseline,
        json!({"revision": baseline_revision.as_str(), "evidence": baseline_path}),
    )?;

    let scan = scan_workspace(
        workspace,
        target,
        ScanOptions {
            approve: Some(options.approve.clone()),
            goal: None,
        },
    )?;
    journal_state(
        &journal,
        JournalState::Diagnosed,
        json!({"proposal_id": options.approve}),
    )?;
    let scan_contract_path = scan
        .contract_path
        .ok_or_else(|| evolution_error(workspace, "approved scan did not produce a contract"))?;
    let proposals_path = run_dir.join("proposals.json");
    fs::copy(&scan.proposals_path, &proposals_path).map_err(|error| AppError::Io {
        path: proposals_path,
        source: error,
    })?;
    let contract_path = run_dir.join("contract.yaml");
    fs::copy(&scan_contract_path, &contract_path).map_err(|error| AppError::Io {
        path: contract_path.clone(),
        source: error,
    })?;
    let contract: sge_protocol::ContractDocument =
        serde_yaml::from_str(&fs::read_to_string(&contract_path).map_err(|error| {
            AppError::Io {
                path: contract_path.clone(),
                source: error,
            }
        })?)
        .map_err(|error| evolution_error(&contract_path, error.to_string()))?;
    journal_state(
        &journal,
        JournalState::Approved,
        json!({"contract": contract_path}),
    )?;

    let declared_files = BTreeMap::from([("instructions.md".to_string(), source_content)]);
    let safety_clauses = required_safety_clauses(
        declared_files
            .get("instructions.md")
            .expect("instructions are declared"),
    );
    let evidence_refs = contract.inputs.clone();
    let provider = RecordedModelProvider::load(&options.provider_fixture)
        .map_err(|error| evolution_error(&options.provider_fixture, error.to_string()))?;
    let operator = SkillPromptMutation::new(provider);
    let context = MutationContext {
        target: &target_ref,
        declared_files: &declared_files,
        required_safety_clauses: &safety_clauses,
        intent: &contract.intent,
        evidence_refs: &evidence_refs,
        budget: RequestBudget {
            max_prompt_tokens: 2_000,
            max_completion_tokens: 2_000,
            timeout_ms: 2_000,
        },
    };
    journal_state(
        &journal,
        JournalState::Mutating,
        json!({"operator": operator.descriptor().id}),
    )?;
    let patches = operator
        .propose(context.clone())
        .await
        .map_err(|error| evolution_error(workspace, error.to_string()))?;
    if patches.len() != options.candidate_count {
        return Err(evolution_error(
            &options.provider_fixture,
            format!(
                "provider returned {} candidates, expected {}",
                patches.len(),
                options.candidate_count
            ),
        ));
    }

    journal_state(
        &journal,
        JournalState::Evaluating,
        json!({"candidate_count": patches.len()}),
    )?;
    let fail_fast = contract
        .extensions
        .get("fail_fast")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let mut candidates = Vec::with_capacity(patches.len());
    for (index, patch) in patches.iter().enumerate() {
        let candidate_id = format!("candidate-{}", index + 1);
        let worktree_path = workspace
            .join(".singularity/worktrees")
            .join(&run_id)
            .join(&candidate_id);
        let evidence_path = run_dir
            .join("candidates")
            .join(&candidate_id)
            .join("evaluation.json");
        repo.checkout_candidate(&baseline_revision, &worktree_path)
            .map_err(|error| evolution_error(&worktree_path, error.to_string()))?;

        let applied = operator
            .apply(patch, &context)
            .map_err(|error| evolution_error(&worktree_path, error.to_string()))?;
        let candidate_source = worktree_path.join(&patch.path);
        fs::write(&candidate_source, applied.complete_replacement.as_bytes()).map_err(|error| {
            AppError::Io {
                path: candidate_source.clone(),
                source: error,
            }
        })?;
        let candidate_revision = repo
            .snapshot_candidate(
                &baseline_revision,
                &worktree_path,
                json!({
                    "op": "evolve-candidate",
                    "run_id": run_id,
                    "candidate_id": candidate_id,
                    "parent": baseline_revision.as_str(),
                }),
            )
            .map_err(|error| evolution_error(&worktree_path, error.to_string()))?;

        let evaluation =
            evaluate_instructions(&suite, &applied.complete_replacement, &worktree_path);
        let (evaluation, rejection_reason) = match evaluation {
            Ok(report) => (Some(report), None),
            Err(error) if !fail_fast => (None, Some(format!("evaluation failed: {error}"))),
            Err(error) => return Err(error),
        };
        candidates.push(CandidateOutcome {
            id: candidate_id,
            worktree_path,
            evidence_path,
            revision: Some(candidate_revision.as_str().to_string()),
            evaluation,
            rejection_reason,
        });
    }

    let selected_candidate = select_candidate(&suite, &baseline, &mut candidates);
    for candidate in &candidates {
        write_json(&candidate.evidence_path, candidate)?;
    }
    write_run_evidence(
        &run_dir,
        target,
        &run_id,
        &baseline_revision,
        &baseline_path,
        &baseline,
        &candidates,
        selected_candidate.as_deref(),
        &patches,
        &source_before,
    )?;

    if fs::read(&standard_source).map_err(|error| AppError::Io {
        path: standard_source.clone(),
        source: error,
    })? != source_before
    {
        return Err(AppError::StandardSourceMutated {
            path: standard_source,
        });
    }

    journal_state(
        &journal,
        JournalState::ReviewPending,
        json!({"selected_candidate": selected_candidate}),
    )?;
    Ok(EvolveOutcome {
        run_id,
        target: target.to_string(),
        baseline_path,
        contract_path,
        journal_path,
        candidates,
        selected_candidate,
    })
}

pub fn test_workspace(
    workspace: impl AsRef<Path>,
    target: &str,
    candidate: Option<&Path>,
) -> Result<TestOutcome> {
    let workspace = workspace.as_ref();
    let target_ref = TargetRef::from_str(target)
        .map_err(|error| evolution_error(workspace, error.to_string()))?;
    let standard_dir = workspace.join("skills").join(target_ref.name());
    let source_dir = candidate.unwrap_or(&standard_dir);
    let source_path = source_dir.join("instructions.md");
    let instructions = fs::read_to_string(&source_path).map_err(|error| AppError::Io {
        path: source_path.clone(),
        source: error,
    })?;
    let suite = read_suite(&standard_dir.join("evals/code-review.yaml"))?;
    let evaluation = evaluate_instructions(&suite, &instructions, source_dir)?;
    Ok(TestOutcome {
        target: target.to_string(),
        source_path,
        evaluation,
    })
}

fn select_candidate(
    suite: &Suite,
    baseline: &EvaluationReport,
    candidates: &mut [CandidateOutcome],
) -> Option<String> {
    let mut selected_index: Option<usize> = None;
    let mut selected_primary = 0.0;

    for index in 0..candidates.len() {
        let Some(evaluation) = candidates[index].evaluation.as_ref() else {
            continue;
        };
        let gate_violations = suite.objective.gate_violations(&evaluation.metrics);
        if !gate_violations.is_empty() {
            candidates[index].rejection_reason =
                Some(format!("hard gate failed: {}", gate_violations.join(", ")));
            continue;
        }
        let regressions = suite
            .objective
            .protected_regressions(&evaluation.metrics, &baseline.metrics);
        if !regressions.is_empty() {
            candidates[index].rejection_reason = Some(format!(
                "protected metrics regressed: {}",
                regressions.join(", ")
            ));
            continue;
        }

        let primary = suite.objective.primary_value(&evaluation.metrics);
        if selected_index.is_none() || suite.objective.primary_is_better(primary, selected_primary)
        {
            if let Some(previous) = selected_index {
                candidates[previous].rejection_reason =
                    Some("lower primary objective than selected candidate".to_string());
            }
            selected_index = Some(index);
            selected_primary = primary;
        } else {
            candidates[index].rejection_reason =
                Some("lower primary objective than selected candidate".to_string());
        }
    }

    selected_index.map(|index| candidates[index].id.clone())
}

pub(crate) fn evaluate_instructions(
    suite: &Suite,
    instructions: &str,
    workspace_path: &Path,
) -> Result<EvaluationReport> {
    if instructions.contains("[evaluation-error]") {
        return Err(evolution_error(
            workspace_path,
            "candidate requested deterministic evaluation failure",
        ));
    }
    let ordered_cases = suite.cases.iter().collect::<Vec<&Case>>();
    let runner = SuiteRunner::new(DeterministicGrader);
    let metadata = RunMeta {
        timestamp_secs: 0,
        workspace_path: workspace_path.display().to_string(),
        env_vars: BTreeMap::new(),
    };
    let lower = instructions.to_ascii_lowercase();
    Ok(runner.evaluate(
        suite,
        &ordered_cases,
        |case| {
            let mut findings = Vec::new();
            for expected in &case.expected_findings {
                let supported = match expected.category.as_str() {
                    "sql_injection" => lower.contains("sql string concatenation"),
                    "secret_leak" => lower.contains("flag secrets"),
                    "style" => lower.contains("spacing"),
                    _ => false,
                };
                if supported {
                    findings.push(ActualFinding {
                        category: expected.category.clone(),
                        severity: expected.severity,
                        message: Some(match expected.category.as_str() {
                            "sql_injection" => {
                                "avoid unsafe string concatenation in SQL queries".to_string()
                            }
                            "secret_leak" => "hardcoded secret detected".to_string(),
                            "style" => "spacing issue detected".to_string(),
                            _ => "finding detected".to_string(),
                        }),
                    });
                }
            }
            (findings, 10, instructions.len() as u64)
        },
        &metadata,
    ))
}

fn required_safety_clauses(instructions: &str) -> Vec<String> {
    instructions
        .lines()
        .filter_map(|line| line.trim().strip_prefix("- "))
        .map(str::to_string)
        .collect()
}

pub(crate) fn read_suite(path: &Path) -> Result<Suite> {
    let content = fs::read_to_string(path).map_err(|error| AppError::Io {
        path: path.to_path_buf(),
        source: error,
    })?;
    Suite::from_yaml(&content).map_err(|error| evolution_error(path, error.to_string()))
}

fn create_run_dir(workspace: &Path) -> Result<(String, PathBuf)> {
    let root = workspace.join(".singularity/runs");
    fs::create_dir_all(&root).map_err(|error| AppError::Io {
        path: root.clone(),
        source: error,
    })?;
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    for attempt in 0..100_u8 {
        let run_id = format!("evolve-{nanos:x}-{:x}-{attempt}", std::process::id());
        let path = root.join(&run_id);
        match fs::create_dir(&path) {
            Ok(()) => return Ok((run_id, path)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(AppError::Io {
                    path,
                    source: error,
                });
            }
        }
    }
    Err(evolution_error(
        &root,
        "could not allocate a unique evolution run directory",
    ))
}

fn journal_state(
    journal: &AppendOnlyJournal,
    state: JournalState,
    payload: serde_json::Value,
) -> Result<()> {
    journal
        .append(state, payload)
        .map(|_| ())
        .map_err(|error| evolution_error(Path::new("journal.ndjson"), error.to_string()))
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| AppError::Io {
            path: parent.to_path_buf(),
            source: error,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value)
        .map_err(|error| evolution_error(path, error.to_string()))?;
    fs::write(path, bytes).map_err(|error| AppError::Io {
        path: path.to_path_buf(),
        source: error,
    })
}

#[allow(clippy::too_many_arguments)]
fn write_run_evidence(
    run_dir: &Path,
    target: &str,
    run_id: &str,
    baseline_revision: &Revision,
    baseline_path: &Path,
    baseline: &EvaluationReport,
    candidates: &[CandidateOutcome],
    selected_candidate: Option<&str>,
    patches: &[sge_evolution::operator::MutationPatch],
    source_before: &[u8],
) -> Result<()> {
    let selected_index = selected_candidate.and_then(|selected| {
        candidates
            .iter()
            .position(|candidate| candidate.id == selected)
    });
    let selected_revision = selected_index.and_then(|index| candidates[index].revision.clone());

    let mutation_path = run_dir.join("mutation.patch");
    let mutation = selected_index
        .map(|index| {
            render_mutation_patch(
                std::str::from_utf8(source_before).unwrap_or_default(),
                &patches[index].complete_replacement,
                &candidates[index].id,
            )
        })
        .unwrap_or_else(|| "# No candidate selected\n".to_string());
    fs::write(&mutation_path, mutation).map_err(|source| AppError::Io {
        path: mutation_path,
        source,
    })?;

    let baseline_relative = relative_path(run_dir, baseline_path)?;
    let baseline_evidence_hash = sha256_file(baseline_path)?;
    let mut replay_candidates = Vec::with_capacity(candidates.len());
    let mut decision_rows = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let evidence_path = relative_path(run_dir, &candidate.evidence_path)?;
        let evidence_hash = sha256_file(&candidate.evidence_path)?;
        let replay_hash = candidate
            .evaluation
            .as_ref()
            .map(|evaluation| evaluation.normalized_replay_hash.clone());
        replay_candidates.push(ReplayCandidate {
            id: candidate.id.clone(),
            revision: candidate.revision.clone().unwrap_or_default(),
            evidence_path: evidence_path.clone(),
            evidence_hash: evidence_hash.clone(),
            normalized_replay_hash: replay_hash.clone(),
        });
        let status = if Some(candidate.id.as_str()) == selected_candidate {
            "selected".to_string()
        } else {
            candidate
                .rejection_reason
                .clone()
                .unwrap_or_else(|| "not selected".to_string())
        };
        decision_rows.push((
            candidate.id.clone(),
            status,
            evidence_path,
            evidence_hash,
            replay_hash.unwrap_or_else(|| "not available".to_string()),
        ));
    }

    let replay = ReplayDocument {
        schema: REPLAY_V1.to_string(),
        run_id: run_id.to_string(),
        target: target.to_string(),
        baseline_revision: baseline_revision.as_str().to_string(),
        baseline_evidence_path: baseline_relative,
        baseline_evidence_hash,
        baseline_replay_hash: baseline.normalized_replay_hash.clone(),
        candidates: replay_candidates,
        selected_candidate: selected_candidate.map(str::to_string),
        selected_revision,
    };
    let replay_path = run_dir.join("replay.yaml");
    write_yaml(&replay_path, &replay)?;

    let mut decision = format!(
        "# Evolution Decision\n\n\
         - Run: `{run_id}`\n\
         - Target: `{target}`\n\
         - Selected candidate: `{}`\n\
         - Baseline metrics: `baseline.json`\n\
         - Baseline replay hash: `{}`\n\n\
         ## Candidate Evidence\n\n",
        selected_candidate.unwrap_or("none"),
        baseline.normalized_replay_hash,
    );
    for (id, status, path, evidence_hash, replay_hash) in decision_rows {
        decision.push_str(&format!(
            "- `{id}`: {status}; metrics `{path}`; evidence SHA-256 `{evidence_hash}`; replay `{replay_hash}`\n"
        ));
    }
    decision.push_str(
        "\n## Trust Boundary\n\n\
         Model-supplied content is stored only in `mutation.patch` and is not trusted rationale. \
         This decision is rendered from typed metrics, gate results, paths, and hashes.\n",
    );
    let decision_path = run_dir.join("decision.md");
    fs::write(&decision_path, decision).map_err(|source| AppError::Io {
        path: decision_path,
        source,
    })
}

fn render_mutation_patch(before: &str, after: &str, candidate_id: &str) -> String {
    let mut patch = format!("--- baseline/instructions.md\n+++ {candidate_id}/instructions.md\n");
    for line in before.lines() {
        patch.push_str(&format!("-{line}\n"));
    }
    for line in after.lines() {
        patch.push_str(&format!("+{line}\n"));
    }
    patch
}

fn relative_path(root: &Path, path: &Path) -> Result<String> {
    path.strip_prefix(root)
        .map(|relative| relative.display().to_string())
        .map_err(|error| evolution_error(path, error.to_string()))
}

fn sha256_file(path: &Path) -> Result<String> {
    let bytes = fs::read(path).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
}

fn write_yaml(path: &Path, value: &impl Serialize) -> Result<()> {
    let content =
        serde_yaml::to_string(value).map_err(|error| evolution_error(path, error.to_string()))?;
    fs::write(path, content).map_err(|source| AppError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn evolution_error(path: impl AsRef<Path>, message: impl Into<String>) -> AppError {
    AppError::Evolution {
        path: path.as_ref().to_path_buf(),
        message: message.into(),
    }
}
