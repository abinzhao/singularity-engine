# P0 Protocol Foundation Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Establish the compilable Rust workspace, five versioned V1 protocols, workspace validation, and recoverable internal Git storage that every later phase depends on.

**Architecture:** Keep serialized documents in `sge-protocol`, behavior-free domain identifiers in `sge-domain`, filesystem/Git persistence behind `sge-store` traits, and use `sge-app` to orchestrate initialization. The CLI is a thin adapter and must not contain protocol or storage logic.

**Tech Stack:** Rust stable, Cargo workspace, clap, serde, serde_yaml, schemars, thiserror, git2, tempfile, assert_cmd, insta, proptest.

---

## File Map

```text
Cargo.toml                         workspace members and shared dependencies
rust-toolchain.toml               pinned stable toolchain and components
crates/sge-domain/                stable IDs, paths, and domain errors
crates/sge-protocol/              Artifact/Contract/Evidence/Memory/Adapter schemas
crates/sge-store/                 internal Git repository and journal
crates/sge-app/                   init and workspace validation use cases
crates/sge-cli/                   `sge init`, `status`, `doctor`
schemas/v1/                       generated JSON Schemas
fixtures/protocol/v1/             valid and invalid protocol documents
xtask/                            schema generation and architecture checks
```

### Task 1: Bootstrap the Cargo workspace

**Files:**
- Create: `Cargo.toml`
- Create: `rust-toolchain.toml`
- Create: `crates/sge-cli/Cargo.toml`
- Create: `crates/sge-cli/src/main.rs`
- Create: `crates/sge-domain/Cargo.toml`
- Create: `crates/sge-domain/src/lib.rs`

- [ ] **Step 1: Add a failing CLI smoke test**

Create `crates/sge-cli/tests/help.rs`:

```rust
use assert_cmd::Command;

#[test]
fn help_exposes_product_name() {
    Command::cargo_bin("sge")
        .unwrap()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicates::str::contains("SINGULARITY ENGINE"));
}
```

- [ ] **Step 2: Verify the test fails**

Run: `cargo test -p sge-cli --test help`

Expected: FAIL because the workspace and binary do not exist.

- [ ] **Step 3: Create the minimal workspace and CLI**

Use resolver `2`, edition `2024`, and workspace dependencies for `clap`, `serde`, `thiserror`, `assert_cmd`, and `predicates`. Implement:

```rust
use clap::Parser;

#[derive(Parser)]
#[command(name = "sge", about = "SINGULARITY ENGINE")]
struct Cli {}

fn main() {
    Cli::parse();
}
```

- [ ] **Step 4: Verify the smoke test passes**

Run: `cargo test -p sge-cli --test help`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add Cargo.toml rust-toolchain.toml crates/sge-cli crates/sge-domain
git commit -m "chore: bootstrap Rust workspace"
```

### Task 2: Define canonical artifact and target identifiers

**Files:**
- Create: `crates/sge-domain/src/artifact.rs`
- Create: `crates/sge-domain/src/target.rs`
- Modify: `crates/sge-domain/src/lib.rs`
- Test: `crates/sge-domain/tests/target_parse.rs`

- [ ] **Step 1: Write target parsing tests**

```rust
use sge_domain::{ArtifactKind, TargetRef};

#[test]
fn parses_skill_target() {
    let target: TargetRef = "skill:code-review".parse().unwrap();
    assert_eq!(target.kind(), ArtifactKind::Skill);
    assert_eq!(target.name(), "code-review");
}

#[test]
fn rejects_path_like_target_names() {
    assert!("skill:../secret".parse::<TargetRef>().is_err());
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p sge-domain --test target_parse`

Expected: FAIL with unresolved imports.

- [ ] **Step 3: Implement validated identifiers**

Implement `ArtifactKind::{Agent, Skill, Rule}`, `ArtifactName`, and `TargetRef`. Accept lowercase ASCII letters, digits, and single hyphens; reject empty names, path separators, `..`, and names over 64 bytes.

- [ ] **Step 4: Verify**

Run: `cargo test -p sge-domain`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add crates/sge-domain
git commit -m "feat: define canonical artifact targets"
```

### Task 3: Implement the five V1 protocol documents

**Files:**
- Create: `crates/sge-protocol/src/{lib,artifact,contract,evidence,memory,adapter,version}.rs`
- Create: `crates/sge-protocol/tests/fixtures.rs`
- Create: `fixtures/protocol/v1/{artifact,contract,evidence,memory,adapter}.yaml`
- Create: `fixtures/protocol/v1/invalid-unknown-version.yaml`

- [ ] **Step 1: Write fixture round-trip tests**

```rust
use sge_protocol::{Document, parse_document};

#[test]
fn v1_fixtures_round_trip_without_semantic_change() {
    for path in glob::glob("../../fixtures/protocol/v1/*.yaml").unwrap().flatten() {
        if path.file_name().unwrap().to_string_lossy().starts_with("invalid-") {
            continue;
        }
        let source = std::fs::read_to_string(&path).unwrap();
        let document = parse_document(&source).unwrap();
        let encoded = serde_yaml::to_string(&document).unwrap();
        let reparsed: Document = parse_document(&encoded).unwrap();
        assert_eq!(document, reparsed);
    }
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p sge-protocol --test fixtures`

Expected: FAIL because protocol types are missing.

- [ ] **Step 3: Implement tagged protocol types**

Use an explicit `schema` field with exact values:

```rust
pub const ARTIFACT_V1: &str = "sge.dev/artifact/v1";
pub const CONTRACT_V1: &str = "sge.dev/contract/v1";
pub const EVIDENCE_V1: &str = "sge.dev/evidence/v1";
pub const MEMORY_V1: &str = "sge.dev/memory/v1";
pub const ADAPTER_V1: &str = "sge.dev/adapter/v1";
```

Represent unknown extension fields with `#[serde(flatten)] BTreeMap<String, serde_yaml::Value>` so V1 round-trips future additive fields.

- [ ] **Step 4: Add strict version rejection**

Test that `sge.dev/artifact/v2` returns `ProtocolError::UnsupportedSchema` and never falls back to V1.

- [ ] **Step 5: Verify**

Run: `cargo test -p sge-protocol`

Expected: PASS for valid fixtures and explicit rejection for unsupported versions.

- [ ] **Step 6: Commit**

```bash
git add crates/sge-protocol fixtures/protocol
git commit -m "feat: define versioned V1 protocols"
```

### Task 4: Generate and check JSON Schemas

**Files:**
- Create: `xtask/Cargo.toml`
- Create: `xtask/src/main.rs`
- Create: `schemas/v1/*.schema.json`
- Modify: `Cargo.toml`

- [ ] **Step 1: Add schema drift test**

Create `crates/sge-protocol/tests/schema_drift.rs` that generates each schema to memory and compares it with `schemas/v1/<name>.schema.json`.

- [ ] **Step 2: Verify failure**

Run: `cargo test -p sge-protocol --test schema_drift`

Expected: FAIL because checked-in schemas are absent.

- [ ] **Step 3: Implement `cargo xtask schema`**

The command must generate files deterministically with sorted object keys and a trailing newline.

- [ ] **Step 4: Generate and verify**

Run:

```bash
cargo xtask schema
cargo test -p sge-protocol --test schema_drift
```

Expected: generated schemas are stable and the test passes.

- [ ] **Step 5: Commit**

```bash
git add xtask schemas Cargo.toml crates/sge-protocol/tests/schema_drift.rs
git commit -m "feat: generate protocol JSON schemas"
```

### Task 5: Create workspace initialization and validation

**Files:**
- Create: `crates/sge-app/src/{lib,init,validate}.rs`
- Create: `crates/sge-app/tests/init_workspace.rs`
- Modify: `crates/sge-cli/src/main.rs`

- [ ] **Step 1: Write the failing initialization test**

```rust
#[test]
fn init_creates_only_declared_workspace_files() {
    let dir = tempfile::tempdir().unwrap();
    sge_app::init::initialize(dir.path()).unwrap();
    assert!(dir.path().join("singularity.yaml").is_file());
    assert!(dir.path().join(".singularity/repo.git").is_dir());
    assert!(!dir.path().join(".git").exists());
}
```

- [ ] **Step 2: Verify failure**

Run: `cargo test -p sge-app --test init_workspace`

Expected: FAIL because `initialize` is missing.

- [ ] **Step 3: Implement idempotent initialization**

Create `agent`, `skills`, `rules`, `memory/{facts,preferences,failures}`, `evals/{datasets,graders,suites}`, and `.singularity/{worktrees,runs,cache,installs}`. Refuse to overwrite existing non-generated files. Initialize `.singularity/repo.git` as a bare repository.

- [ ] **Step 4: Wire `sge init`**

Return structured errors with stable codes such as `SGE-PROTOCOL-001` and `SGE-STORE-001`. Add `--json` output for host Skill use.

- [ ] **Step 5: Verify**

Run:

```bash
cargo test -p sge-app
cargo test -p sge-cli
cargo run -p sge-cli -- init /tmp/sge-p0-smoke
```

Expected: tests pass; smoke command creates a valid workspace without a business `.git`.

- [ ] **Step 6: Commit**

```bash
git add crates/sge-app crates/sge-cli
git commit -m "feat: initialize validated local workspaces"
```

### Task 6: Implement internal Git repository and append-only journal

**Files:**
- Create: `crates/sge-store/src/{lib,repository,journal,recovery}.rs`
- Create: `crates/sge-store/tests/{repository,journal_recovery}.rs`

- [ ] **Step 1: Write interruption and recovery tests**

Test these states: `Prepared`, `Mutating`, `Evaluating`, `ReviewPending`, `Applying`, `Completed`, `Aborted`. A run ending in a non-terminal state must be classified as resumable or abortable, never as completed.

- [ ] **Step 2: Verify failure**

Run: `cargo test -p sge-store`

Expected: FAIL because store types are missing.

- [ ] **Step 3: Implement repository traits**

Define:

```rust
pub trait LineageRepository {
    fn snapshot(&self, tree: &Path, metadata: &CommitMetadata) -> Result<Revision>;
    fn checkout_candidate(&self, parent: &Revision, target: &Path) -> Result<()>;
    fn restore(&self, revision: &Revision, target: &Path) -> Result<()>;
    fn verify(&self) -> Result<VerificationReport>;
}
```

Write journal records as newline-delimited JSON with sequence numbers and `fsync` before advancing a side-effect boundary.

- [ ] **Step 4: Add corruption behavior**

`verify` must report corruption without mutating the repository. Recovery must refuse destructive repair and point to source export instructions.

- [ ] **Step 5: Verify**

Run: `cargo test -p sge-store`

Expected: PASS, including simulated process interruption fixtures.

- [ ] **Step 6: Commit**

```bash
git add crates/sge-store
git commit -m "feat: add recoverable lineage store"
```

### Task 7: Add architecture and P0 quality gates

**Files:**
- Modify: `xtask/src/main.rs`
- Create: `.github/workflows/ci.yml`
- Create: `docs/adr/0001-modular-monolith.md`
- Create: `docs/evidence/p0/summary.md`

- [ ] **Step 1: Implement dependency-direction check**

`cargo xtask architecture` must parse `cargo metadata` and reject `sge-domain → sge-store`, `sge-domain → sge-cli`, any core crate → concrete adapter, and `sge-protocol → sge-cli`.

- [ ] **Step 2: Add CI jobs**

Add separate jobs for `fmt`, `clippy`, `unit`, `protocol-compat`, and `architecture`. Pin action major versions and use minimal permissions:

```yaml
permissions:
  contents: read
```

- [ ] **Step 3: Record ADR and evidence**

Copy ADR-001 from the master plan into `docs/adr/0001-modular-monolith.md`. Record exact P0 commands and results in the evidence summary.

- [ ] **Step 4: Run the P0 exit gate**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask architecture
git diff --check
```

Expected: all commands exit `0`.

- [ ] **Step 5: Commit**

```bash
git add .github xtask docs/adr docs/evidence
git commit -m "ci: enforce P0 architecture gates"
```

## P0 Exit Gate

- All five V1 documents parse, validate, round-trip, and reject unsupported versions.
- Workspace initialization is idempotent and never creates or mutates business Git.
- Internal Git snapshot/restore and journal interruption tests pass.
- Architecture dependency rules are executable.
- P0 evidence contains fresh command output.

