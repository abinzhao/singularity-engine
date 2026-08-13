# P2 Full Evolution Surface Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the proven P1 workflow to Agent, Skill, and Rule assets, add governed memory, and expose all fourteen mutation operators with enforceable maturity and risk policies.

**Architecture:** Reuse one operator registry and one evolution state machine. Asset-specific behavior lives in typed capabilities and validators, not separate pipelines. Keep high-risk operators behind Preview policy and never allow automatic apply.

**Tech Stack:** Rust, serde, schemars, proptest, insta, git2, existing P0–P1 crates.

---

## File Map

```text
crates/sge-domain/src/capability.rs            asset/operator capability model
crates/sge-evolution/src/registry.rs           fourteen-operator registry
crates/sge-evolution/src/operators/            one focused module per operator
crates/sge-evolution/src/policy.rs             maturity, risk, approval policy
crates/sge-app/src/memory.rs                    memory commands and confirmation
crates/sge-protocol/src/memory.rs               governed memory schema
fixtures/operators/<operator>/                  golden input/output/failure fixtures
fixtures/assets/{agent,skill,rule}/              conformance fixtures
```

### Task 1: Add Agent and Rule conformance fixtures

**Files:**
- Create: `fixtures/assets/agent/*`
- Create: `fixtures/assets/rule/*`
- Create: `crates/sge-app/tests/asset_lifecycle.rs`

- [ ] Write failing lifecycle tests that import, validate, snapshot, diff, and undo one Agent and one Rule.
- [ ] Run `cargo test -p sge-app --test asset_lifecycle`; expect failure.
- [ ] Extend import and target resolution through `ArtifactCapabilities`, not `match` branches in CLI handlers.
- [ ] Run the lifecycle test; expect PASS.
- [ ] Commit with `feat: support Agent and Rule lifecycles`.

Required interface:

```rust
pub trait ArtifactCapabilities {
    fn mutable_surfaces(&self) -> &[MutableSurface];
    fn evaluation_requirements(&self) -> EvaluationRequirements;
    fn validate_composition(&self, workspace: &WorkspaceView) -> Result<()>;
}
```

### Task 2: Implement governed memory records

**Files:**
- Modify: `crates/sge-protocol/src/memory.rs`
- Create: `crates/sge-app/src/memory.rs`
- Create: `crates/sge-app/tests/memory_governance.rs`
- Modify: `crates/sge-cli/src/main.rs`

- [ ] Write tests proving Fact requires a source, Preference requires `source.kind=user`, Failure requires evidence or explicit confirmation, and proposed records do not affect evolution.
- [ ] Run `cargo test -p sge-app --test memory_governance`; expect failure.
- [ ] Implement `MemoryRecord` with `id`, `kind`, `statement`, `scope`, `source`, `confidence`, `status`, `created_at`, `expires_at`, and evidence hashes.
- [ ] Implement `sge memory add|propose|list|show|diff|remove`.
- [ ] Ensure removal creates a lineage revision rather than deleting history.
- [ ] Run tests; expect PASS.
- [ ] Commit with `feat: govern versioned memory records`.

### Task 3: Add operator descriptors and policy enforcement

**Files:**
- Create: `crates/sge-evolution/src/{registry,policy}.rs`
- Create: `crates/sge-evolution/tests/operator_policy.rs`

- [ ] Write a table-driven test containing all fourteen exact operator IDs, risk, maturity, supported asset kinds, allowed surfaces, required sandbox, and automatic-apply permission.
- [ ] Run `cargo test -p sge-evolution --test operator_policy`; expect failure.
- [ ] Implement `OperatorDescriptor` and a registry that rejects duplicate IDs.
- [ ] Enforce: high-risk operators require Preview or Experimental, explicit review, and `automatic_apply=false`.
- [ ] Run tests; expect PASS with registry count `14`.
- [ ] Commit with `feat: register mutation operator policies`.

Descriptor shape:

```rust
pub struct OperatorDescriptor {
    pub id: OperatorId,
    pub risk: RiskLevel,
    pub maturity: Maturity,
    pub asset_kinds: BTreeSet<ArtifactKind>,
    pub surfaces: BTreeSet<MutableSurface>,
    pub sandbox: SandboxRequirement,
    pub automatic_apply: bool,
}
```

### Task 4: Implement low-risk operators

**Files:**
- Create: `crates/sge-evolution/src/operators/{prompt,skill_prompt,memory_retention,verification,failure_adaptation}.rs`
- Create: `fixtures/operators/{prompt_mutation,skill_prompt_mutation,memory_retention_policy,verification_policy,failure_pattern_adaptation}/`
- Create: `crates/sge-evolution/tests/low_risk_operators.rs`

- [ ] For each operator, add a golden success fixture, stale-source fixture, forbidden-path fixture, and required-clause preservation fixture.
- [ ] Run the focused test; expect failure for unimplemented operators.
- [ ] Implement operators using structured replacements and expected source hashes.
- [ ] Verify no low-risk operator can add permissions or modify undeclared assets.
- [ ] Run `cargo test -p sge-evolution --test low_risk_operators`; expect PASS.
- [ ] Commit with `feat: implement low-risk mutation operators`.

### Task 5: Implement medium-risk operators

**Files:**
- Create: `crates/sge-evolution/src/operators/{tool_selection,skill_tool,memory_schema,planning,reasoning,context}.rs`
- Create: `fixtures/operators/<six-operator-ids>/`
- Create: `crates/sge-evolution/tests/medium_risk_operators.rs`

- [ ] Write tests requiring permission deltas, dependency manifests, memory migrations, rollback plans, and protected-metric budgets.
- [ ] Run the focused test; expect failure.
- [ ] Implement:
  - `tool_selection`
  - `skill_tool_mutation`
  - `memory_schema`
  - `planning_policy`
  - `reasoning_depth`
  - `context_window_strategy`
- [ ] Reject tool additions without declared executables and permissions.
- [ ] Reject memory migrations without reversible `up` and `down` transforms.
- [ ] Run tests; expect PASS.
- [ ] Commit with `feat: implement medium-risk mutation operators`.

### Task 6: Implement high-risk structural and code operators

**Files:**
- Create: `crates/sge-evolution/src/operators/{skill_split,skill_merge,tool_implementation}.rs`
- Create: `fixtures/operators/{skill_split,skill_merge,tool_implementation}/`
- Create: `crates/sge-evolution/tests/high_risk_operators.rs`

- [ ] Write tests proving all three operators refuse Content-only execution, cannot auto-apply, and require impact analysis.
- [ ] Add split tests for caller mapping and merge tests for trigger/permission conflicts.
- [ ] Add tool implementation tests for generated-file allowlists and mandatory project tests.
- [ ] Run the focused test; expect failure.
- [ ] Implement preview generation and validation. Do not implement silent dependency installation.
- [ ] Run tests; expect PASS.
- [ ] Commit with `feat: add guarded structural mutation operators`.

### Task 7: Add multi-generation and stopping policy

**Files:**
- Create: `crates/sge-evolution/src/{generation,stopping}.rs`
- Create: `crates/sge-evolution/tests/stopping.rs`
- Modify: `crates/sge-app/src/evolve.rs`

- [ ] Write property tests for target reached, two stagnant generations, exhausted cost/time/generation budgets, hard-gate failure, insufficient evidence, and user cancellation.
- [ ] Run `cargo test -p sge-evolution --test stopping`; expect failure.
- [ ] Implement deterministic `StopReason` values and persist them in Evidence.
- [ ] Ensure only selected candidates become parents of the next generation.
- [ ] Run tests; expect PASS.
- [ ] Commit with `feat: enforce evolution stopping policies`.

### Task 8: Add cross-asset composition validation

**Files:**
- Create: `crates/sge-domain/src/composition.rs`
- Create: `crates/sge-app/tests/composition.rs`

- [ ] Write tests for missing Skill references, Rule priority cycles, incompatible permissions, memory scope violations, and duplicate capability providers.
- [ ] Run the focused test; expect failure.
- [ ] Implement a validation graph with stable diagnostic codes and source locations.
- [ ] Run tests; expect PASS.
- [ ] Commit with `feat: validate Agent composition boundaries`.

### Task 9: Expose P2 CLI behavior

**Files:**
- Modify: `crates/sge-cli/src/main.rs`
- Create: `crates/sge-cli/tests/full_surface.rs`

- [ ] Add CLI tests for `sge evolve` against Agent, Skill, and Rule targets and for operator selection via `--operator`.
- [ ] Verify Preview/Experimental warnings are present in text and JSON output.
- [ ] Implement `sge branch`, `pack`, and complete `history/diff` target support.
- [ ] Run `cargo test -p sge-cli --test full_surface`; expect PASS.
- [ ] Commit with `feat: expose the full evolution surface`.

### Task 10: Run the P2 gate

**Files:**
- Create: `docs/evidence/p2/summary.md`
- Modify: `.github/workflows/ci.yml`

- [ ] Add operator conformance matrix CI that fails when registry descriptors and fixtures diverge.
- [ ] Run:

```bash
cargo nextest run --workspace
cargo test -p sge-evolution --test operator_policy
cargo test -p sge-evolution --test low_risk_operators
cargo test -p sge-evolution --test medium_risk_operators
cargo test -p sge-evolution --test high_risk_operators
cargo xtask architecture
```

- [ ] Record the matrix of 14 operators, maturity, tests, sandbox, and apply policy.
- [ ] Commit with `test: prove the full evolution surface`.

## P2 Exit Gate

- Agent, Skill, and Rule use one lifecycle and one state machine.
- Registry contains exactly fourteen operators.
- Every operator has positive, negative, containment, and rollback fixtures.
- High-risk operators cannot auto-apply.
- Proposed memory cannot influence evolution until confirmed.
- Composition diagnostics are deterministic and source-located.

