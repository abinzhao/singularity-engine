# P2 Agent/Rule Lifecycle Design

**Status:** Approved for implementation planning
**Date:** 2026-08-14
**Parent plan:** `docs/superpowers/plans/2026-08-13-p2-full-evolution-surface.md`
**Scope:** P2 Task 1 only

## 1. Goal

Extend the proven artifact lifecycle from Skill-only imports to Agent, Skill, and Rule artifacts
without creating three separate application pipelines.

The slice is complete when one Agent and one Rule can:

1. be discovered and parsed from a source directory;
2. be validated before workspace mutation;
3. be imported into the canonical workspace directory;
4. be snapshotted into the existing internal Git lineage;
5. participate in revision-level diff;
6. be restored by explicit revision while creating a new restoration revision.

## 2. Non-Goals

This slice does not:

- evolve Agent or Rule content;
- enable Agent or Rule scan, apply, or replay;
- implement mutation operator selection or the fourteen-operator registry;
- validate cross-asset references, permission compatibility, or Rule priority cycles;
- complete the full `sge history` and `sge diff` CLI target surface assigned to P2 Task 9;
- change the Artifact V1 protocol schema;
- add a database or another source of truth.

`scan`, `evolve`, and run-based `apply` remain Skill-only until their later P2 tasks add the
required operator and composition policies.

## 3. Confirmed Existing Behavior

- `ArtifactKind` and `TargetRef` already model `agent`, `skill`, and `rule`.
- Workspace initialization already creates `agents/`, `skills/`, and `rules/`.
- `import_artifact` is currently hard-coded to `skill.yaml`, `kind: skill`, and `skills/<name>`.
- `undo_revision` parses a generic `TargetRef` but restores only into `skills/<name>`.
- `diff_revisions` compares arbitrary internal Git revisions and is already asset-agnostic.
- Internal Git snapshots store operation metadata including the target string.
- Artifact documents expose extensible `files` data without requiring a protocol change.

## 4. Chosen Architecture

### 4.1 Domain-owned capability registry

Add `crates/sge-domain/src/capability.rs`. The domain crate owns the mapping from artifact kind to
layout and lifecycle capabilities. Application and CLI code must not add independent
`match ArtifactKind` branches for paths or manifest names.

The central interface is:

```rust
pub trait ArtifactCapabilities: Sync {
    fn kind(&self) -> ArtifactKind;
    fn layout(&self) -> ArtifactLayout;
    fn mutable_surfaces(&self) -> &'static [MutableSurface];
    fn evaluation_requirements(&self) -> EvaluationRequirements;
    fn validate_composition(&self, workspace: &WorkspaceView<'_>)
        -> Result<(), CapabilityError>;
}
```

The registry exposes one lookup:

```rust
pub fn capabilities_for(kind: ArtifactKind) -> &'static dyn ArtifactCapabilities;
```

Static implementations avoid runtime registration, duplicate kinds, and initialization order.
They also give the later operator registry one authoritative source for supported surfaces.

### 4.2 Typed layout

```rust
pub struct ArtifactLayout {
    pub workspace_directory: &'static str,
    pub manifest_filename: &'static str,
    pub default_declared_files: &'static [&'static str],
}
```

The V1 layout is fixed:

| Kind | Workspace directory | Manifest | Default declared file |
| --- | --- | --- | --- |
| Agent | `agents` | `agent.yaml` | `prompt.md` |
| Skill | `skills` | `skill.yaml` | `instructions.md` |
| Rule | `rules` | `rule.yaml` | `rules.md` |

An explicit non-empty `files` array replaces the default file list. Missing or malformed `files`
must not silently produce an empty artifact. If `files` is absent, the default applies. If it is
present but invalid, validation fails.

### 4.3 Initial surfaces and evaluation requirements

This slice introduces only the values required to describe the three current content surfaces:

```rust
pub enum MutableSurface {
    Prompt,
    SkillInstructions,
    Rules,
}
```

`EvaluationRequirements` is a typed value returned by every capability implementation. In this
slice it records the primary mutable surface and whether composition validation is required. It
does not define metric thresholds or mutation policy.

`validate_composition` checks only lifecycle-local invariants in this slice: the target kind and
name match the manifest and all required declared files exist. Cross-asset graph validation remains
deferred to P2 Task 8.

## 5. Artifact Resolution

`import_artifact(workspace, source_dir)` keeps its public signature. Resolution proceeds as follows:

1. Inspect the source directory for the three recognized manifest filenames.
2. Require exactly one recognized manifest.
3. Parse it as `ArtifactDocument`.
4. Parse `artifact.kind` into `ArtifactKind`.
5. Resolve capabilities from the registry.
6. Require the discovered manifest filename to equal the capability layout manifest.
7. Parse and validate `artifact.name` through `ArtifactName`.
8. Build `TargetRef` and the canonical destination path from capabilities.

Zero manifests produce a missing-manifest import error. Multiple recognized manifests produce an
ambiguous-manifest import error. A filename/kind mismatch produces a kind mismatch error and no
workspace files are written.

## 6. Import Transaction

The import transaction preserves the existing staging-and-rename model:

```text
resolve manifest
  -> parse target
  -> validate declared paths and required files
  -> copy manifest and declared files into cache staging
  -> validate staged bundle
  -> rename staging to canonical destination
  -> snapshot canonical directory into internal Git
```

All validation must complete before the canonical destination is created. Declared files retain
the current protections:

- no absolute or parent-traversal paths;
- canonical paths must remain under the source root;
- symbolic links are refused;
- required files must exist;
- duplicate targets are refused within the same artifact kind.

An Agent and Skill with the same name may coexist because their canonical roots differ.

Snapshot metadata remains structured:

```json
{
  "op": "import",
  "target": "agent:code-review"
}
```

The imported result returns the canonical target and the created revision.

## 7. Revision Diff and Undo

`diff_revisions` requires no behavioral change. Lifecycle conformance tests will prove that it
reports Agent and Rule manifest/body changes from internal Git revisions.

`undo_revision` resolves the target through `capabilities_for(target.kind())`, then restores into
the canonical directory from `ArtifactLayout`. It must:

1. reject an invalid target or revision before mutation;
2. restore the complete artifact directory, not only the default body file;
3. snapshot the restored directory as a new lineage revision;
4. persist the existing `undo.json` record with the exact target;
5. never rewrite or delete prior revisions.

Run-based `undo_run` remains unchanged because Agent and Rule do not yet produce evolution runs.

## 8. Error Model

Existing stable error codes remain unchanged where semantics are unchanged. Add distinct import
errors for:

- no recognized artifact manifest;
- more than one recognized artifact manifest;
- manifest filename and `kind` disagreement;
- malformed `files` declaration;
- invalid artifact name.

Errors must include the relevant path or target and must not expose file contents. All pre-rename
errors leave canonical workspace directories untouched. A failed snapshot after rename must return
an error and preserve the imported directory for explicit recovery; silently deleting user-visible
content is not allowed.

## 9. Fixtures

Add deterministic fixtures:

```text
fixtures/assets/agent/basic-agent/
├── agent.yaml
└── prompt.md

fixtures/assets/rule/basic-rule/
├── rule.yaml
└── rules.md
```

Each manifest uses `sge.dev/artifact/v1`, declares its body file through `files`, and contains no
host-specific generated content.

Negative cases should be created inside tests unless they are reused by more than one test. This
keeps the fixture surface small.

## 10. Testing Strategy

Use TDD with `crates/sge-app/tests/asset_lifecycle.rs` as the vertical acceptance test.

Required tests:

1. Agent import copies the full declared tree into `agents/<name>` and creates a revision.
2. Rule import copies the full declared tree into `rules/<name>` and creates a revision.
3. Agent and Skill with the same name do not conflict.
4. Missing, ambiguous, and filename/kind-mismatched manifests fail before workspace mutation.
5. Invalid or empty `files` declarations fail rather than importing an incomplete artifact.
6. Agent revision diff reports manifest and prompt changes.
7. Rule revision diff reports manifest and rules changes.
8. Explicit revision undo restores the complete Agent directory and creates a new revision.
9. Explicit revision undo restores the complete Rule directory and creates a new revision.
10. Existing Skill import and undo tests remain unchanged and pass.

Focused verification:

```bash
cargo test -p sge-domain
cargo test -p sge-app --test asset_lifecycle
cargo test -p sge-app --test import_skill
cargo test -p sge-app --test apply_undo
```

Slice gate:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo xtask architecture
git diff --check
```

## 11. Expected File Changes

```text
crates/sge-domain/src/capability.rs
crates/sge-domain/src/lib.rs
crates/sge-app/src/import.rs
crates/sge-app/src/undo.rs
crates/sge-app/src/lib.rs
crates/sge-app/tests/asset_lifecycle.rs
fixtures/assets/agent/basic-agent/agent.yaml
fixtures/assets/agent/basic-agent/prompt.md
fixtures/assets/rule/basic-rule/rule.yaml
fixtures/assets/rule/basic-rule/rules.md
```

Changes to `scan.rs`, `evolve.rs`, `apply.rs`, provider behavior, protocol schemas, or CLI command
shape are outside this slice unless compilation requires a mechanical import update.

## 12. Completion Criteria

- All three artifact kinds resolve layout through one domain capability registry.
- Import contains no Skill-only path or manifest assumption.
- Agent and Rule imports are validated and snapshotted with canonical targets.
- Revision diff works for Agent and Rule without asset-specific branches.
- Explicit undo restores the whole Agent or Rule directory and creates a restoration revision.
- Existing Skill behavior remains compatible.
- Focused and workspace-wide gates pass with no skipped lifecycle tests.
