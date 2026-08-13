# P1 Vertical Evolution Slice Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver one complete, recoverable Skill evolution path: import, scan, approve a direction, mutate the Skill prompt, evaluate candidates, explain the decision, inspect history, and undo.

**Architecture:** Implement the evolution workflow as a persisted state machine in `sge-app`, with operator logic in `sge-evolution`, deterministic graders in `sge-eval`, model calls behind `sge-provider`, and all candidate writes confined to internal worktrees. The first slice supports `skill_prompt_mutation`; later operators must conform to the same contract.

**Tech Stack:** Rust, async traits, reqwest, serde_json, git2, assert_cmd, wiremock, insta, proptest.

---

## File Map

```text
crates/sge-evolution/            run state machine, scan proposals, operator trait
crates/sge-eval/                 suites, cases, deterministic graders, comparison
crates/sge-provider/             provider contract and recorded test transport
crates/sge-app/                  import/scan/evolve/test/explain/history/undo use cases
crates/sge-cli/                  corresponding commands and JSON output
fixtures/evolution/basic-skill/  reproducible vulnerable Skill fixture
fixtures/provider/               recorded provider responses
```

### Task 1: Import and validate an existing Skill

**Files:**
- Create: `crates/sge-app/src/import.rs`
- Create: `crates/sge-app/tests/import_skill.rs`
- Modify: `crates/sge-cli/src/main.rs`
- Create: `fixtures/evolution/basic-skill/{skill.yaml,instructions.md}`

- [ ] **Step 1: Write import tests**

Test that import copies only declared Skill files into the standard source tree, rejects symlinks escaping the source root, and snapshots the imported revision in internal Git.

- [ ] **Step 2: Verify failure**

Run: `cargo test -p sge-app --test import_skill`

Expected: FAIL because import is missing.

- [ ] **Step 3: Implement `ImportArtifact`**

Parse the source protocol, validate `kind: skill`, canonicalize paths, reject duplicate names, copy via a temporary directory, and atomically rename into `skills/<name>`.

- [ ] **Step 4: Wire `sge import`**

Support:

```bash
sge import ./path/to/skill
sge import ./path/to/skill --json
```

JSON must include `target`, `revision`, and `warnings`.

- [ ] **Step 5: Verify**

Run:

```bash
cargo test -p sge-app --test import_skill
cargo test -p sge-cli
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/sge-app crates/sge-cli fixtures/evolution
git commit -m "feat: import standard Skill artifacts"
```

### Task 2: Define evaluation suites and deterministic graders

**Files:**
- Create: `crates/sge-eval/src/{lib,suite,case,grader,metrics,runner}.rs`
- Create: `crates/sge-eval/tests/deterministic_suite.rs`
- Create: `fixtures/evolution/basic-skill/evals/code-review.yaml`

- [ ] **Step 1: Write grader tests**

Define a fixture with three cases and exact assertions for required findings. Test that missing a SQL injection finding lowers `task_success`, while latency and token metrics remain separately reported.

- [ ] **Step 2: Verify failure**

Run: `cargo test -p sge-eval --test deterministic_suite`

Expected: FAIL because evaluation types are absent.

- [ ] **Step 3: Implement metric vectors**

```rust
pub struct MetricVector {
    pub task_success: f64,
    pub safety: f64,
    pub latency_p95_ms: u64,
    pub token_cost: u64,
    pub stability: f64,
    pub compatibility: f64,
}
```

Do not compute a universal weighted score. Implement explicit contract comparison with hard gates, primary objective, and protected metrics.

- [ ] **Step 4: Add repeat normalization**

Normalize case order, timestamps, and environment-specific paths before hashing replay output.

- [ ] **Step 5: Verify**

Run: `cargo test -p sge-eval`

Expected: PASS with byte-stable normalized replay snapshots.

- [ ] **Step 6: Commit**

```bash
git add crates/sge-eval fixtures/evolution/basic-skill/evals
git commit -m "feat: evaluate Skills with metric vectors"
```

### Task 3: Add the model provider contract and recorded transport

**Files:**
- Create: `crates/sge-provider/src/{lib,model,transport,recorded}.rs`
- Create: `crates/sge-provider/tests/contract.rs`
- Create: `fixtures/provider/scan-proposals.json`
- Create: `fixtures/provider/prompt-candidates.json`

- [ ] **Step 1: Write provider contract tests**

Test timeout, malformed JSON, over-budget usage, and valid structured response. Ensure the provider returns data, not file writes or shell commands.

- [ ] **Step 2: Verify failure**

Run: `cargo test -p sge-provider --test contract`

Expected: FAIL because the provider trait is missing.

- [ ] **Step 3: Implement provider-neutral request types**

```rust
#[async_trait::async_trait]
pub trait ModelProvider {
    async fn generate<T: serde::de::DeserializeOwned>(
        &self,
        request: ModelRequest,
        budget: RequestBudget,
    ) -> Result<ModelResponse<T>, ProviderError>;
}
```

The request includes a Data Manifest and explicit JSON Schema. Reject responses that fail schema validation.

- [ ] **Step 4: Implement recorded provider**

Use fixture responses in CI. Do not add live credentials or network calls in this task.

- [ ] **Step 5: Verify**

Run: `cargo test -p sge-provider`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/sge-provider fixtures/provider
git commit -m "feat: add structured model provider contract"
```

### Task 4: Implement scan proposals and Evolution Contract approval

**Files:**
- Create: `crates/sge-evolution/src/{lib,scan,proposal,operator,state}.rs`
- Create: `crates/sge-evolution/tests/scan.rs`
- Create: `crates/sge-app/src/scan.rs`
- Modify: `crates/sge-cli/src/main.rs`

- [ ] **Step 1: Write proposal ordering tests**

Given evidence with repeated SQL injection misses, test that proposals contain evidence references, risk, affected files, evaluation method, estimated range, and confidence. Do not assert a fabricated exact percentage.

- [ ] **Step 2: Verify failure**

Run: `cargo test -p sge-evolution --test scan`

Expected: FAIL because scan types are absent.

- [ ] **Step 3: Implement scan**

Gather only declared evaluation results, confirmed failure memories, and artifact structure. Produce 2–5 proposals. Persist them to `proposals.json`.

- [ ] **Step 4: Implement approval**

Convert a selected proposal or `--goal` into a versioned Contract. Require explicit confirmation in TTY mode and `--approve <proposal-id>` in non-interactive mode.

- [ ] **Step 5: Wire `sge scan`**

Support:

```bash
sge scan skill:code-review
sge scan skill:code-review --json
```

- [ ] **Step 6: Verify**

Run:

```bash
cargo test -p sge-evolution
cargo test -p sge-app scan
cargo test -p sge-cli
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add crates/sge-evolution crates/sge-app crates/sge-cli
git commit -m "feat: propose evidence-backed evolution directions"
```

### Task 5: Implement the first mutation operator

**Files:**
- Create: `crates/sge-evolution/src/operators/{mod,skill_prompt}.rs`
- Create: `crates/sge-evolution/tests/skill_prompt_operator.rs`

- [ ] **Step 1: Write operator containment tests**

Test that `skill_prompt_mutation` may change only `skills/<target>/instructions.md`, preserves required safety clauses, and rejects absolute paths or undeclared files in model output.

- [ ] **Step 2: Verify failure**

Run: `cargo test -p sge-evolution --test skill_prompt_operator`

Expected: FAIL because the operator is missing.

- [ ] **Step 3: Define the operator trait**

```rust
#[async_trait::async_trait]
pub trait MutationOperator {
    fn descriptor(&self) -> OperatorDescriptor;
    fn allowed_paths(&self, target: &TargetRef) -> Vec<PathPolicy>;
    async fn propose(&self, context: MutationContext<'_>) -> Result<Vec<MutationPatch>>;
    fn validate(&self, patch: &MutationPatch, context: &MutationContext<'_>) -> Result<()>;
}
```

- [ ] **Step 4: Implement structured patch application**

The model returns the complete replacement content plus expected source hash. Apply only when the source hash still matches. Reject arbitrary unified diff paths.

- [ ] **Step 5: Verify**

Run: `cargo test -p sge-evolution --test skill_prompt_operator`

Expected: PASS, including path traversal and stale-source tests.

- [ ] **Step 6: Commit**

```bash
git add crates/sge-evolution
git commit -m "feat: mutate Skill prompts safely"
```

### Task 6: Orchestrate candidate generation and evaluation

**Files:**
- Create: `crates/sge-app/src/evolve.rs`
- Create: `crates/sge-app/tests/evolve_skill.rs`
- Modify: `crates/sge-cli/src/main.rs`

- [ ] **Step 1: Write the vertical E2E test**

The test must initialize a workspace, import the fixture Skill, run its baseline, approve a proposal, generate three candidates, evaluate them, select the only candidate satisfying the Contract, and leave the standard source unchanged.

- [ ] **Step 2: Verify failure**

Run: `cargo test -p sge-app --test evolve_skill`

Expected: FAIL because orchestration is missing.

- [ ] **Step 3: Implement persisted transitions**

Persist:

```text
Prepared → Baseline → Diagnosed → Approved → Mutating
→ Evaluating → ReviewPending
```

Each candidate receives its own worktree and evidence directory. A candidate failure must not abort unrelated candidates unless the Contract says `fail_fast`.

- [ ] **Step 4: Implement selection**

Filter hard-gate failures first. Select by primary objective only among candidates respecting protected metrics. Record why every loser was rejected.

- [ ] **Step 5: Wire `sge evolve` and `sge test`**

`evolve` stops at `ReviewPending`. `test` can rerun a baseline, candidate, or replay.

- [ ] **Step 6: Verify**

Run:

```bash
cargo test -p sge-app --test evolve_skill
cargo test -p sge-cli
```

Expected: PASS; no candidate writes appear in the standard source.

- [ ] **Step 7: Commit**

```bash
git add crates/sge-app crates/sge-cli
git commit -m "feat: run isolated Skill evolution"
```

### Task 7: Generate explanation, history, diff, and replay evidence

**Files:**
- Create: `crates/sge-app/src/{explain,history,replay}.rs`
- Create: `crates/sge-app/tests/evidence.rs`
- Modify: `crates/sge-cli/src/main.rs`

- [ ] **Step 1: Write evidence completeness tests**

Assert every run contains `contract.yaml`, `baseline.json`, `proposals.json`, candidate evaluations, `decision.md`, `mutation.patch`, and `replay.yaml`. The decision must reference actual metric files and evidence hashes.

- [ ] **Step 2: Verify failure**

Run: `cargo test -p sge-app --test evidence`

Expected: FAIL because evidence generation is incomplete.

- [ ] **Step 3: Implement evidence rendering**

Generate Markdown from typed data. Never interpolate raw model prose as trusted rationale; label model-supplied analysis explicitly.

- [ ] **Step 4: Wire read-only commands**

```bash
sge explain <run-id>
sge history skill:code-review
sge diff <revision-a> <revision-b>
sge test --replay <run-id>
```

- [ ] **Step 5: Verify**

Run:

```bash
cargo test -p sge-app --test evidence
cargo run -p sge-cli -- explain --help
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/sge-app crates/sge-cli
git commit -m "feat: explain and replay evolution runs"
```

### Task 8: Apply and undo the winning internal revision

**Files:**
- Create: `crates/sge-app/src/{apply,undo}.rs`
- Create: `crates/sge-app/tests/apply_undo.rs`
- Modify: `crates/sge-cli/src/main.rs`

- [ ] **Step 1: Write atomic apply/undo tests**

Inject failure after backup and before replacement. Assert the standard source tree hash remains unchanged. On success, assert `undo` restores the exact prior tree.

- [ ] **Step 2: Verify failure**

Run: `cargo test -p sge-app --test apply_undo`

Expected: FAIL because apply is missing.

- [ ] **Step 3: Implement internal apply transaction**

Require `ReviewPending`, passing gates, and explicit approval. Copy through a staging directory, verify hashes, atomically replace, snapshot the applied revision, and journal completion.

- [ ] **Step 4: Implement undo**

Undo must accept a run ID or revision, create a new restoration revision, and never rewrite lineage history.

- [ ] **Step 5: Verify**

Run: `cargo test -p sge-app --test apply_undo`

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add crates/sge-app crates/sge-cli
git commit -m "feat: apply and undo proven revisions"
```

### Task 9: Run the P1 phase gate

**Files:**
- Create: `docs/evidence/p1/summary.md`
- Modify: `.github/workflows/ci.yml`

- [ ] **Step 1: Add vertical E2E CI job**

Run only recorded provider fixtures. Archive normalized evidence as a CI artifact.

- [ ] **Step 2: Execute the phase demo**

```bash
sge init /tmp/sge-p1-demo
cd /tmp/sge-p1-demo
sge import <repo>/fixtures/evolution/basic-skill
sge scan skill:code-review
sge evolve skill:code-review --approve proposal-1
sge test --replay <run-id>
sge explain <run-id>
sge undo <applied-run-id>
```

Expected: the run reaches review, evidence is complete, replay matches, and undo restores the original Skill.

- [ ] **Step 3: Run quality checks**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo xtask architecture
git diff --check
```

Expected: all commands exit `0`.

- [ ] **Step 4: Record evidence and commit**

```bash
git add .github docs/evidence/p1
git commit -m "test: prove the vertical evolution slice"
```

## P1 Exit Gate

- One imported Skill completes scan → contract → mutate → evaluate → review.
- Candidate writes remain isolated until explicit apply.
- Metric selection does not collapse into one universal score.
- Explanation and replay are generated from typed evidence.
- Apply and undo are atomic and fault-tested.
- CI uses no live model credentials.

