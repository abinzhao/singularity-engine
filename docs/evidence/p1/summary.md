# P1 Vertical Evolution Evidence

## Scope

P1 proves one imported Skill can complete:

```text
scan → contract → mutate → evaluate → review
→ replay → apply → undo
```

The phase uses only committed recorded-provider fixtures. No live model credentials or network
model calls are required.

## Local Phase Demo

Executed on 2026-08-14 from the repository root with a temporary workspace:

```text
/tmp/sge-p1-demo.QNRXoU
```

Observed result:

| Field | Value |
| --- | --- |
| Run ID | `evolve-18cb6b6f6ae3a3b8-10719-0` |
| Selected candidate | `candidate-2` |
| Replay matched | `true` |
| Applied revision | `bc89ba134b9d71a316e94c3cce79b5de05216ffc` |
| Restoration revision | `48dd93c98d16bda2232f94fff58cdccd6fa5824c` |
| Skill hash before apply | `97256b1d3be23040ea765c4ccf56a30b3e97a0f16cb350e12099fb3019ec48aa` |
| Skill hash after undo | `97256b1d3be23040ea765c4ccf56a30b3e97a0f16cb350e12099fb3019ec48aa` |

The matching directory hashes prove undo restored the complete standard Skill tree, not only
`instructions.md`.

Each evolution run contains:

- `contract.yaml`
- `baseline.json`
- `proposals.json`
- candidate evaluation JSON
- `decision.md`
- `mutation.patch`
- `replay.yaml`
- append-only `journal.ndjson`

## Quality Gates

Commands executed:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo nextest run --workspace
cargo xtask architecture
git diff --check
```

Results:

- `cargo-nextest 0.9.143`
- 71 tests passed, 0 failed, 0 skipped
- formatting passed
- Clippy passed with warnings denied
- architecture dependency rules passed
- diff whitespace validation passed
- `.github/workflows/ci.yml` parsed as valid YAML

## Exit Gate

| Requirement | Evidence | Status |
| --- | --- | --- |
| Imported Skill completes scan through review | application and CLI vertical tests | Passed |
| Candidate writes remain isolated before apply | per-candidate internal worktrees and source hash tests | Passed |
| Selection preserves multidimensional metrics | hard-gate and protected-metric tests | Passed |
| Explanation and replay use typed evidence | evidence completeness, hash, and replay tests | Passed |
| Apply and undo are fault-tested | backup fault injection, stale-source, full-tree restore tests | Passed |
| CI uses no live model credentials | recorded fixture only `p1-vertical-e2e` job | Passed |

## CI Evidence

The `p1-vertical-e2e` GitHub Actions job runs the same recorded flow and uploads:

- command JSON outputs
- selected run ID
- `.singularity/runs/` normalized evidence

GitHub Actions run
[`31724855006`](https://github.com/abinzhao/singularity-engine/actions/runs/31724855006)
completed successfully on 2026-08-14. All six jobs passed: formatting, Clippy, unit tests,
protocol compatibility, architecture rules, and the P1 vertical E2E flow.

The run archived `p1-vertical-evidence` as artifact `9190835284` (12,806 bytes).

## Residual Risk

The directory transaction is rollback-safe for injected process failures and fsyncs files and
directories. A host power loss between the two directory renames can leave the backup directory
requiring explicit recovery; crash-start recovery for that interval is deferred beyond P1.
