# SINGULARITY ENGINE V1 Master Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Deliver SINGULARITY ENGINE V1 as a local-first Rust CLI and portable host Skill that can evolve, evaluate, explain, apply, and roll back Agent, Skill, and Rule artifacts across five AI coding hosts.

**Architecture:** Build a modular Rust monolith with stable protocol crates, an append-only run journal, an internal bare Git repository, isolated candidate worktrees, pluggable mutation/evaluation/sandbox/provider contracts, and host adapters behind an anti-corruption layer. Keep user-authored YAML/Markdown as the source of truth; treat SQLite and generated host files as rebuildable projections.

**Tech Stack:** Rust stable, Cargo workspace, clap, tokio, serde, serde_yaml, schemars, git2, rusqlite, reqwest, wasmtime, tracing, insta, proptest, assert_cmd, predicates, tempfile, cargo-nextest, cargo-deny, cargo-audit, npm binary bootstrapper, GitHub Actions.

**Source Specification:** `docs/superpowers/specs/2026-08-13-singularity-engine-design.md`

---

## 1. Plan Decomposition

The V1 specification contains multiple independently testable subsystems. Execute these plans in order:

| Phase | Plan | Working Increment | GitHub Milestone |
|---|---|---|---|
| P0 | `2026-08-13-p0-protocol-foundation.md` | Valid workspace, five versioned protocols, internal Git store | `P0 Protocols` |
| P1 | `2026-08-13-p1-vertical-evolution-slice.md` | One Skill can scan, evolve, test, explain, and undo | `P1 Vertical Slice` |
| P2 | `2026-08-13-p2-full-evolution-surface.md` | Three asset kinds, memory governance, fourteen operators | `P2 Evolution Surface` |
| P3 | `2026-08-13-p3-host-skill-and-adapters.md` | Portable Skill and five transactional host adapters | `P3 Host Integration` |
| P4 | `2026-08-13-p4-security-sandbox-and-recovery.md` | Four execution backends, privacy controls, fault recovery | `P4 Hardening` |
| P5 | `2026-08-13-p5-release-engineering.md` | Cross-platform packages, CI, docs, reproducible V1 release | `P5 Release` |

Do not start a phase until the preceding phase exit gate passes. Preview-only work may be prototyped on an isolated branch, but it cannot change a frozen protocol without an approved ADR and migration.

## 2. Architecture Decision Record

### ADR-001: Modular Monolith With Protocol-First Boundaries

**Status:** Accepted

**Decision question:** How should V1 isolate protocol, evolution, evaluation, storage, security, and host integration responsibilities without paying distributed-system cost?

**Goals**

- Keep `sge` installable as one local binary.
- Make each subsystem independently testable.
- Prevent host-specific formats from contaminating standard artifacts.
- Make all mutations recoverable after process interruption.
- Preserve a future path to extract long-running workers only when justified.

**Non-goals**

- No network service topology.
- No shared multi-user state.
- No cloud control plane.
- No runtime requirement for SQLite beyond rebuildable indexing.

**Forces**

| Force | Rationale |
|---|---|
| Local-first distribution | A single binary minimizes setup and trust surface. |
| Protocol longevity | Artifact, Contract, Evidence, Memory, and Adapter schemas outlive implementations. |
| High-risk mutation | Candidate execution and host writes need explicit isolation and rollback. |
| Host churn | Five external host formats change independently. |
| V1 breadth | Fourteen operators require shared invariants rather than bespoke pipelines. |

**Decision**

Use a Cargo workspace that builds one `sge` binary. Enforce dependency direction through crate boundaries. Keep host adapters and mutation operators as in-process plugins registered through typed traits. Use internal events only for journal and observability; do not introduce a broker or daemon.

**Alternatives**

| Alternative | Decision | Reason |
|---|---|---|
| One large crate | Rejected | Fast initially, but protocol, host, and security boundaries become untestable. |
| Multiple local processes | Rejected for V1 | Adds IPC, lifecycle, packaging, and recovery complexity without a proven scaling need. |
| Cloud service plus thin CLI | Rejected | Violates local-first product boundary. |
| WASM plugins for every extension | Deferred | Useful later for third-party isolation; premature for first-party V1 operators. |

**Consequences**

| Positive | Negative |
|---|---|
| One binary and one failure domain | Compile time grows with workspace size |
| Explicit contracts and dependency checks | More crates and fixture maintenance |
| Easy local debugging and rollback | In-process bugs can still terminate the CLI |
| Host adapters remain replaceable | Adapter API must be designed carefully |

**Reversibility:** Two-way door. Extract a crate into a process only when independent scaling, release cadence, or crash isolation is measured as necessary. Expected reversal cost: medium, because trait contracts and serialized journal events already define the boundary.

**Reconsideration triggers**

- A sandboxed operation must survive CLI process termination independently.
- One subsystem requires a separate release cadence.
- Memory or CPU isolation cannot be achieved safely in process.
- A local daemon demonstrably reduces repeated startup cost by at least 30% on measured workflows.

**Responsibility:** `crates/sge-domain` owns shared language; `crates/sge-protocol` owns serialized contracts; architecture changes require ADR review and `cargo xtask architecture`.

## 3. Bounded-Context Map

| Context | Responsibility / Check Path | Model and Language | Upstream | Downstream | Relationship / Translation |
|---|---|---|---|---|---|
| CLI | `crates/sge-cli`; CLI snapshot tests | Commands, targets, user approvals | User / host Skill | Application | Customer/supplier; translates CLI input into application commands |
| Application | `crates/sge-app`; use-case integration tests | Scan, evolve, test, apply workflows | CLI | Evolution, Eval, Store, Adapter | Orchestrator; no domain logic in handlers |
| Domain | `crates/sge-domain`; unit and property tests | Artifact, Run, Contract, Candidate, Decision | Protocol | All core contexts | Shared kernel; only stable domain types |
| Protocol | `crates/sge-protocol`; schema fixtures | Versioned YAML/JSON documents | User files | Domain, Adapter, Store | Anti-corruption parsing and migration |
| Evolution | `crates/sge-evolution`; operator contract tests | Operator, proposal, candidate, selection | Application, Eval evidence | Sandbox, Eval, Store | Customer of Eval and Sandbox |
| Evaluation | `crates/sge-eval`; deterministic replay tests | Suite, case, grader, metric vector | Evolution, project tests | Application, Store | Separate ways from mutation logic |
| Store | `crates/sge-store`; crash/recovery tests | Internal Git, CAS, journal, projection | Application | All contexts | Shared infrastructure behind repository traits |
| Sandbox | `crates/sge-sandbox`; containment tests | Content, WASI, Process, Container execution | Evolution, Eval | Host OS/runtime | Anti-corruption layer over execution backends |
| Provider | `crates/sge-provider`; recorded contract tests | Model request, response, usage, data manifest | Application, Evolution | Remote/local models | Anti-corruption layer over providers |
| Adapter | `crates/sge-adapter` + `adapters/*`; golden tests | Detect, render, validate, apply, rollback | Application | External hosts | Anti-corruption layer; standard artifact to host format |
| Security | `crates/sge-security`; abuse-case tests | Path policy, network policy, secret redaction | All contexts | All side effects | Policy provider; deny by default |
| Host Skill | `skill/`; fixture tests | Natural-language intent to CLI JSON | Host model | CLI | Conformist to public CLI JSON contract |

### Dependency Direction

```text
sge-cli → sge-app → domain traits
                    ├── sge-evolution
                    ├── sge-eval
                    ├── sge-store
                    ├── sge-sandbox
                    ├── sge-provider
                    ├── sge-adapter
                    └── sge-security

sge-protocol → sge-domain
adapters/* → sge-adapter + sge-protocol
skill/* → public CLI JSON only
```

Infrastructure crates may depend on `sge-domain`; `sge-domain` must never depend on infrastructure, CLI, or a specific host.

## 4. Critical Dependency Decisions

| Dependency | Forces | Default | Failure Model | Fallback / Exit |
|---|---|---|---|---|
| Internal Git | Audit, branching, rollback | `git2` bare repo | lock failure, corrupt object, interrupted commit | verify before mutation; export source tree; replace backend behind repository trait |
| SQLite | Fast history and metric queries | Rebuildable projection | lock/corruption | delete and rebuild from Git + evidence; never sole source of truth |
| Model provider | Candidate analysis and generation | explicit configured provider | timeout, quota, malformed output | retry only before side effects; local provider; preserve run as resumable |
| WASI runtime | Portable restricted execution | `wasmtime` | trap, timeout, resource exhaustion | Process backend with explicit approval; Content backend for non-code mutations |
| Container runtime | High-risk execution | optional Docker/Podman adapter | unavailable daemon, image pull failure | mark backend unavailable; never silently downgrade high-risk run |
| Host filesystem | Apply generated assets | transactional adapter | conflict, permission failure, partial write | prepare/backup/atomic replace/smoke test/rollback |

## 5. Interaction-Style Decision

| Interaction | Decision | Reason |
|---|---|---|
| CLI command request/response | Default | User actions need immediate validation and an explicit result. |
| Persisted local journal | Required | Long model/evaluation operations must resume after interruption without a daemon. |
| In-process domain events | Allowed | Decouples evidence and telemetry updates without external infrastructure. |
| Local daemon | Rejected for V1 | Adds lifecycle and trust complexity without measured startup pressure. |
| External queue or event broker | Rejected | Conflicts with local-first and single-user operation. |
| Batch candidate evaluation | Required | Candidates are independent and can run with bounded concurrency. |
| Streaming model output | Optional presentation only | Structured result is authoritative; partial tokens never authorize side effects. |

Quota and overload handling stays local: the application scheduler limits candidate concurrency, Provider requests consume Contract budgets, and journaled work pauses rather than polling or creating unbounded retries.

## 6. Architectural Fitness Functions

| Property | Metric | Threshold / Rule | Source | Cadence | Failure Response | Local Check |
|---|---|---|---|---|---|---|
| Dependency direction | forbidden crate edges | zero violations | Cargo metadata graph | every PR | block merge | `cargo xtask architecture` |
| Protocol compatibility | old fixture parse + round-trip | all supported V1 fixtures pass | `schemas/fixtures` | every PR | block merge | `cargo test -p sge-protocol` |
| Deterministic replay | equal deterministic metric output | byte-identical normalized result | replay fixtures | every PR | block merge | `cargo test -p sge-eval replay` |
| Workspace isolation | writes outside allowed roots | zero | sandbox integration tests | every PR | block merge | `cargo test -p sge-sandbox containment` |
| Secret containment | known canary secret in logs/evidence | zero occurrences | redaction tests | every PR | block merge | `cargo test -p sge-security secret` |
| Host rollback | original tree restored after injected failure | exact tree hash match | adapter fault tests | every PR | block merge | `cargo test -p sge-adapter rollback` |
| CLI startup | warm `sge --version` | p95 ≤ 300 ms on CI reference runner | benchmark job | nightly | open regression issue | `cargo bench -p sge-cli startup` |
| Init latency | empty workspace initialization | p95 ≤ 2 s | benchmark job | nightly | investigate before RC | `cargo bench -p sge-app init` |
| Journal recovery | interrupted state classified | 100% fixture coverage | fault fixtures | every PR | block merge | `cargo test -p sge-store recovery` |
| Blast radius | candidate mutation outside its worktree | zero | file hash diff | every PR | abort run and quarantine operator | `cargo test -p sge-evolution isolation` |
| Public CLI JSON | backward-compatible fixture output | no breaking diff in V1 | CLI golden fixtures | every PR | require protocol ADR | `cargo insta test -p sge-cli` |

## 7. GitHub Delivery Model

No remote exists yet. When the repository is created, configure:

### Milestones

1. `P0 Protocols`
2. `P1 Vertical Slice`
3. `P2 Evolution Surface`
4. `P3 Host Integration`
5. `P4 Hardening`
6. `P5 Release`

### Labels

```text
area:protocol area:evolution area:eval area:store area:sandbox
area:provider area:adapter area:security area:cli area:docs
kind:feature kind:bug kind:security kind:adr kind:chore
risk:low risk:medium risk:high
maturity:stable maturity:preview maturity:experimental
blocked decision-needed good-first-issue
```

### Branch and Review Rules

- Protect `main`; require pull requests.
- Require `fmt`, `clippy`, `unit`, `integration`, `protocol-compat`, `security`, and `architecture` checks.
- Require signed release tags; do not require signed development commits until contributor impact is measured.
- Require code owner review for `schemas/`, `crates/sge-security/`, `crates/sge-sandbox/`, and `adapters/`.
- High-risk operator changes require one security review and fault-injection evidence.
- Do not allow generated host fixtures to update without visible golden diff review.

### Issue Contract

Every implementation issue must contain:

```markdown
## Outcome
Observable user or engineering result.

## Scope
Exact files/modules and behavior included.

## Exclusions
Explicitly deferred behavior.

## Acceptance
- [ ] Testable condition
- [ ] Failure/rollback condition

## Verification
Exact local commands and expected result.

## Risk
Low / Medium / High with rationale.
```

## 8. Cross-Phase Definition of Done

A task is complete only when:

- tests were written before implementation for behavior changes;
- exact verification commands pass;
- no unrelated formatting or generated files changed;
- user-facing behavior has CLI help and error text;
- schema changes include fixtures and migration behavior;
- side effects include cancellation, interruption, and rollback tests;
- security-sensitive paths include abuse-case tests;
- the final diff contains no credentials, temporary files, or debug output.

## 9. Risk Register

| ID | Risk | Likelihood | Impact | Mitigation | Record / Check |
|---|---|---:|---:|---|---|
| R1 | V1 breadth delays usable feedback | High | High | Enforce phase exit gates and vertical increments | milestone burndown + phase demos |
| R2 | “Improvement” cannot be reproduced | Medium | Critical | Contract, normalized environment, replay, deterministic graders | `sge test --replay` fixture |
| R3 | Host updates break generated output | High | High | version detection, capability matrix, golden fixtures | adapter compatibility CI |
| R4 | Model output performs unsafe mutation | Medium | Critical | untrusted-output validation, scoped worktree, deny-by-default policy | security abuse suite |
| R5 | Internal Git state corrupts | Low | High | transactional refs, fsck, source export, recovery fixtures | store fault-injection suite |
| R6 | Memory records become false authority | Medium | High | source, confidence, status, scope, expiry, confirmation | protocol validation + CLI approval |
| R7 | High-risk operator escapes sandbox | Low | Critical | container-only policy, no silent downgrade, output validation | containment tests |
| R8 | Remote model leaks sensitive content | Medium | Critical | data manifest, exclusion rules, redaction, explicit network policy | canary secret tests |
| R9 | Five adapters diverge semantically | High | High | shared capability contract and conformance suite | adapter conformance job |
| R10 | SQLite becomes accidental source of truth | Medium | Medium | rebuild command and destructive projection tests | delete/rebuild integration test |

## 10. Specification Traceability

| Specification Surface | Implementation Plan |
|---|---|
| Product forms and CLI | P0, P1, P3 |
| Artifact model and workspace | P0, P2 |
| CLI command system | P0, P1, P2, P3 |
| Directed and suggested evolution | P1, P2 |
| Evolution state machine | P1, P2 |
| Fourteen mutation operators | P2 |
| Evaluation and stopping | P1, P2 |
| Memory governance | P2 |
| Lineage and evidence | P0, P1 |
| Five host adapters | P3 |
| Model privacy and Data Manifest | P1, P4 |
| Four sandbox backends | P4 |
| Engineering architecture | P0 and this master plan |
| Test and quality gates | Every phase |
| V1 release strategy | P5 |
| Brand and terminal presentation | P5 docs and CLI snapshots |
| Launch demo and community assets | P5 |
| Open source and license | P5 |
| Risk controls | P4, P5, and this master plan |

No V1 specification section is intentionally deferred beyond P5. V1.1, V1.5, V2, and V3 remain outside this implementation plan.

## 11. Phase Execution Protocol

For each phase:

1. Create a branch named `phase/pN-short-name`.
2. Create milestone issues from the corresponding phase plan.
3. Execute tasks in listed order using TDD.
4. Run the phase local quality command.
5. Produce a phase evidence note under `docs/evidence/pN/summary.md`.
6. Review against this master plan and the source specification.
7. Merge only after the phase exit gate passes.

## 12. Full V1 Completion Gate

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features
cargo test -p sge-protocol --test compatibility
cargo test -p sge-eval --test replay
cargo test -p sge-sandbox --test containment
cargo test -p sge-adapter --test conformance
cargo deny check
cargo audit
npm test --prefix packages/npm
```

Expected:

- all commands exit `0`;
- no unsupported schema migration;
- no host adapter writes outside its transaction root;
- no canary secret appears in logs or evidence;
- every Stable operator passes its golden and regression suites;
- every Preview operator rejects automatic apply;
- all five hosts pass detect/render/validate/apply/rollback fixtures;
- all public demos replay from a clean machine.
