# P5 Release Engineering Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce reproducible, signed, cross-platform SINGULARITY ENGINE V1 artifacts with complete documentation, compatibility evidence, install verification, and a reversible release process.

**Architecture:** Build Rust binaries once per target from a tagged source revision, generate checksums and provenance, and let the npm package download verified binaries. Separate artifact creation from publication; promotion must never rebuild.

**Tech Stack:** Cargo, cross or native target runners, GitHub Actions, npm, cargo-dist or equivalent scripted packaging, SHA-256 checksums, release attestations, cargo-deny, cargo-audit.

---

## File Map

```text
packages/npm/                         npm bootstrapper and platform resolver
.github/workflows/{ci,nightly,release}.yml
dist-workspace.toml                   release target configuration
docs/{install,quickstart,cli,providers,hosts,security}/
docs/evidence/p5/
scripts/{verify-dist,smoke-install}.sh
CHANGELOG.md
LICENSE
SECURITY.md
CONTRIBUTING.md
```

### Task 1: Define version and release identity

- [ ] Add one workspace version source and ensure `sge --version`, Cargo packages, npm package, schemas, and evidence report the same version.
- [ ] Write `crates/sge-cli/tests/version.rs` that compares CLI and Cargo metadata.
- [ ] Add a release metadata structure containing source commit, dirty flag, target triple, Rust version, schema version, and adapter versions.
- [ ] Run the focused test; expect PASS.
- [ ] Commit with `build: define release identity`.

### Task 2: Build the npm binary bootstrapper

**Files:**
- Create: `packages/npm/{package.json,install.js,index.js,README.md}`
- Create: `packages/npm/test/install.test.js`

- [ ] Write tests for supported platform mapping, unsupported platforms, checksum mismatch, interrupted download, proxy handling, and offline cached install.
- [ ] Implement download to a temporary path, SHA-256 verification, executable permission, and atomic rename.
- [ ] Never run arbitrary downloaded scripts.
- [ ] Add `npx singularity-engine init` forwarding to the installed `sge`.
- [ ] Run `npm test --prefix packages/npm`; expect PASS.
- [ ] Commit with `feat: add verified npm binary installer`.

### Task 3: Configure cross-platform artifacts

**Files:**
- Create: `dist-workspace.toml`
- Create: `scripts/verify-dist.sh`
- Modify: `.github/workflows/release.yml`

- [ ] Define macOS arm64/x64, Linux x64/arm64, and Windows x64 targets.
- [ ] Produce archives, SHA-256 files, license, notices, and shell completions.
- [ ] Add a local dry-run command that does not publish:

```bash
cargo dist build --artifacts=local
./scripts/verify-dist.sh target/distrib
```

- [ ] Verify archive contents and executable versions.
- [ ] Commit with `build: package cross-platform binaries`.

### Task 4: Enforce build-once promotion

**Files:**
- Modify: `.github/workflows/release.yml`
- Create: `scripts/promote-release.sh`
- Create: `docs/release-process.md`

- [ ] Split release workflow into `build`, `verify`, and `publish` jobs.
- [ ] Pass immutable artifact IDs and checksums between jobs.
- [ ] Prevent publish from invoking Cargo build.
- [ ] Require a version tag matching workspace and npm versions.
- [ ] Document abort and rollback: stop before publish, deprecate npm package version if needed, remove hosted release assets only with explicit approval, and publish a new corrective version instead of replacing artifacts.
- [ ] Commit with `ci: separate release build and promotion`.

### Task 5: Add supply-chain and dependency gates

**Files:**
- Create: `deny.toml`
- Modify: `.github/workflows/{ci,nightly,release}.yml`
- Create: `SECURITY.md`

- [ ] Configure license allowlist, duplicate dependency policy, source allowlist, and advisory checks.
- [ ] Add `cargo deny check` and `cargo audit`.
- [ ] Generate release provenance/attestations using the hosted CI identity.
- [ ] Document vulnerability reporting and supported versions.
- [ ] Commit with `security: gate dependencies and release provenance`.

### Task 6: Write user and operator documentation

**Files:**
- Create: `README.md`
- Create: `docs/install.md`
- Create: `docs/quickstart.md`
- Create: `docs/cli.md`
- Create: `docs/providers.md`
- Create: `docs/hosts.md`
- Create: `docs/memory.md`
- Create: `docs/evaluation.md`
- Create: `docs/security.md`
- Create: `docs/recovery.md`

- [ ] Generate CLI reference from clap help and fail CI on drift.
- [ ] Include one clean-machine quickstart using recorded/offline fixtures and one real-provider setup.
- [ ] State local-data-first network semantics and sandbox non-guarantees explicitly.
- [ ] Document every rollback command next to its side-effecting command.
- [ ] Add docs link and command tests.
- [ ] Commit with `docs: add complete V1 operator guidance`.

### Task 7: Build public demo and replay bundle

**Files:**
- Create: `examples/broken-code-review-skill/`
- Create: `examples/test-generation-agent/`
- Create: `examples/project-rules/`
- Create: `scripts/replay-demo.sh`
- Create: `docs/demo.md`

- [ ] Create deterministic before/evolve/test/explain/apply fixtures.
- [ ] Ensure the demo exposes a real failing case and does not fabricate metrics.
- [ ] Make `scripts/replay-demo.sh` run without network using recorded provider data.
- [ ] Verify the replay bundle on a clean temporary directory.
- [ ] Commit with `docs: add reproducible V1 demos`.

### Task 8: Add clean-machine install smoke tests

**Files:**
- Create: `scripts/smoke-install.sh`
- Modify: `.github/workflows/release.yml`

- [ ] Test archive install on each target runner.
- [ ] Test npm install on supported Node LTS versions.
- [ ] Run `sge doctor`, `sge init`, protocol validation, recorded evolution replay, and `sge undo`.
- [ ] Verify uninstall removes only package-managed files.
- [ ] Commit with `test: verify clean-machine installs`.

### Task 9: Production readiness and RC gate

**Files:**
- Create: `docs/evidence/p5/readiness.md`
- Create: `docs/evidence/p5/compatibility.md`
- Create: `docs/evidence/p5/reproducibility.md`

- [ ] Record all V1 requirements with evidence links.
- [ ] Record five-host versions and conformance results.
- [ ] Build the same tag twice in isolated jobs and compare normalized artifact hashes; document unavoidable platform-signing differences separately.
- [ ] Run fault, security, protocol, and adapter suites from a release candidate artifact.
- [ ] Do not promote if any Stable operator, rollback path, or host adapter is missing evidence.
- [ ] Commit with `docs: record V1 release readiness`.

### Task 10: Cut the release only after explicit authorization

This task is intentionally gated. Before any tag, package publication, hosted release, or artifact promotion:

- [ ] Read the Staff Engineer Mode `release-build-reproducibility` specialist.
- [ ] Read the Staff Engineer Mode `production-readiness-review` specialist.
- [ ] Present both structured review artifacts to the user.
- [ ] Record the approval receipt separately.
- [ ] Create a new signed tag without rewriting history.
- [ ] Publish the already-built verified artifacts.
- [ ] Verify npm, archives, checksums, provenance, and quickstart.

No release command is authorized by this plan alone.

## P5 / V1 Exit Gate

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
cargo dist build --artifacts=local
./scripts/verify-dist.sh target/distrib
./scripts/replay-demo.sh
```

Expected:

- every command exits `0`;
- public schemas and CLI JSON remain compatible;
- all fourteen operators have declared maturity and evidence;
- all five adapters pass conformance;
- security and recovery gates pass from release artifacts;
- clean-machine replay reproduces deterministic evidence;
- release artifacts are immutable between verification and publication.

