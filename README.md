# SINGULARITY ENGINE

> 让 Agent、Skill 与 Rule 像代码一样被管理、评测和进化。

SINGULARITY ENGINE 是一个本地数据优先的 AI 资产工程工具链。它希望把不可解释的 Prompt 调整，转化为可指定目标、可重复评测、可审计解释、可安全应用、可完整回滚的工程流程。

项目当前处于 **Pre-alpha** 阶段。协议与本地存储基础已经可用，首个 Skill 垂直进化闭环仍在开发中，暂不建议用于生产环境。

## 为什么需要它

Agent、Skill 与 Rule 的改进通常依赖人工试错，容易遇到以下问题：

- 修改目标不明确，优化结果无法量化；
- Prompt 变强的同时引入安全、稳定性或兼容性退化；
- 缺少候选版本、评测证据和决策过程；
- 修改直接污染标准源，失败后难以可靠恢复；
- 同一能力在不同 AI 编程宿主中重复维护。

SINGULARITY ENGINE 以“资产协议 + 多维评测 + 内部 Git 谱系 + 人工确认门禁”为基础，让每一次进化都有证据，每一次试错都能回滚。

## 当前能力

已实现：

- Rust workspace 与模块化单体架构；
- Agent、Skill、Rule 目标引用和名称校验；
- Artifact、Contract、Evidence、Memory、Adapter 五类 V1 协议；
- 提交级 JSON Schema 与 schema drift 检查；
- 本地工作区初始化和结构校验；
- 内部 bare Git 快照、恢复与完整性检查；
- append-only journal 与中断恢复分类；
- Skill 安全导入、声明文件白名单和路径逃逸防护；
- 多维 `MetricVector`、硬门优先比较和确定性评测；
- recorded provider 驱动的安全 Prompt 变异和三候选隔离评估；
- baseline、受保护指标和主目标驱动的候选选择；
- 归一化 replay hash，屏蔽 case 顺序、时间戳和本机路径差异；
- CLI JSON 输出、架构依赖检查和 CI 质量门禁。

当前 CLI 命令：

```text
sge init [PATH] [--json]
sge import <PATH> [--workspace <WORKSPACE>] [--json]
sge scan <TARGET> [--workspace <WORKSPACE>] [--json] [--approve <PROPOSAL_ID> | --goal <GOAL>]
sge evolve <TARGET> --approve <PROPOSAL_ID> --provider-fixture <PATH> [--candidates 3] [--workspace <WORKSPACE>] [--json]
sge test [<TARGET> [--candidate <WORKTREE>] | --replay <RUN_ID>] [--workspace <WORKSPACE>] [--json]
sge explain <RUN_ID> [--workspace <WORKSPACE>] [--json]
sge history <TARGET> [--workspace <WORKSPACE>] [--json]
sge diff <REVISION_A> <REVISION_B> [--workspace <WORKSPACE>] [--json]
sge apply <RUN_ID> --approve [--workspace <WORKSPACE>] [--json]
sge undo <RUN_ID> [--workspace <WORKSPACE>] [--json]
sge undo --revision <REVISION> --target <TARGET> [--workspace <WORKSPACE>] [--json]
```

开发中：

- Claude Code、Codex、OpenCode、OpenClaw 等宿主适配；
- 统一源 Singularity Skill。

## 快速开始

### 环境要求

- macOS、Linux 或 Windows；
- Rust stable，最低版本 `1.97`；
- Git。

### 从源码构建

```bash
git clone https://github.com/abinzhao/singularity-engine.git
cd singularity-engine
cargo build --workspace
```

开发环境可直接运行：

```bash
cargo run -p sge-cli -- --help
```

### 初始化工作区

```bash
cargo run -p sge-cli -- init ./my-lab --json
```

初始化后会生成：

```text
my-lab/
├── singularity.yaml
├── agents/
├── skills/
├── rules/
├── memory/
├── evals/
└── .singularity/
    ├── repo.git/
    ├── worktrees/
    ├── runs/
    ├── cache/
    └── installs/
```

`.singularity/repo.git` 是 SINGULARITY ENGINE 使用的内部谱系仓库，不会在业务目录创建或接管 `.git`。

### 导入 Skill

一个最小 Skill：

```text
code-review/
├── skill.yaml
└── instructions.md
```

`skill.yaml`：

```yaml
schema: sge.dev/artifact/v1
id: code-review-skill-v1
kind: skill
name: code-review
title: Code Review Skill
summary: Review code with explicit safety rules.
body: see instructions.md
files:
  - path: instructions.md
    required: true
```

导入：

```bash
cargo run -p sge-cli -- import ./code-review \
  --workspace ./my-lab \
  --json
```

成功输出包含稳定目标引用、内部 revision 和 warnings：

```json
{
  "ok": true,
  "code": "OK",
  "target": "skill:code-review",
  "revision": "<git-oid>",
  "warnings": []
}
```

导入过程只复制 manifest 声明的文件，并拒绝重复名称、缺失文件、符号链接和越界路径。

### 扫描并批准提案

`scan` 只使用 `evals/results` 中已确认的 Evidence 和 `memory/failures` 中已确认的
Memory，提案写入独立运行目录，不会修改 Skill 标准源：

```bash
cargo run -p sge-cli -- scan skill:code-review \
  --workspace ./my-lab \
  --json
```

确认提案后，显式批准会在同一运行目录生成版本化 Contract：

```bash
cargo run -p sge-cli -- scan skill:code-review \
  --workspace ./my-lab \
  --approve prop-sql-injection-guard \
  --json
```

### 生成并评估隔离候选

当前 P1 使用 recorded provider fixture 运行可复现的离线进化：

```bash
cargo run -p sge-cli -- evolve skill:code-review \
  --workspace ./my-lab \
  --approve prop-sql-injection-guard \
  --provider-fixture ./fixtures/provider/prompt-candidates.json \
  --candidates 3 \
  --json
```

每个候选位于 `.singularity/worktrees/<run-id>/<candidate-id>`，拥有独立 revision
和评估证据。编排按硬门、受保护指标、主目标依次筛选，并停在
`ReviewPending`；此阶段不会修改 `skills/code-review` 标准源。

重新评估 baseline 或指定候选：

```bash
cargo run -p sge-cli -- test skill:code-review --workspace ./my-lab --json
cargo run -p sge-cli -- test skill:code-review \
  --workspace ./my-lab \
  --candidate ./.singularity/worktrees/<run-id>/<candidate-id> \
  --json
```

每次 evolve run 会保存 `contract.yaml`、`baseline.json`、`proposals.json`、候选评估、
`decision.md`、`mutation.patch` 和 `replay.yaml`。`decision.md` 只从 typed metrics、
淘汰原因、证据路径与 SHA-256 哈希生成，不把模型原文当作可信解释。

读取和复放证据：

```bash
cargo run -p sge-cli -- explain <run-id> --workspace ./my-lab
cargo run -p sge-cli -- history skill:code-review --workspace ./my-lab --json
cargo run -p sge-cli -- diff <baseline-revision> <candidate-revision> \
  --workspace ./my-lab
cargo run -p sge-cli -- test --replay <run-id> --workspace ./my-lab --json
```

Replay 会从内部不可变 revision 重新评估 baseline 与候选，同时校验持久化 evidence
文件的 SHA-256；任一指标复放哈希或 evidence 哈希变化都会返回 mismatch。

### 应用与撤销已验证候选

`apply` 只接受处于 `ReviewPending`、Replay 通过且拥有获胜候选的 run，并要求显式
`--approve`：

```bash
cargo run -p sge-cli -- apply <run-id> \
  --workspace ./my-lab \
  --approve \
  --json
```

应用事务以整个 `skills/<name>` 目录为单位。在同一父目录恢复 staging，校验标准源
仍等于进化 baseline，再通过 backup 和目录 rename 切换。备份后发生故障时会恢复原目录，
不会留下半写入的 Skill。成功后会生成新的 applied revision 和 `apply.json`。

按 applied run 或指定内部 revision 撤销：

```bash
cargo run -p sge-cli -- undo <run-id> --workspace ./my-lab --json
cargo run -p sge-cli -- undo \
  --revision <revision> \
  --target skill:code-review \
  --workspace ./my-lab \
  --json
```

Undo 不重写 Git 历史，而是把目标 revision 恢复到标准目录后创建新的 restoration
revision。若标准 Skill 在 review 或 apply 后被用户修改，apply/undo 会拒绝覆盖。

## 核心设计

### 本地数据优先

工作区、记忆、证据、运行记录和版本谱系默认保存在本机。本地数据优先不等于永不联网：未来的模型提供方由用户显式配置，持久化边界仍保持在本地。

### 协议优先

所有公开文档都通过显式 `schema` 字段分发。未知版本会被拒绝，不会静默回退；扩展字段在 round-trip 中保留。

```yaml
schema: sge.dev/artifact/v1
```

当前协议类型：

| 类型 | 用途 |
| --- | --- |
| Artifact | 描述 Agent、Skill、Rule 资产 |
| Contract | 定义进化目标、硬门和受保护指标 |
| Evidence | 保存评测结果与可审计证据 |
| Memory | 保存带来源和置信度的经验记录 |
| Adapter | 描述宿主能力与安装映射 |

JSON Schema 位于 [`schemas/v1`](schemas/v1)。

### 不使用通用加权总分

评测结果保留多维指标：

```rust
pub struct MetricVector {
    pub task_success: f64,
    pub safety: f64,
    pub latency_p95_ms: u64,
    pub token_cost: u64,
    pub stability: f64,
    pub compatibility: f64,
}
```

候选版本先通过硬门，再比较主目标，同时保护 safety、stability 等指标。高任务成功率不能抵消安全门失败。

### 人工确认门禁

扫描和评测可以自动化，方向批准、权限扩张、应用到标准源和宿主安装必须经过显式确认。模型输出只被视为候选数据，不被视为授权。

### 可恢复谱系

候选修改与标准源隔离，内部 Git 保存不可变 revision，journal 记录状态迁移。中断状态只会被分类为可恢复或可终止，不会被误判为完成。

## 架构

```text
sge-cli
   │
   ▼
sge-app ───────────────┐
   │                   │
   ▼                   ▼
sge-domain         sge-store
   ▲
   │
sge-protocol

sge-eval  ── 独立评测与契约比较
xtask     ── Schema 生成与架构约束检查
```

核心依赖规则：

- CLI 只做参数与输出适配，不承载业务逻辑；
- `sge-domain` 不依赖存储、CLI 或具体宿主；
- `sge-protocol` 不依赖 CLI；
- 基础设施可以依赖领域层，领域层不能反向依赖基础设施；
- `cargo xtask architecture` 持续检查依赖方向。

详细决策见 [`docs/adr/0001-modular-monolith.md`](docs/adr/0001-modular-monolith.md)。

## 开发与验证

常用命令：

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo xtask architecture
cargo xtask schema
git diff --check
```

协议兼容性测试：

```bash
cargo test -p sge-protocol
```

确定性评测测试：

```bash
cargo test -p sge-eval
```

生成的 JSON Schema 必须与仓库中的提交版本一致。修改协议类型后运行：

```bash
cargo xtask schema
```

## 项目路线图

| 阶段 | 目标 | 状态 |
| --- | --- | --- |
| P0 | 协议、工作区、内部 Git、恢复与架构门禁 | 已完成 |
| P1 | 单个 Skill 的完整进化、解释、应用与回滚闭环 | 进行中 |
| P2 | Agent、Skill、Rule 完整命令面与更多变异算子 | 规划中 |
| P3 | Singularity Skill 与多宿主事务式适配 | 规划中 |
| P4 | 沙箱、安全策略与故障恢复强化 | 规划中 |
| P5 | 发布工程、兼容门禁与公开发行 | 规划中 |

实施计划位于 [`docs/superpowers/plans`](docs/superpowers/plans)。

## 安全边界

当前实现遵循以下约束：

- 不读取或提交模型服务凭据；
- CI 不调用在线模型；
- 导入只接受声明文件并拒绝符号链接；
- 候选修改在显式 apply 前不得写入标准源；
- 未知协议版本直接失败；
- 任何宿主写入、权限扩张和高风险操作都必须经过人工确认。

发现安全问题时，请不要在公开 Issue 中披露可利用细节。项目建立正式安全策略前，请通过 GitHub 私密安全报告联系维护者。

## 贡献

项目仍在快速建立核心协议和行为边界。提交贡献前请：

1. 先通过 Issue 描述问题、目标和兼容影响；
2. 保持改动聚焦，不混入无关重构；
3. 为行为变化添加失败用例和回归测试；
4. 运行完整质量门禁；
5. 不提交凭据、真实敏感数据或在线模型录制内容。

提交信息建议使用：

```text
feat: add structured model provider contract
fix: reject undeclared import paths
test: prove replay normalization
docs: clarify local-first security boundary
```

## 许可证

本项目基于 [MIT License](LICENSE) 开源。

## 致谢

SINGULARITY ENGINE 受到编译器验证、演化计算、可复现实验和 Git 内容寻址模型的启发。它不是一个替代 AI 编程宿主的 Chat Runtime，而是服务于 Agent、Skill 与 Rule 生命周期的本地工程基础设施。
