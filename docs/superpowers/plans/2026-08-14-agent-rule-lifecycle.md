# Agent/Rule Lifecycle Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Extend the existing Skill lifecycle so Agent, Skill, and Rule artifacts share one capability-driven import, validation, revision diff, and explicit undo path.

**Architecture:** Add a static artifact capability registry to `sge-domain` and make `sge-app` resolve canonical directories, manifest names, default files, and mutable surfaces through that registry. Preserve the current staging, internal Git lineage, and whole-directory restoration behavior; keep scan, evolve, run-based apply, and full CLI history support outside this slice.

**Tech Stack:** Rust 1.97, serde, serde_json, serde_yaml, thiserror, git2 through `sge-store`, tempfile, assert-style integration tests, cargo-nextest.

**Source specification:** `docs/superpowers/specs/2026-08-14-agent-rule-lifecycle-design.md`

---

## 1. Execution Rules

- Work in a dedicated branch or worktree.
- Use one behavior-level test at a time.
- Do not write the next production behavior while the current focused test is RED.
- Never refactor while RED.
- Tests must use public APIs: `capabilities_for`, `import_artifact`, `diff_revisions`, and
  `undo_revision`.
- Do not modify `scan.rs`, `evolve.rs`, `apply.rs`, provider code, protocol schemas, or CLI command
  shape in this slice.
- Do not commit generated files or change `Cargo.lock`; this plan adds no dependency.

## 2. File Map

| File | Responsibility |
| --- | --- |
| `crates/sge-domain/src/capability.rs` | Static capability registry, layouts, surfaces, and local composition checks |
| `crates/sge-domain/src/lib.rs` | Export capability API |
| `crates/sge-domain/tests/capabilities.rs` | Public registry behavior for all three artifact kinds |
| `crates/sge-app/src/import.rs` | Generic manifest discovery, strict file declaration validation, staged import, snapshot |
| `crates/sge-app/src/undo.rs` | Capability-driven canonical target resolution for explicit undo |
| `crates/sge-app/src/lib.rs` | Stable import error variants and codes |
| `crates/sge-app/tests/asset_lifecycle.rs` | Agent/Rule vertical lifecycle acceptance tests |
| `fixtures/assets/agent/basic-agent/*` | Deterministic Agent fixture |
| `fixtures/assets/rule/basic-rule/*` | Deterministic Rule fixture |

## 3. Observable Behaviors

1. Registry maps each `ArtifactKind` to one canonical layout and mutable surface.
2. Agent import writes `agents/<name>` and snapshots `agent:<name>`.
3. Rule import writes `rules/<name>` and snapshots `rule:<name>`.
4. Cross-kind name reuse is allowed; same-kind duplicate import is refused.
5. Missing, ambiguous, mismatched, malformed, traversing, missing, and symlinked input fails before
   canonical workspace mutation.
6. Revision diff is asset-agnostic.
7. Explicit undo restores the complete Agent or Rule directory and creates a new revision.
8. Existing Skill lifecycle behavior remains unchanged.

### Task 1: Add the Artifact Capability Registry

**Files:**
- Create: `crates/sge-domain/src/capability.rs`
- Modify: `crates/sge-domain/src/lib.rs:1-5`
- Create: `crates/sge-domain/tests/capabilities.rs`

- [ ] **Step 1: Write the failing public registry test**

Create `crates/sge-domain/tests/capabilities.rs`:

```rust
use sge_domain::{
    ArtifactKind, EvaluationRequirements, MutableSurface, capabilities_for,
};

#[test]
fn every_artifact_kind_has_one_canonical_capability_descriptor() {
    let cases = [
        (
            ArtifactKind::Agent,
            "agents",
            "agent.yaml",
            &["prompt.md"][..],
            MutableSurface::Prompt,
        ),
        (
            ArtifactKind::Skill,
            "skills",
            "skill.yaml",
            &["instructions.md"][..],
            MutableSurface::SkillInstructions,
        ),
        (
            ArtifactKind::Rule,
            "rules",
            "rule.yaml",
            &["rules.md"][..],
            MutableSurface::Rules,
        ),
    ];

    for (kind, directory, manifest, default_files, surface) in cases {
        let capabilities = capabilities_for(kind);
        let layout = capabilities.layout();

        assert_eq!(capabilities.kind(), kind);
        assert_eq!(layout.workspace_directory, directory);
        assert_eq!(layout.manifest_filename, manifest);
        assert_eq!(layout.default_declared_files, default_files);
        assert_eq!(capabilities.mutable_surfaces(), &[surface]);
        assert_eq!(
            capabilities.evaluation_requirements(),
            EvaluationRequirements {
                primary_surface: surface,
                composition_validation: false,
            }
        );
    }
}
```

- [ ] **Step 2: Run the focused test and verify RED**

Run:

```bash
cargo test -p sge-domain --test capabilities
```

Expected: compilation fails because `capabilities_for`, `EvaluationRequirements`, and
`MutableSurface` do not exist.

- [ ] **Step 3: Implement the minimal capability module**

Create `crates/sge-domain/src/capability.rs`:

```rust
use thiserror::Error;

use crate::{ArtifactKind, TargetRef};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MutableSurface {
    Prompt,
    SkillInstructions,
    Rules,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArtifactLayout {
    pub workspace_directory: &'static str,
    pub manifest_filename: &'static str,
    pub default_declared_files: &'static [&'static str],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationRequirements {
    pub primary_surface: MutableSurface,
    pub composition_validation: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct WorkspaceView<'a> {
    pub target: &'a TargetRef,
    pub manifest_kind: ArtifactKind,
    pub manifest_name: &'a str,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CapabilityError {
    #[error("manifest kind `{manifest}` does not match target kind `{target}`")]
    KindMismatch {
        target: ArtifactKind,
        manifest: ArtifactKind,
    },
    #[error("manifest name `{manifest}` does not match target name `{target}`")]
    NameMismatch {
        target: String,
        manifest: String,
    },
}

pub trait ArtifactCapabilities: Sync {
    fn kind(&self) -> ArtifactKind;
    fn layout(&self) -> ArtifactLayout;
    fn mutable_surfaces(&self) -> &'static [MutableSurface];
    fn evaluation_requirements(&self) -> EvaluationRequirements;

    fn validate_composition(
        &self,
        workspace: &WorkspaceView<'_>,
    ) -> Result<(), CapabilityError> {
        if workspace.target.kind() != workspace.manifest_kind {
            return Err(CapabilityError::KindMismatch {
                target: workspace.target.kind(),
                manifest: workspace.manifest_kind,
            });
        }
        if workspace.target.name() != workspace.manifest_name {
            return Err(CapabilityError::NameMismatch {
                target: workspace.target.name().to_owned(),
                manifest: workspace.manifest_name.to_owned(),
            });
        }
        Ok(())
    }
}

#[derive(Debug)]
struct StaticCapabilities {
    kind: ArtifactKind,
    layout: ArtifactLayout,
    surfaces: &'static [MutableSurface],
    evaluation: EvaluationRequirements,
}

impl ArtifactCapabilities for StaticCapabilities {
    fn kind(&self) -> ArtifactKind {
        self.kind
    }

    fn layout(&self) -> ArtifactLayout {
        self.layout
    }

    fn mutable_surfaces(&self) -> &'static [MutableSurface] {
        self.surfaces
    }

    fn evaluation_requirements(&self) -> EvaluationRequirements {
        self.evaluation
    }
}

const AGENT_SURFACES: &[MutableSurface] = &[MutableSurface::Prompt];
const SKILL_SURFACES: &[MutableSurface] = &[MutableSurface::SkillInstructions];
const RULE_SURFACES: &[MutableSurface] = &[MutableSurface::Rules];

static AGENT: StaticCapabilities = StaticCapabilities {
    kind: ArtifactKind::Agent,
    layout: ArtifactLayout {
        workspace_directory: "agents",
        manifest_filename: "agent.yaml",
        default_declared_files: &["prompt.md"],
    },
    surfaces: AGENT_SURFACES,
    evaluation: EvaluationRequirements {
        primary_surface: MutableSurface::Prompt,
        composition_validation: false,
    },
};

static SKILL: StaticCapabilities = StaticCapabilities {
    kind: ArtifactKind::Skill,
    layout: ArtifactLayout {
        workspace_directory: "skills",
        manifest_filename: "skill.yaml",
        default_declared_files: &["instructions.md"],
    },
    surfaces: SKILL_SURFACES,
    evaluation: EvaluationRequirements {
        primary_surface: MutableSurface::SkillInstructions,
        composition_validation: false,
    },
};

static RULE: StaticCapabilities = StaticCapabilities {
    kind: ArtifactKind::Rule,
    layout: ArtifactLayout {
        workspace_directory: "rules",
        manifest_filename: "rule.yaml",
        default_declared_files: &["rules.md"],
    },
    surfaces: RULE_SURFACES,
    evaluation: EvaluationRequirements {
        primary_surface: MutableSurface::Rules,
        composition_validation: false,
    },
};

pub fn capabilities_for(kind: ArtifactKind) -> &'static dyn ArtifactCapabilities {
    match kind {
        ArtifactKind::Agent => &AGENT,
        ArtifactKind::Skill => &SKILL,
        ArtifactKind::Rule => &RULE,
    }
}
```

Modify `crates/sge-domain/src/lib.rs`:

```rust
pub mod artifact;
pub mod capability;
pub mod target;

pub use artifact::{ArtifactKind, ArtifactKindParseError, ArtifactName, ArtifactNameError};
pub use capability::{
    ArtifactCapabilities, ArtifactLayout, CapabilityError, EvaluationRequirements, MutableSurface,
    WorkspaceView, capabilities_for,
};
pub use target::{TargetRef, TargetRefParseError};

pub const PRODUCT_NAME: &str = "SINGULARITY ENGINE";
```

- [ ] **Step 4: Run registry and existing domain tests**

Run:

```bash
cargo test -p sge-domain --test capabilities
cargo test -p sge-domain
```

Expected: both commands pass.

- [ ] **Step 5: Commit the registry**

```bash
git add crates/sge-domain/src/capability.rs crates/sge-domain/src/lib.rs \
  crates/sge-domain/tests/capabilities.rs
git commit -m "feat: define artifact lifecycle capabilities"
```

### Task 2: Import an Agent Through the Generic Lifecycle

**Files:**
- Create: `fixtures/assets/agent/basic-agent/agent.yaml`
- Create: `fixtures/assets/agent/basic-agent/prompt.md`
- Create: `crates/sge-app/tests/asset_lifecycle.rs`
- Modify: `crates/sge-app/src/import.rs:1-180`
- Modify: `crates/sge-app/src/lib.rs:28-42,74-79`

- [ ] **Step 1: Add the Agent fixture**

Create `fixtures/assets/agent/basic-agent/agent.yaml`:

```yaml
schema: sge.dev/artifact/v1
id: code-review-agent-v1
kind: agent
name: code-review
title: Code Review Agent
summary: Coordinates deterministic code review.
body: see prompt.md
files:
  - path: prompt.md
    required: true
```

Create `fixtures/assets/agent/basic-agent/prompt.md`:

```markdown
# Code Review Agent

Inspect changed code, report concrete findings, and preserve user-authored work.
```

- [ ] **Step 2: Write the Agent import tracer-bullet test**

Create `crates/sge-app/tests/asset_lifecycle.rs`:

```rust
use std::{fs, path::PathBuf};

use sge_app::{import::import_artifact, init};

fn fixture(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative)
}

#[test]
fn agent_import_uses_the_canonical_layout_and_creates_a_revision() {
    let workspace = tempfile::tempdir().expect("create workspace");
    init::initialize(workspace.path()).expect("initialize workspace");

    let source = fixture("fixtures/assets/agent/basic-agent");
    let imported = import_artifact(workspace.path(), &source).expect("import Agent");
    let target = workspace.path().join("agents/code-review");

    assert_eq!(imported.target, "agent:code-review");
    assert!(!imported.revision.is_empty());
    assert_eq!(
        fs::read_to_string(target.join("prompt.md")).expect("read imported prompt"),
        fs::read_to_string(source.join("prompt.md")).expect("read fixture prompt")
    );
    assert!(target.join("agent.yaml").is_file());
    assert!(!workspace.path().join("skills/code-review").exists());
}
```

- [ ] **Step 3: Run the tracer test and verify RED**

Run:

```bash
cargo test -p sge-app --test asset_lifecycle \
  agent_import_uses_the_canonical_layout_and_creates_a_revision -- --exact
```

Expected: FAIL because `import_artifact` looks only for `skill.yaml`.

- [ ] **Step 4: Add stable resolver errors**

Replace `KindMismatch` in `crates/sge-app/src/lib.rs` and add the new variants:

```rust
#[error("artifact kind mismatch: expected {expected}, got {got}")]
KindMismatch { expected: String, got: String },
#[error("no recognized artifact manifest exists in {path}")]
MissingArtifactManifest { path: PathBuf },
#[error("multiple artifact manifests exist in {path}: {manifests:?}")]
AmbiguousArtifactManifest {
    path: PathBuf,
    manifests: Vec<String>,
},
#[error("artifact name `{name}` is invalid: {message}")]
InvalidArtifactName { name: String, message: String },
```

Extend `AppError::code`:

```rust
Self::KindMismatch { .. } => "SGE-IMPORT-003",
Self::MissingArtifactManifest { .. } => "SGE-IMPORT-007",
Self::AmbiguousArtifactManifest { .. } => "SGE-IMPORT-008",
Self::InvalidArtifactName { .. } => "SGE-IMPORT-009",
```

Keep all existing codes unchanged.

- [ ] **Step 5: Replace Skill-only manifest and target resolution**

Add these imports and helpers to `crates/sge-app/src/import.rs`:

```rust
use std::path::{Component, Path, PathBuf};

use serde_json::Value;
use sge_domain::{
    ArtifactKind, ArtifactName, TargetRef, WorkspaceView, capabilities_for,
};

const RECOGNIZED_MANIFESTS: &[&str] = &["agent.yaml", "skill.yaml", "rule.yaml"];

#[derive(Debug, Clone)]
struct DeclaredFile {
    path: String,
}

fn discover_manifest(source_dir: &Path) -> Result<PathBuf> {
    let manifests = RECOGNIZED_MANIFESTS
        .iter()
        .filter(|name| source_dir.join(name).is_file())
        .map(|name| (*name).to_owned())
        .collect::<Vec<_>>();

    match manifests.as_slice() {
        [] => Err(AppError::MissingArtifactManifest {
            path: source_dir.to_path_buf(),
        }),
        [manifest] => Ok(source_dir.join(manifest)),
        _ => Err(AppError::AmbiguousArtifactManifest {
            path: source_dir.to_path_buf(),
            manifests,
        }),
    }
}

fn parse_declared_files(
    files: Option<&Value>,
    defaults: &[&str],
) -> Vec<DeclaredFile> {
    match files {
        None => defaults
            .iter()
            .map(|path| DeclaredFile {
                path: (*path).to_owned(),
            })
            .collect(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| match item {
                Value::String(path) if !path.is_empty() => {
                    Some(DeclaredFile { path: path.clone() })
                }
                Value::Object(object) => object
                    .get("path")
                    .and_then(Value::as_str)
                    .filter(|path| !path.is_empty())
                    .map(|path| DeclaredFile {
                        path: path.to_owned(),
                    }),
                _ => None,
            })
            .collect(),
        Some(_) => defaults
            .iter()
            .map(|path| DeclaredFile {
                path: (*path).to_owned(),
            })
            .collect(),
    }
}

fn validate_relative_path(path: &str) -> bool {
    !path.is_empty()
        && Path::new(path).components().all(|component| {
            matches!(component, Component::Normal(_) | Component::CurDir)
        })
}
```

Replace the beginning of `import_artifact` through destination resolution with:

```rust
let manifest_path = discover_manifest(source_dir)?;
let manifest_content =
    fs::read_to_string(&manifest_path).map_err(|error| io_err(manifest_path.clone(), error))?;
let artifact = match parse_document(&manifest_content).map_err(|error| {
    AppError::InvalidArtifactDoc {
        path: manifest_path.clone(),
        source: Box::new(error),
    }
})? {
    Document::Artifact(artifact) => artifact,
    _ => {
        return Err(AppError::InvalidArtifactDoc {
            path: manifest_path.clone(),
            source: "document is not an ArtifactDocument".into(),
        });
    }
};

let kind = artifact
    .kind
    .parse::<ArtifactKind>()
    .map_err(|_| AppError::KindMismatch {
        expected: "agent, skill, or rule".to_owned(),
        got: artifact.kind.clone(),
    })?;
let capabilities = capabilities_for(kind);
let layout = capabilities.layout();
let discovered = manifest_path
    .file_name()
    .and_then(|name| name.to_str())
    .expect("recognized manifest names are valid UTF-8");
if discovered != layout.manifest_filename {
    return Err(AppError::KindMismatch {
        expected: layout.manifest_filename.to_owned(),
        got: discovered.to_owned(),
    });
}
let name = ArtifactName::new(artifact.name.clone()).map_err(|error| {
    AppError::InvalidArtifactName {
        name: artifact.name.clone(),
        message: error.to_string(),
    }
})?;
let target = TargetRef::new(kind, name);
capabilities
    .validate_composition(&WorkspaceView {
        target: &target,
        manifest_kind: kind,
        manifest_name: &artifact.name,
    })
    .map_err(|error| AppError::InvalidArtifactDoc {
        path: manifest_path.clone(),
        source: Box::new(error),
    })?;
let target_dir = workspace_root
    .join(layout.workspace_directory)
    .join(target.name());
```

Replace declared-file parsing with:

```rust
let declared_files = parse_declared_files(
    artifact.extensions.get("files"),
    layout.default_declared_files,
);
```

Before `symlink_metadata`, reject unsafe paths:

```rust
if !validate_relative_path(&declared.path) {
    return Err(AppError::PathTraversal {
        declared: declared.path.clone(),
    });
}
```

Use `layout.manifest_filename`, `target`, and the capability-derived destination for copy and
snapshot:

```rust
let manifest_dest = temp_dir.join(layout.manifest_filename);
fs::copy(&manifest_path, &manifest_dest)
    .map_err(|error| io_err(manifest_dest, error))?;

let metadata = serde_json::json!({
    "op": "import",
    "target": target.to_string(),
});

Ok(ImportedArtifact {
    target: target.to_string(),
    revision: revision.as_str().to_owned(),
    warnings: Vec::new(),
})
```

- [ ] **Step 6: Run the Agent test and Skill import regression**

Run:

```bash
cargo test -p sge-app --test asset_lifecycle \
  agent_import_uses_the_canonical_layout_and_creates_a_revision -- --exact
cargo test -p sge-app --test import_skill
```

Expected: Agent tracer passes; all existing Skill import tests pass.

- [ ] **Step 7: Commit the Agent tracer**

```bash
git add crates/sge-app/src/import.rs crates/sge-app/src/lib.rs \
  crates/sge-app/tests/asset_lifecycle.rs fixtures/assets/agent/basic-agent
git commit -m "feat: import Agent artifacts through capabilities"
```

### Task 3: Prove Rule Import and Cross-Kind Isolation

**Files:**
- Create: `fixtures/assets/rule/basic-rule/rule.yaml`
- Create: `fixtures/assets/rule/basic-rule/rules.md`
- Modify: `crates/sge-app/tests/asset_lifecycle.rs`

- [ ] **Step 1: Add the Rule fixture**

Create `fixtures/assets/rule/basic-rule/rule.yaml`:

```yaml
schema: sge.dev/artifact/v1
id: code-review-rule-v1
kind: rule
name: code-review
title: Code Review Rule
summary: Enforces evidence-backed code review.
body: see rules.md
files:
  - path: rules.md
    required: true
```

Create `fixtures/assets/rule/basic-rule/rules.md`:

```markdown
# Code Review Rule

Report only findings supported by the changed code and observable behavior.
```

- [ ] **Step 2: Add the Rule and cross-kind behavior test**

Append to `crates/sge-app/tests/asset_lifecycle.rs`:

```rust
#[test]
fn rule_import_is_isolated_from_an_agent_with_the_same_name() {
    let workspace = tempfile::tempdir().expect("create workspace");
    init::initialize(workspace.path()).expect("initialize workspace");

    let agent = import_artifact(
        workspace.path(),
        fixture("fixtures/assets/agent/basic-agent"),
    )
    .expect("import Agent");
    let rule = import_artifact(
        workspace.path(),
        fixture("fixtures/assets/rule/basic-rule"),
    )
    .expect("import Rule");

    assert_eq!(agent.target, "agent:code-review");
    assert_eq!(rule.target, "rule:code-review");
    assert_ne!(agent.revision, rule.revision);
    assert!(workspace.path().join("agents/code-review/agent.yaml").is_file());
    assert!(workspace.path().join("rules/code-review/rule.yaml").is_file());
}
```

- [ ] **Step 3: Run the new test**

Run:

```bash
cargo test -p sge-app --test asset_lifecycle \
  rule_import_is_isolated_from_an_agent_with_the_same_name -- --exact
```

Expected: PASS because Task 2 resolves Rule layout through the same capability interface used by
Agent import. A failure means Task 2 is incomplete; stop before adding Rule-specific production
branches.

- [ ] **Step 4: Add same-kind duplicate coverage**

Append:

```rust
#[test]
fn duplicate_rule_import_is_refused_without_changing_the_existing_rule() {
    let workspace = tempfile::tempdir().expect("create workspace");
    init::initialize(workspace.path()).expect("initialize workspace");
    let source = fixture("fixtures/assets/rule/basic-rule");

    import_artifact(workspace.path(), &source).expect("first Rule import");
    let before = fs::read_to_string(workspace.path().join("rules/code-review/rules.md"))
        .expect("read existing Rule");
    let error = import_artifact(workspace.path(), &source).expect_err("reject duplicate Rule");

    assert_eq!(error.code(), "SGE-IMPORT-002");
    assert_eq!(
        fs::read_to_string(workspace.path().join("rules/code-review/rules.md"))
            .expect("read unchanged Rule"),
        before
    );
}
```

- [ ] **Step 5: Run the complete lifecycle test file**

Run:

```bash
cargo test -p sge-app --test asset_lifecycle
```

Expected: all current Agent and Rule tests pass.

- [ ] **Step 6: Commit Rule coverage**

```bash
git add crates/sge-app/tests/asset_lifecycle.rs fixtures/assets/rule/basic-rule \
  crates/sge-app/src/import.rs
git commit -m "test: prove Rule import isolation"
```

### Task 4: Enforce Manifest and File Declaration Containment

**Files:**
- Modify: `crates/sge-app/tests/asset_lifecycle.rs`
- Modify: `crates/sge-app/src/import.rs`
- Modify: `crates/sge-app/src/lib.rs`

- [ ] **Step 1: Write the missing-manifest test**

Append:

```rust
#[test]
fn import_without_a_recognized_manifest_fails_before_workspace_mutation() {
    let workspace = tempfile::tempdir().expect("create workspace");
    let source = tempfile::tempdir().expect("create source");
    init::initialize(workspace.path()).expect("initialize workspace");
    fs::write(source.path().join("prompt.md"), "orphan prompt\n").expect("write prompt");

    let error = import_artifact(workspace.path(), source.path()).expect_err("reject source");

    assert_eq!(error.code(), "SGE-IMPORT-007");
    assert!(fs::read_dir(workspace.path().join("agents"))
        .expect("read agents")
        .next()
        .is_none());
}
```

- [ ] **Step 2: Run it and verify GREEN from manifest discovery**

Run:

```bash
cargo test -p sge-app --test asset_lifecycle \
  import_without_a_recognized_manifest_fails_before_workspace_mutation -- --exact
```

Expected: PASS because Task 2 routes zero-manifest discovery through `MissingArtifactManifest`.

- [ ] **Step 3: Write and run the ambiguous-manifest test**

Append:

```rust
#[test]
fn import_with_multiple_recognized_manifests_is_ambiguous() {
    let workspace = tempfile::tempdir().expect("create workspace");
    let source = tempfile::tempdir().expect("create source");
    init::initialize(workspace.path()).expect("initialize workspace");
    fs::write(source.path().join("agent.yaml"), "kind: agent\n").expect("write Agent manifest");
    fs::write(source.path().join("rule.yaml"), "kind: rule\n").expect("write Rule manifest");

    let error = import_artifact(workspace.path(), source.path()).expect_err("reject ambiguity");

    assert_eq!(error.code(), "SGE-IMPORT-008");
}
```

Run:

```bash
cargo test -p sge-app --test asset_lifecycle \
  import_with_multiple_recognized_manifests_is_ambiguous -- --exact
```

Expected: PASS.

- [ ] **Step 4: Write and run the filename/kind mismatch test**

Append:

```rust
#[test]
fn manifest_filename_must_match_the_declared_kind() {
    let workspace = tempfile::tempdir().expect("create workspace");
    let source = tempfile::tempdir().expect("create source");
    init::initialize(workspace.path()).expect("initialize workspace");
    fs::write(
        source.path().join("agent.yaml"),
        r#"schema: sge.dev/artifact/v1
id: mismatch-v1
kind: rule
name: mismatch
title: Mismatch
summary: Invalid manifest placement.
body: see rules.md
files:
  - rules.md
"#,
    )
    .expect("write manifest");
    fs::write(source.path().join("rules.md"), "rule\n").expect("write rules");

    let error = import_artifact(workspace.path(), source.path()).expect_err("reject mismatch");

    assert_eq!(error.code(), "SGE-IMPORT-003");
    assert!(!workspace.path().join("agents/mismatch").exists());
    assert!(!workspace.path().join("rules/mismatch").exists());
}
```

Run:

```bash
cargo test -p sge-app --test asset_lifecycle \
  manifest_filename_must_match_the_declared_kind -- --exact
```

Expected: PASS.

- [ ] **Step 5: Write the strict files-declaration test**

Append:

```rust
#[test]
fn an_explicit_empty_files_array_is_rejected() {
    let workspace = tempfile::tempdir().expect("create workspace");
    let source = tempfile::tempdir().expect("create source");
    init::initialize(workspace.path()).expect("initialize workspace");
    fs::write(
        source.path().join("rule.yaml"),
        r#"schema: sge.dev/artifact/v1
id: empty-files-v1
kind: rule
name: empty-files
title: Empty Files
summary: Must not import an incomplete artifact.
body: see rules.md
files: []
"#,
    )
    .expect("write manifest");

    let error = import_artifact(workspace.path(), source.path()).expect_err("reject empty files");

    assert_eq!(error.code(), "SGE-IMPORT-010");
    assert!(!workspace.path().join("rules/empty-files").exists());
}
```

- [ ] **Step 6: Run it and verify RED**

Run:

```bash
cargo test -p sge-app --test asset_lifecycle \
  an_explicit_empty_files_array_is_rejected -- --exact
```

Expected: FAIL because the minimal parser from Task 2 accepts an empty array and imports an
incomplete artifact.

- [ ] **Step 7: Implement strict file declaration parsing**

Add this error variant to `crates/sge-app/src/lib.rs`:

```rust
#[error("artifact file declaration at {path} is invalid: {message}")]
InvalidFileDeclaration { path: PathBuf, message: String },
```

Map it in `AppError::code`:

```rust
Self::InvalidFileDeclaration { .. } => "SGE-IMPORT-010",
```

Replace `DeclaredFile` in `crates/sge-app/src/import.rs` with:

```rust
#[derive(Debug, Clone)]
struct DeclaredFile {
    path: String,
    required: bool,
}
```

Replace `parse_declared_files` in `crates/sge-app/src/import.rs` with:

```rust
fn parse_declared_files(
    files: Option<&Value>,
    defaults: &[&str],
    manifest_path: &Path,
) -> Result<Vec<DeclaredFile>> {
    let Some(files) = files else {
        return Ok(defaults
            .iter()
            .map(|path| DeclaredFile {
                path: (*path).to_owned(),
                required: true,
            })
            .collect());
    };
    let Value::Array(items) = files else {
        return Err(AppError::InvalidFileDeclaration {
            path: manifest_path.to_path_buf(),
            message: "`files` must be a non-empty array".to_owned(),
        });
    };
    if items.is_empty() {
        return Err(AppError::InvalidFileDeclaration {
            path: manifest_path.to_path_buf(),
            message: "`files` must be a non-empty array".to_owned(),
        });
    }

    items
        .iter()
        .map(|item| {
            let (path, required) = match item {
                Value::String(path) if !path.is_empty() => (path.as_str(), true),
                Value::Object(object) => {
                    let path = object
                        .get("path")
                        .and_then(Value::as_str)
                        .filter(|path| !path.is_empty())
                        .ok_or_else(|| AppError::InvalidFileDeclaration {
                            path: manifest_path.to_path_buf(),
                            message:
                                "each file object requires a non-empty string `path`".to_owned(),
                        })?;
                    let required = match object.get("required") {
                        None => true,
                        Some(Value::Bool(required)) => *required,
                        Some(_) => {
                            return Err(AppError::InvalidFileDeclaration {
                                path: manifest_path.to_path_buf(),
                                message: "`required` must be a boolean".to_owned(),
                            });
                        }
                    };
                    (path, required)
                }
                _ => {
                    return Err(AppError::InvalidFileDeclaration {
                        path: manifest_path.to_path_buf(),
                        message:
                            "each file must be a non-empty path string or object".to_owned(),
                    });
                }
            };
            Ok(DeclaredFile {
                path: path.to_owned(),
                required,
            })
        })
        .collect()
}
```

Update the call site:

```rust
let declared_files = parse_declared_files(
    artifact.extensions.get("files"),
    layout.default_declared_files,
    &manifest_path,
)?;
```

Replace the declared-file validation loop with a loop that retains only present files:

```rust
let mut import_files = Vec::new();
for declared in &declared_files {
    if !validate_relative_path(&declared.path) {
        return Err(AppError::PathTraversal {
            declared: declared.path.clone(),
        });
    }
    let declared_path = source_dir.join(&declared.path);
    let sym_meta = match fs::symlink_metadata(&declared_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound && !declared.required => {
            continue;
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err(AppError::MissingDeclaredFile {
                declared: declared.path.clone(),
            });
        }
        Err(error) => return Err(io_err(declared_path.clone(), error)),
    };
    if sym_meta.file_type().is_symlink() {
        return Err(AppError::SymlinkRefused {
            declared: declared.path.clone(),
        });
    }
    let resolved = declared_path
        .canonicalize()
        .map_err(|error| io_err(declared_path.clone(), error))?;
    if !resolved.starts_with(&source_canonical) {
        return Err(AppError::PathTraversal {
            declared: declared.path.clone(),
        });
    }
    import_files.push(declared);
}
```

Change the staging copy loop to:

```rust
for declared in import_files {
    let src = source_dir.join(&declared.path);
    let dst = temp_dir.join(&declared.path);
    if let Some(parent) = dst.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|error| io_err(parent.to_path_buf(), error))?;
    }
    fs::copy(&src, &dst).map_err(|error| io_err(dst, error))?;
}
```

Run:

```bash
cargo test -p sge-app --test asset_lifecycle \
  an_explicit_empty_files_array_is_rejected -- --exact
```

Expected: PASS.

- [ ] **Step 8: Add invalid-name coverage**

Append:

```rust
#[test]
fn invalid_artifact_names_are_rejected_before_destination_creation() {
    let workspace = tempfile::tempdir().expect("create workspace");
    let source = tempfile::tempdir().expect("create source");
    init::initialize(workspace.path()).expect("initialize workspace");
    fs::write(
        source.path().join("agent.yaml"),
        r#"schema: sge.dev/artifact/v1
id: invalid-name-v1
kind: agent
name: Invalid_Name
title: Invalid Name
summary: Uses an invalid canonical name.
body: see prompt.md
files:
  - prompt.md
"#,
    )
    .expect("write manifest");
    fs::write(source.path().join("prompt.md"), "prompt\n").expect("write prompt");

    let error = import_artifact(workspace.path(), source.path()).expect_err("reject name");

    assert_eq!(error.code(), "SGE-IMPORT-009");
}
```

Run:

```bash
cargo test -p sge-app --test asset_lifecycle \
  invalid_artifact_names_are_rejected_before_destination_creation -- --exact
```

Expected: PASS.

- [ ] **Step 9: Prove capability defaults apply when `files` is absent**

Append:

```rust
#[test]
fn agent_without_files_uses_the_capability_default_prompt() {
    let workspace = tempfile::tempdir().expect("create workspace");
    let source = tempfile::tempdir().expect("create source");
    init::initialize(workspace.path()).expect("initialize workspace");
    fs::write(
        source.path().join("agent.yaml"),
        r#"schema: sge.dev/artifact/v1
id: default-files-v1
kind: agent
name: default-files
title: Default Files
summary: Uses the Agent capability default.
body: see prompt.md
"#,
    )
    .expect("write manifest");
    fs::write(source.path().join("prompt.md"), "default prompt\n").expect("write prompt");

    let imported = import_artifact(workspace.path(), source.path()).expect("import Agent");

    assert_eq!(imported.target, "agent:default-files");
    assert_eq!(
        fs::read_to_string(workspace.path().join("agents/default-files/prompt.md"))
            .expect("read imported prompt"),
        "default prompt\n"
    );
}
```

Run:

```bash
cargo test -p sge-app --test asset_lifecycle \
  agent_without_files_uses_the_capability_default_prompt -- --exact
```

Expected: PASS.

- [ ] **Step 10: Prove required declared files cannot be omitted**

Append:

```rust
#[test]
fn missing_required_rule_file_fails_before_destination_creation() {
    let workspace = tempfile::tempdir().expect("create workspace");
    let source = tempfile::tempdir().expect("create source");
    init::initialize(workspace.path()).expect("initialize workspace");
    fs::write(
        source.path().join("rule.yaml"),
        r#"schema: sge.dev/artifact/v1
id: missing-required-v1
kind: rule
name: missing-required
title: Missing Required File
summary: Declares a file that is absent.
body: see rules.md
files:
  - path: rules.md
    required: true
"#,
    )
    .expect("write manifest");

    let error = import_artifact(workspace.path(), source.path())
        .expect_err("reject missing required file");

    assert_eq!(error.code(), "SGE-IMPORT-005");
    assert!(!workspace.path().join("rules/missing-required").exists());
}
```

Run:

```bash
cargo test -p sge-app --test asset_lifecycle \
  missing_required_rule_file_fails_before_destination_creation -- --exact
```

Expected: PASS.

- [ ] **Step 11: Re-run import security regressions**

Run:

```bash
cargo test -p sge-app --test asset_lifecycle
cargo test -p sge-app --test import_skill
```

Expected: all tests pass, including the existing symlink and duplicate protections.

- [ ] **Step 12: Commit containment behavior**

```bash
git add crates/sge-app/src/import.rs crates/sge-app/src/lib.rs \
  crates/sge-app/tests/asset_lifecycle.rs
git commit -m "fix: contain multi-kind artifact imports"
```

### Task 5: Restore Agent and Rule Revisions Through Capabilities

**Files:**
- Modify: `crates/sge-app/src/undo.rs:6-8,78-106`
- Modify: `crates/sge-app/tests/asset_lifecycle.rs`

- [ ] **Step 1: Add a public-behavior helper for creating a changed revision**

Add these imports and helper to `crates/sge-app/tests/asset_lifecycle.rs`:

```rust
use sge_app::{history::diff_revisions, undo::undo_revision};
use sge_store::{GitLineageRepository, LineageRepository};

fn snapshot_changed_tree(
    workspace: &std::path::Path,
    target_dir: &std::path::Path,
    target: &str,
) -> String {
    let repository =
        GitLineageRepository::init_or_open_bare(workspace.join(".singularity/repo.git"))
            .expect("open lineage repository");
    repository
        .snapshot(
            target_dir,
            serde_json::json!({
                "op": "test-change",
                "target": target,
            }),
        )
        .expect("snapshot changed tree")
        .as_str()
        .to_owned()
}
```

- [ ] **Step 2: Write the Agent diff-and-undo tracer**

Append:

```rust
#[test]
fn agent_diff_and_undo_restore_the_complete_directory() {
    let workspace = tempfile::tempdir().expect("create workspace");
    init::initialize(workspace.path()).expect("initialize workspace");
    let imported = import_artifact(
        workspace.path(),
        fixture("fixtures/assets/agent/basic-agent"),
    )
    .expect("import Agent");
    let target_dir = workspace.path().join("agents/code-review");
    let baseline_manifest = fs::read(target_dir.join("agent.yaml")).expect("read manifest");
    let baseline_prompt = fs::read(target_dir.join("prompt.md")).expect("read prompt");

    fs::write(target_dir.join("prompt.md"), "changed prompt\n").expect("change prompt");
    fs::write(target_dir.join("notes.md"), "new file\n").expect("add file");
    let changed = snapshot_changed_tree(
        workspace.path(),
        &target_dir,
        "agent:code-review",
    );

    let diff = diff_revisions(workspace.path(), &imported.revision, &changed)
        .expect("diff Agent revisions");
    assert!(diff.contains("prompt.md"));
    assert!(diff.contains("+changed prompt"));
    assert!(diff.contains("notes.md"));

    let undone = undo_revision(
        workspace.path(),
        "agent:code-review",
        &imported.revision,
    )
    .expect("undo Agent revision");

    assert_eq!(fs::read(target_dir.join("agent.yaml")).unwrap(), baseline_manifest);
    assert_eq!(fs::read(target_dir.join("prompt.md")).unwrap(), baseline_prompt);
    assert!(!target_dir.join("notes.md").exists());
    assert_ne!(undone.restoration_revision, imported.revision);
    assert!(undone.record_path.is_file());
}
```

- [ ] **Step 3: Run the Agent undo test and verify RED**

Run:

```bash
cargo test -p sge-app --test asset_lifecycle \
  agent_diff_and_undo_restore_the_complete_directory -- --exact
```

Expected: FAIL because `undo_revision` restores into `skills/code-review`.

- [ ] **Step 4: Resolve undo destinations through capabilities**

Change the `sge-domain` import in `crates/sge-app/src/undo.rs`:

```rust
use sge_domain::{TargetRef, capabilities_for};
```

Replace the hard-coded standard directory:

```rust
let capabilities = capabilities_for(target_ref.kind());
let standard_dir = workspace
    .join(capabilities.layout().workspace_directory)
    .join(target_ref.name());
```

Do not change `replace_directory_from_revision`, snapshot metadata, or undo record semantics.

- [ ] **Step 5: Run the Agent test and existing Skill undo regression**

Run:

```bash
cargo test -p sge-app --test asset_lifecycle \
  agent_diff_and_undo_restore_the_complete_directory -- --exact
cargo test -p sge-app --test apply_undo
```

Expected: both commands pass.

- [ ] **Step 6: Add the Rule diff-and-undo behavior**

Append:

```rust
#[test]
fn rule_diff_and_undo_restore_the_complete_directory() {
    let workspace = tempfile::tempdir().expect("create workspace");
    init::initialize(workspace.path()).expect("initialize workspace");
    let imported = import_artifact(
        workspace.path(),
        fixture("fixtures/assets/rule/basic-rule"),
    )
    .expect("import Rule");
    let target_dir = workspace.path().join("rules/code-review");
    let baseline_rules = fs::read(target_dir.join("rules.md")).expect("read rules");

    fs::write(target_dir.join("rules.md"), "changed rules\n").expect("change rules");
    let changed = snapshot_changed_tree(
        workspace.path(),
        &target_dir,
        "rule:code-review",
    );
    let diff = diff_revisions(workspace.path(), &imported.revision, &changed)
        .expect("diff Rule revisions");
    assert!(diff.contains("rules.md"));
    assert!(diff.contains("+changed rules"));

    let undone = undo_revision(
        workspace.path(),
        "rule:code-review",
        &imported.revision,
    )
    .expect("undo Rule revision");

    assert_eq!(fs::read(target_dir.join("rules.md")).unwrap(), baseline_rules);
    assert_ne!(undone.restoration_revision, imported.revision);
}
```

- [ ] **Step 7: Run all lifecycle tests**

Run:

```bash
cargo test -p sge-app --test asset_lifecycle
cargo test -p sge-app --test apply_undo
```

Expected: all tests pass.

- [ ] **Step 8: Commit multi-kind undo**

```bash
git add crates/sge-app/src/undo.rs crates/sge-app/tests/asset_lifecycle.rs
git commit -m "feat: restore Agent and Rule revisions"
```

### Task 6: Refactor at GREEN and Run the Slice Gate

**Files:**
- Modify only if duplication remains: `crates/sge-app/src/import.rs`
- Modify only if formatting requires it: files changed in Tasks 1-5
- Verify: `docs/superpowers/specs/2026-08-14-agent-rule-lifecycle-design.md`

- [ ] **Step 1: Confirm the focused suite is GREEN before refactoring**

Run:

```bash
cargo test -p sge-domain --test capabilities
cargo test -p sge-app --test asset_lifecycle
cargo test -p sge-app --test import_skill
cargo test -p sge-app --test apply_undo
```

Expected: all commands pass. Do not refactor if any command is RED.

- [ ] **Step 2: Remove only demonstrated duplication**

Inspect `import_artifact` for repeated capability-derived values. If `target.to_string()` or
`layout` fields are recomputed, bind them once:

```rust
let target_string = target.to_string();
let layout = capabilities.layout();
```

Use `target_string` consistently in snapshot metadata and `ImportedArtifact`. Do not introduce a
generic repository, factory, plugin registry, or CLI abstraction.

- [ ] **Step 3: Re-run focused tests after refactoring**

Run:

```bash
cargo test -p sge-domain
cargo test -p sge-app --test asset_lifecycle
cargo test -p sge-app --test import_skill
cargo test -p sge-app --test apply_undo
```

Expected: all commands pass.

- [ ] **Step 4: Run formatting and Clippy**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
```

Expected: both commands exit successfully with no warnings.

- [ ] **Step 5: Run the workspace test and architecture gates**

Run:

```bash
cargo nextest run --workspace
cargo xtask architecture
git diff --check
```

Expected:

- every workspace test passes with zero failures;
- architecture dependency rules pass;
- diff whitespace validation reports no errors.

- [ ] **Step 6: Review the final diff against scope**

Run:

```bash
git status --short
git diff -- crates/sge-domain crates/sge-app fixtures/assets
```

Verify:

- no changes exist in `scan.rs`, `evolve.rs`, `apply.rs`, provider crates, protocol schemas, or CLI;
- no generated files or credentials are present;
- all new errors have deterministic `SGE-IMPORT-*` codes;
- whole-directory undo is covered for Agent and Rule;
- existing Skill tests remain unchanged unless compilation required a mechanical update.

- [ ] **Step 7: Commit the verified slice if refactoring changed files**

If Step 2 changed files:

```bash
git add crates/sge-domain crates/sge-app fixtures/assets
git commit -m "refactor: consolidate artifact lifecycle resolution"
```

If Step 2 produced no changes, do not create an empty commit.

## 4. Completion Gate

The implementation is complete only when:

- `capabilities_for` is the only source of canonical layout metadata;
- Agent, Skill, and Rule import through one application function;
- malformed source bundles fail before canonical destination creation;
- Agent and Rule diff through the existing revision API;
- explicit Agent and Rule undo creates a new restoration revision;
- existing Skill import and apply/undo tests pass unchanged;
- the workspace nextest, Clippy, formatting, architecture, and diff gates pass.
