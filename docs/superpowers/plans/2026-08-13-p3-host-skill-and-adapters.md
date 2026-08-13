# P3 Host Skill and Adapters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make one standard Singularity Skill invoke `sge` from five AI coding hosts and transactionally render, validate, apply, link, and roll back Agent, Skill, and Rule assets.

**Architecture:** Define one host capability and transaction contract in `sge-adapter`. Keep each host implementation in an independent adapter crate/directory with golden fixtures. The portable Skill targets only the stable CLI JSON protocol and never imports a concrete adapter.

**Tech Stack:** Rust, serde_json, tempfile, insta golden tests, filesystem transactions, Markdown Skill source.

---

## File Map

```text
crates/sge-adapter/                 adapter traits and transaction engine
adapters/{claude,codex,trae,opencode,openclaw}/
fixtures/hosts/<host>/<version>/    detect input and golden output trees
skill/SKILL.md                      portable source Skill
skill/references/cli-contract.md
skill/templates/                    host-neutral prompts and result summaries
crates/sge-cli/tests/host_commands.rs
```

### Task 1: Define adapter capability and transaction contracts

**Files:**
- Create: `crates/sge-adapter/src/{lib,capability,detect,render,transaction,error}.rs`
- Create: `crates/sge-adapter/tests/contract.rs`

- [ ] Write tests for `native`, `mapped`, and `unsupported` capability results.
- [ ] Write transaction tests for prepare, validate, backup, commit, smoke test, and rollback.
- [ ] Run `cargo test -p sge-adapter`; expect failure.
- [ ] Implement:

```rust
pub trait HostAdapter {
    fn id(&self) -> HostId;
    fn detect(&self, root: &Path) -> Result<DetectedHost>;
    fn capabilities(&self, version: &HostVersion) -> CapabilityMatrix;
    fn render(&self, request: RenderRequest<'_>) -> Result<RenderedTree>;
    fn validate(&self, tree: &RenderedTree) -> Result<ValidationReport>;
    fn smoke_test(&self, installation: &Installation) -> Result<SmokeReport>;
}
```

- [ ] Ensure adapters cannot write directly; only the shared transaction engine writes.
- [ ] Run tests; expect PASS.
- [ ] Commit with `feat: define transactional host adapters`.

### Task 2: Build the adapter conformance harness

**Files:**
- Create: `crates/sge-adapter/src/conformance.rs`
- Create: `crates/sge-adapter/tests/conformance.rs`
- Create: `fixtures/hosts/common/standard-artifacts/`

- [ ] Write a harness that tests detect, capability declaration, deterministic render, path containment, conflict reporting, apply, injected failure, rollback, and idempotent reapply.
- [ ] Run the test; expect failure without adapters.
- [ ] Implement reusable conformance functions accepting a boxed `HostAdapter`.
- [ ] Run the empty harness unit tests; expect PASS.
- [ ] Commit with `test: add host adapter conformance harness`.

### Task 3: Implement Claude Code adapter

**Files:**
- Create: `adapters/claude/Cargo.toml`
- Create: `adapters/claude/src/lib.rs`
- Create: `fixtures/hosts/claude/<supported-version>/*`
- Test: `adapters/claude/tests/conformance.rs`

- [ ] Capture supported native/mapped/unsupported semantics in fixtures and README.
- [ ] Add golden tests for Agent, Skill, Rule, permission changes, and conflicts.
- [ ] Run adapter tests; expect failure.
- [ ] Implement detect/render/validate/smoke behavior using only documented host surfaces.
- [ ] Run conformance; expect PASS.
- [ ] Commit with `feat: add Claude Code adapter`.

### Task 4: Implement Codex adapter

**Files:**
- Create: `adapters/codex/Cargo.toml`
- Create: `adapters/codex/src/lib.rs`
- Create: `adapters/codex/tests/conformance.rs`
- Create: `fixtures/hosts/codex/<supported-version>/*`

- [ ] Write failing fixtures for version detection, Agent/Skill/Rule rendering, unsupported semantics, conflicting destination files, and rollback after an injected write failure.
- [ ] Run `cargo test -p sge-adapter-codex`; expect failure because the adapter is absent.
- [ ] Implement `HostAdapter` for Codex using Codex-specific paths and formats; report every mapped field in `SemanticLossReport`.
- [ ] Run `cargo test -p sge-adapter-codex`; expect all golden and conformance tests to pass.
- [ ] Commit with `feat: add Codex adapter`.

### Task 5: Implement TRAE adapter

**Files:**
- Create: `adapters/trae/Cargo.toml`
- Create: `adapters/trae/src/lib.rs`
- Create: `adapters/trae/tests/conformance.rs`
- Create: `fixtures/hosts/trae/<supported-version>/*`

- [ ] Write failing fixtures for TRAE version detection, destination paths, Skill/Agent/Rule mapping, conflicts, semantic loss, and transaction rollback.
- [ ] Run `cargo test -p sge-adapter-trae`; expect failure because the adapter is absent.
- [ ] Implement `HostAdapter` for TRAE without embedding local credentials or absolute workspace paths.
- [ ] Run `cargo test -p sge-adapter-trae`; expect all golden and conformance tests to pass.
- [ ] Commit with `feat: add TRAE adapter`.

### Task 6: Implement OpenCode adapter

**Files:**
- Create: `adapters/opencode/Cargo.toml`
- Create: `adapters/opencode/src/lib.rs`
- Create: `adapters/opencode/tests/conformance.rs`
- Create: `fixtures/hosts/opencode/<supported-version>/*`

- [ ] Write failing fixtures for OpenCode detection, rendering, conflict reporting, unsupported fields, idempotent reapply, and rollback.
- [ ] Run `cargo test -p sge-adapter-opencode`; expect failure because the adapter is absent.
- [ ] Implement `HostAdapter` for OpenCode and produce deterministic files from the standard artifact model.
- [ ] Run `cargo test -p sge-adapter-opencode`; expect all golden and conformance tests to pass.
- [ ] Commit with `feat: add OpenCode adapter`.

### Task 7: Implement OpenClaw adapter

**Files:**
- Create: `adapters/openclaw/Cargo.toml`
- Create: `adapters/openclaw/src/lib.rs`
- Create: `adapters/openclaw/tests/conformance.rs`
- Create: `fixtures/hosts/openclaw/<supported-version>/*`

- [ ] Write failing fixtures for OpenClaw detection, rendering, conflict reporting, unsupported fields, idempotent reapply, and rollback.
- [ ] Run `cargo test -p sge-adapter-openclaw`; expect failure because the adapter is absent.
- [ ] Implement `HostAdapter` for OpenClaw and preserve explicit warnings for every standard concept that lacks a native representation.
- [ ] Run `cargo test -p sge-adapter-openclaw`; expect all golden and conformance tests to pass.
- [ ] Commit with `feat: add OpenClaw adapter`.

### Task 8: Implement host detection and CLI commands

**Files:**
- Create: `crates/sge-app/src/hosts.rs`
- Modify: `crates/sge-cli/src/main.rs`
- Create: `crates/sge-cli/tests/host_commands.rs`

- [ ] Write tests for zero, one, and multiple detected hosts.
- [ ] Require `--to` when multiple hosts are present; allow `--to current` only with unambiguous host context.
- [ ] Implement:

```bash
sge hosts
sge apply <target> --to <host>
sge export <target> --to <host> --output <path>
sge link <target> --to <host>
sge undo --install <install-id>
```

- [ ] Add `--json` responses with capability matrix, semantic loss, paths, conflicts, and rollback command.
- [ ] Run CLI tests; expect PASS.
- [ ] Commit with `feat: expose host installation commands`.

### Task 9: Author the portable Singularity Skill

**Files:**
- Create: `skill/SKILL.md`
- Create: `skill/references/cli-contract.md`
- Create: `skill/templates/{scan,evolve,apply,memory}.md`
- Create: `skill/tests/intents.yaml`

- [ ] Write intent fixtures for scan, directed evolution, explain, apply, propose memory, and undo.
- [ ] Define the Skill rule: natural language may propose actions, but only explicit CLI approval fields authorize them.
- [ ] Document exact JSON request/response shapes used by the Skill.
- [ ] Add a test runner that validates generated CLI arguments against an allowlist and rejects shell metacharacters or free-form commands.
- [ ] Run `cargo test -p sge-adapter --test skill_contract`; expect PASS.
- [ ] Commit with `feat: add portable Singularity Skill`.

### Task 10: Generate host-specific Skill packages

**Files:**
- Create: `crates/sge-adapter/src/skill_package.rs`
- Create: `crates/sge-adapter/tests/skill_packages.rs`
- Create: `fixtures/hosts/<host>/skill-package/`

- [ ] Write golden tests that transform one standard `skill/` source into five host packages.
- [ ] Verify generated packages contain no host credentials, absolute local paths, or duplicated core logic.
- [ ] Implement generation through each host adapter.
- [ ] Run snapshots; inspect every diff; expect PASS.
- [ ] Commit with `feat: package the Skill for five hosts`.

### Task 11: Run P3 compatibility gate

**Files:**
- Create: `docs/compatibility/hosts.md`
- Create: `docs/evidence/p3/summary.md`
- Modify: `.github/workflows/ci.yml`

- [ ] Add a five-host matrix job running the shared conformance suite.
- [ ] Generate the compatibility table from adapter descriptors, not hand-maintained prose.
- [ ] Run:

```bash
cargo test -p sge-adapter --test conformance
cargo test -p sge-adapter-claude
cargo test -p sge-adapter-codex
cargo test -p sge-adapter-trae
cargo test -p sge-adapter-opencode
cargo test -p sge-adapter-openclaw
cargo test -p sge-adapter --test skill_contract
```

- [ ] Record transaction and rollback tree hashes in P3 evidence.
- [ ] Commit with `test: prove five-host compatibility`.

## P3 Exit Gate

- All five adapters pass one shared conformance suite.
- Unsupported semantics are rejected, never silently dropped.
- Installation is prepared and previewed before write.
- Injected failure restores the exact original host tree.
- Portable Skill emits only schema-valid allowlisted CLI requests.
- Generated Skill packages contain no copied evolution implementation.
