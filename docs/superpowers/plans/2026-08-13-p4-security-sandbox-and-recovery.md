# P4 Security, Sandbox, and Recovery Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove that untrusted model output, candidate code, provider failures, process interruption, and host write failures cannot escape declared boundaries or corrupt user state.

**Architecture:** Centralize path, network, secret, and approval policy in `sge-security`; expose four explicit sandbox backends with no silent downgrade; make every side-effecting workflow journaled and resumable or abortable; verify recovery through fault injection.

**Tech Stack:** Rust, cap-std, wasmtime, tokio process controls, Docker/Podman CLI adapters, secrecy, keyring, proptest, fault-injection test harness.

---

## File Map

```text
crates/sge-security/                path/network/secret/approval policy
crates/sge-sandbox/                 content/WASI/process/container backends
crates/sge-provider/src/redaction.rs
crates/sge-store/src/fault.rs       injectable failure points
fixtures/security/                  traversal, injection, secret canaries
fixtures/sandbox/                   backend containment workloads
docs/security/threat-model.md
docs/security/sandbox-guarantees.md
docs/evidence/p4/summary.md
```

### Task 1: Threat model and trust-boundary tests

- [ ] Create `docs/security/threat-model.md` covering user files, model input/output, provider transport, generated patches, candidate execution, internal Git, host adapters, and credentials.
- [ ] Add abuse cases for prompt injection, path traversal, command injection, symlink escape, secret exfiltration, malicious test command, adapter overwrite, replay tampering, and dependency substitution.
- [ ] Convert each high/critical abuse case into a named test fixture under `fixtures/security/`.
- [ ] Commit with `docs: define SGE threat model`.

### Task 2: Implement path and write policy

**Files:**
- Create: `crates/sge-security/src/{lib,path_policy,write_policy}.rs`
- Create: `crates/sge-security/tests/path_policy.rs`

- [ ] Write tests for `..`, absolute paths, symlink escape, case-fold collisions, Unicode normalization collisions, device files, and writes outside candidate worktrees.
- [ ] Run the focused test; expect failure.
- [ ] Implement canonical-root checks that validate every path component and re-check after file creation.
- [ ] Use open-relative-to-directory operations where supported; do not rely only on string prefix checks.
- [ ] Run tests; expect PASS.
- [ ] Commit with `feat: enforce filesystem containment`.

### Task 3: Implement secret sourcing and redaction

**Files:**
- Create: `crates/sge-security/src/{secret,redaction}.rs`
- Modify: `crates/sge-provider/src/redaction.rs`
- Create: `crates/sge-security/tests/secret_redaction.rs`

- [ ] Write tests with canary tokens in environment variables, keychain mock values, input files, model responses, errors, tracing fields, and evidence.
- [ ] Run tests; expect failure.
- [ ] Implement `SecretSource` for environment, OS keychain, and external credential command.
- [ ] Wrap values in secrecy types and redact exact and encoded forms before logging or persistence.
- [ ] Run tests and recursively scan test evidence for canaries; expect zero matches.
- [ ] Commit with `feat: protect provider credentials and evidence`.

### Task 4: Implement Data Manifest and network policy

**Files:**
- Create: `crates/sge-security/src/{data_manifest,network_policy}.rs`
- Create: `crates/sge-security/tests/data_manifest.rs`
- Modify: `crates/sge-provider/src/model.rs`

- [ ] Write tests for default exclusions: `.env`, certificates, private keys, Git credentials, binary files, and user-configured patterns.
- [ ] Test `deny`, `local`, and `remote` network modes.
- [ ] Implement a manifest containing normalized paths, content classifications, byte estimates, redactions, and destination provider.
- [ ] Require explicit approval when remote mode includes confirmed memory or files outside target scope.
- [ ] Run tests; expect PASS.
- [ ] Commit with `feat: gate model data exposure`.

### Task 5: Implement Content and WASI backends

**Files:**
- Create: `crates/sge-sandbox/src/{lib,backend,content,wasi,limits}.rs`
- Create: `crates/sge-sandbox/tests/{content,wasi_containment}.rs`

- [ ] Write Content tests proving no executable operation is accepted.
- [ ] Write WASI tests for filesystem escape, denied network, wall timeout, fuel exhaustion, memory limit, and trapped guest.
- [ ] Run tests; expect failure.
- [ ] Implement the backend trait:

```rust
#[async_trait::async_trait]
pub trait SandboxBackend {
    fn capability(&self) -> SandboxCapability;
    async fn execute(&self, request: ExecutionRequest) -> Result<ExecutionResult>;
}
```

- [ ] Record normalized stdout/stderr, exit status, resource use, and limit reason.
- [ ] Run containment tests; expect PASS.
- [ ] Commit with `feat: add Content and WASI sandboxes`.

### Task 6: Implement restricted Process backend

**Files:**
- Create: `crates/sge-sandbox/src/process.rs`
- Create: `crates/sge-sandbox/tests/process_containment.rs`

- [ ] Write tests for cwd isolation, environment allowlist, timeout with process-tree termination, output cap, executable allowlist, and cancellation.
- [ ] Run tests; expect failure.
- [ ] Implement execution only inside a candidate worktree using explicit argv arrays, never shell interpolation.
- [ ] Strip environment by default and add only declared variables.
- [ ] Kill the full child process group on timeout/cancel.
- [ ] Run tests; expect PASS.
- [ ] Commit with `feat: add restricted process sandbox`.

### Task 7: Implement optional Container backend

**Files:**
- Create: `crates/sge-sandbox/src/container.rs`
- Create: `crates/sge-sandbox/tests/container_contract.rs`

- [ ] Write adapter tests against a fake Docker/Podman command transport.
- [ ] Require pinned image digest, read-only root filesystem, no network, dropped capabilities, resource limits, and mounted candidate directory only.
- [ ] Implement runtime detection without pulling images automatically.
- [ ] Return `BackendUnavailable` when no supported runtime exists; never downgrade a high-risk run.
- [ ] Run tests; expect PASS.
- [ ] Commit with `feat: add guarded container sandbox`.

### Task 8: Enforce operator-to-sandbox policy

**Files:**
- Modify: `crates/sge-evolution/src/policy.rs`
- Create: `crates/sge-evolution/tests/sandbox_policy.rs`

- [ ] Add a table test for all fourteen operators and allowed backends.
- [ ] Prove high-risk operators reject Content, WASI when native project execution is required, and unavailable Container policy.
- [ ] Implement preflight checks before model calls and before candidate mutation.
- [ ] Run tests; expect PASS.
- [ ] Commit with `feat: enforce mutation sandbox requirements`.

### Task 9: Add fault injection and recovery matrix

**Files:**
- Create: `crates/sge-store/src/fault.rs`
- Create: `crates/sge-app/tests/recovery_matrix.rs`
- Create: `fixtures/recovery/*.json`

- [ ] Define failure points after prepare, snapshot, provider response, each candidate write, each evaluation, review selection, host backup, host write, and smoke test.
- [ ] Write a table-driven test that interrupts at every point and asserts source tree, internal refs, journal state, and host tree invariants.
- [ ] Implement resumable/abortable classification and cleanup.
- [ ] Verify no state is misreported as completed.
- [ ] Run `cargo test -p sge-app --test recovery_matrix`; expect PASS.
- [ ] Commit with `test: prove interruption recovery`.

### Task 10: Fuzz untrusted boundaries

**Files:**
- Create: `fuzz/Cargo.toml`
- Create: `fuzz/fuzz_targets/{protocol,patch,path,adapter_output}.rs`

- [ ] Add fuzz targets for YAML/JSON protocol parsing, structured patch validation, path resolution, and rendered host trees.
- [ ] Seed corpora from valid and malicious fixtures.
- [ ] Run each target for at least 60 seconds locally and record crashes as fixtures.
- [ ] Add a bounded nightly CI fuzz job.
- [ ] Commit with `test: fuzz untrusted input boundaries`.

### Task 11: Run the P4 security gate

- [ ] Run:

```bash
cargo test -p sge-security
cargo test -p sge-sandbox
cargo test -p sge-evolution --test sandbox_policy
cargo test -p sge-app --test recovery_matrix
cargo deny check
cargo audit
rg -n 'SGE_CANARY_SECRET' target test-results .singularity || true
```

- [ ] Expected: all tests pass and the canary search returns no persisted secret.
- [ ] Document exact guarantees and non-guarantees in `docs/security/sandbox-guarantees.md`.
- [ ] Record residual risks and results in `docs/evidence/p4/summary.md`.
- [ ] Commit with `security: complete P4 hardening gates`.

## P4 Exit Gate

- No backend silently grants more capability than requested.
- High-risk operators cannot run without the required backend.
- Model input has a reviewable Data Manifest.
- Canary secrets never persist in logs, evidence, or errors.
- Every side-effect boundary has an interruption recovery test.
- Host and source trees are byte-identical after injected rollback.
- Threat model residual risks are explicit.

