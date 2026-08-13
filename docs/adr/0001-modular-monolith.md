# ADR-001: Modular Monolith With Protocol-First Boundaries

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
