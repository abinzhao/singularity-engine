# SINGULARITY ENGINE V1 产品定位与执行方案

- 文档状态：已批准设计
- 创建日期：2026-08-13
- 品牌署名：SINGULARITY ENGINE by ZJB.DEV
- 中文主张：让每一次变强，都有证据。
- 英文主张：Evolve with evidence.

## 1. 执行摘要

SINGULARITY ENGINE 是本地数据优先的 Agent、Skill 与 Rule 进化工具链。它将“变强”从不可解释的 Prompt 调整，转化为一套可指定方向、可重复评测、可审计解释、可安全安装、可完整回滚的工程流程。

产品只提供两种形态：

1. `sge` CLI：唯一完整操作端。
2. Singularity Skill：供 Claude Code、Codex、TRAE、OpenCode、OpenClaw 等宿主调用 `sge`。

产品不提供 Web UI、中心化云服务、公共榜单、在线 Registry 或完整 Agent Chat Runtime。状态、记忆、证据与谱系默认保存在本机。独立 CLI 模式可以使用用户配置的远程模型 API 或本地模型；宿主模式优先复用当前宿主模型。

项目不发布功能残缺的 MVP。首个公开版本为 V1.0。开发过程仅提供 Internal Build、Tech Preview、Private Beta 和 Release Candidate。

## 2. 产品定位

### 2.1 一句话定位

> SINGULARITY ENGINE 是你本地的 Agent 基因实验室：让 Agent、Skill 与 Rule 像代码一样被管理、被理解、被进化，每一次变强都有证据，每一次试错都能安全回滚。

### 2.2 核心价值

| 价值 | 用户获得的结果 |
|---|---|
| 方向可控 | 用户可以指定正确率、成本、安全、速度或领域能力等进化目标 |
| 无方向可引导 | 系统基于证据给出 2–5 个预选方向，由用户选择 |
| 结果可证明 | 使用用户自己的任务集和多维指标验证是否真正提升 |
| 过程可解释 | 每次进化生成 Contract、Diff、评测和决策说明 |
| 状态可回滚 | 资产、记忆、候选与宿主安装都有独立恢复点 |
| 结果可迁移 | 同一标准资产可以适配多个 AI 工具 |
| 数据可掌控 | 所有持久化数据默认留在本机 |

### 2.3 目标用户

V1 首要服务 AI 工具开发者和重度 AI 编程用户：

- 使用 Claude Code、Codex、TRAE、OpenCode 或 OpenClaw 的开发者；
- 维护 Agent、Skill、Rules、Prompts 或工具链的个人与团队；
- 需要在本地验证 Agent 改进效果的研究者和框架作者；
- 对隐私、审计、回滚和可复现性有明确要求的用户。

### 2.4 非目标

V1 明确不做：

- 不提供通用聊天客户端或完整 Agent Runtime；
- 不承诺无限递归自我改进；
- 不将单一综合分作为“变强”的唯一证据；
- 不静默读取宿主历史或修改宿主配置；
- 不允许模型输出绕过权限、评测和人工门禁；
- 不建设中心化账号、社交、排行榜或 Registry；
- 不声称 Process Sandbox 等价于安全容器；
- 不承诺所有进化在固定时间内完成。

## 3. 设计原则

1. **本地数据优先**：持久化状态、记忆、证据和谱系全部保存在本机。
2. **证据优先**：没有基线、评测和回放记录，就不能声称变强。
3. **人工门禁**：方向、预算、权限扩张和宿主安装均需要用户确认。
4. **标准源唯一**：Agent、Skill、Rule 只维护一份标准源，宿主格式由适配器生成。
5. **可逆变更**：进化、记忆迁移和宿主安装均必须支持回滚。
6. **多维评测**：主目标提升不能掩盖安全、成本或兼容性退化。
7. **渐进成熟**：算子与适配器使用 Stable、Preview、Experimental 标记。
8. **宿主无关**：核心进化逻辑不依赖任何单一 AI 工具。

## 4. 产品形态

### 4.1 `sge` CLI

`sge` 是唯一完整操作端，负责：

- 工作区初始化与资产导入；
- 诊断和方向预选；
- 进化状态机与候选管理；
- 评测、比较和停止判断；
- 内部 Git、证据和记忆管理；
- 宿主检测、转换、安装和回滚；
- Provider、沙箱和安全策略。

安装方式：

```bash
npm install -g singularity-engine
# 或
cargo install singularity-engine
```

npm 包只负责下载对应平台的 Rust 二进制，不引入 Node/Rust FFI。

### 4.2 Singularity Skill

Singularity Skill 是宿主调用层，不复制业务逻辑。它负责：

- 将自然语言意图转换为受 Schema 约束的 CLI 请求；
- 调用 `sge` 并解释结构化结果；
- 在宿主模型与 `sge` 之间传递候选分析；
- 引导用户确认方向、预算、权限和安装。

Skill 不独立保存状态，不直接修改文件，不将宿主生成文本视为授权。

统一源 Skill 在安装时由适配器转换为宿主格式。

## 5. 核心资产模型

### 5.1 资产类型

| 类型 | 可进化内容 |
|---|---|
| Agent | 身份、Prompt、Skill 组合、Rule、记忆引用、规划与验证策略 |
| Skill | 指令、触发条件、工具依赖、脚本、参考资料、输出 Schema、测试 |
| Rule | 项目规则、行为门禁、权限约束、输出要求、优先级和冲突策略 |

### 5.2 统一资产协议

```yaml
schema: sge.dev/artifact/v1
kind: skill
name: code-review
version: 1.4.0
capabilities:
  - code-analysis
permissions:
  filesystem: read
  network: deny
compatibility:
  hosts:
    - claude
    - codex
    - trae
    - opencode
    - openclaw
tests:
  suite: evals/code-review.yaml
```

协议规则：

- Agent 通过引用组合 Skill、Rule 和 Memory，不复制其内容；
- Skill 必须可独立打包、测试、进化和安装；
- Rule 必须声明作用域、优先级和冲突策略；
- 宿主专属字段保存在适配器覆盖层，不污染标准源；
- 未知字段默认保留，破坏性 Schema 变化必须提供迁移器；
- 所有资产都必须声明权限和评测入口。

## 6. 工作区结构

```text
my-agent/
├── singularity.yaml
├── agent/
│   ├── agent.yaml
│   └── prompt.md
├── skills/
│   └── code-review/
│       ├── skill.yaml
│       ├── instructions.md
│       ├── scripts/
│       ├── references/
│       └── tests/
├── rules/
│   └── project/
│       ├── rule.yaml
│       ├── rules.md
│       └── tests/
├── memory/
│   ├── facts/
│   ├── preferences/
│   └── failures/
├── evals/
│   ├── datasets/
│   ├── graders/
│   └── suites/
└── .singularity/
    ├── repo.git
    ├── worktrees/
    ├── runs/
    ├── cache/
    └── installs/
```

`.singularity/repo.git` 是独立 bare repository，不修改、不提交、不重置用户业务 Git。

## 7. CLI 命令体系

```text
sge
├── init
├── import <path>
├── status
├── scan [target]
├── evolve [target]
├── test [target]
├── apply [target]
├── explain [evolve-id]
├── history [target]
├── diff [from] [to]
├── undo [revision]
├── branch <name>
├── memory add|propose|list|show|diff|remove
├── pack [target]
├── export [target]
├── link [target]
├── hosts
├── config
└── doctor
```

统一目标选择器：

```bash
sge evolve
sge evolve skill:code-review
sge evolve rule:project
sge test skill:code-review
sge apply skill:code-review --to claude
```

主路径：

```bash
npx singularity-engine init
sge scan
sge evolve skill:code-review
sge test skill:code-review
sge apply --to claude
```

命令约束：

- `sge evolve` 未指定方向时，先执行诊断并展示预选方案；
- `sge test` 运行基线、候选和回归评测；
- `sge apply` 只能安装满足质量门禁的候选；
- `sge undo` 可以撤销进化或撤销宿主安装；
- `sge link` 是显式高级模式，链接生成目录而非标准源。

## 8. 进化方向

### 8.1 用户指定方向

```bash
sge evolve --toward reliability
sge evolve skill:code-review \
  --goal "SQL 注入召回率达到 95%，误报率不超过 8%"
```

目标被编译为不可变 Evolution Contract：

```yaml
objective:
  primary: sql_injection_recall
  target: ">= 0.95"
constraints:
  false_positive_rate: "<= 0.08"
  token_cost_change: "<= 0.10"
budget:
  generations: 3
  candidates_per_generation: 5
  max_cost_usd: 5
```

### 8.2 用户未指定方向

系统只执行分析，不立即变异：

1. 展示证据来源和覆盖范围；
2. 给出 2–5 个候选方向；
3. 展示预期收益区间、置信度、风险、成本、影响资产和评测方法；
4. 允许用户选择、组合、修改或拒绝；
5. 生成 Contract 并再次确认。

执行前不得展示没有证据支撑的伪精确收益。

## 9. 进化状态机

```text
Contract
  → Baseline
  → Diagnose
  → Propose
  → Approve
  → Mutate
  → Sandbox
  → Evaluate
  → Select
  → Review
  → Apply | Continue | Abort
```

每一步写入 Journal。进程中断后，用户可以查看、继续或清理，不允许留下不可识别的半完成状态。

## 10. 变异算子

完整产品包含 14 个算子：

| 类别 | 算子 | 风险 | 强制约束 |
|---|---|---:|---|
| Prompt | `prompt_mutation` | 低 | 不得删除安全约束 |
| Prompt | `skill_prompt_mutation` | 低 | 只修改目标 Skill |
| 工具 | `tool_selection` | 中 | 新增权限必须确认 |
| 工具 | `skill_tool_mutation` | 中 | 不自动安装系统依赖 |
| 记忆 | `memory_schema` | 中 | 必须提供迁移和回滚 |
| 记忆 | `memory_retention_policy` | 低 | 不得静默删除记忆 |
| 结构 | `skill_split` | 高 | 验证调用方兼容性 |
| 结构 | `skill_merge` | 高 | 检测职责与触发冲突 |
| 策略 | `planning_policy` | 中 | 只调整可观察规划流程 |
| 策略 | `verification_policy` | 低 | 不得通过放宽断言提分 |
| 推理 | `reasoning_depth` | 中 | 调整预算和步骤，不依赖隐藏 CoT |
| 实现 | `tool_implementation` | 高 | 隔离分支和强制人工审查 |
| 上下文 | `context_window_strategy` | 中 | 调整分块、摘要和预算 |
| 失败学习 | `failure_pattern_adaptation` | 低 | 必须引用失败证据 |

成熟度：

- Stable：可以自动生成候选并执行评测；
- Preview：默认逐步确认；
- Experimental：默认只允许 Dry Run 或显式开启。

V1.0 中低中风险核心算子必须达到 Stable。高风险算子至少达到 Preview，并具备完整沙箱、评测、人工门禁和回滚。

## 11. 评测与停止条件

### 11.1 多维评测

```yaml
metrics:
  task_success: 0.91
  safety: 1.00
  latency_p95_ms: 3200
  token_cost: 14800
  stability: 0.94
  compatibility: 1.00
```

候选选择规则：

1. 硬门禁全部通过；
2. 主目标达到 Contract 阈值；
3. 受保护指标没有超出允许退化范围；
4. 重复运行达到指定次数；
5. 报告样本量、均值、波动和环境；
6. 无统计意义的变化不能声称提升。

LLM Judge 只作为辅助评分。优先使用确定性断言、结构校验、静态分析、真实任务结果和项目原生测试。

### 11.2 停止条件

满足任一条件即停止：

- 达到目标且通过稳定性验证；
- 连续两代没有实质提升；
- 成本、时间或代数预算耗尽；
- 安全或兼容门禁失败；
- 评测数据不足；
- 用户主动终止。

停止后提供：

```text
Apply / Export / Keep branch / Continue / Discard
```

## 12. 记忆系统

记忆分为 Facts、Preferences、Failures：

```yaml
schema: sge.dev/memory/v1
id: failure.sql-injection-like
type: failure
statement: LIKE 查询中的通配符必须参数化处理
scope: skill:code-review
source:
  kind: eval
  ref: run-20260813/case-17
confidence: 0.96
status: confirmed
created_at: 2026-08-13T14:32:01Z
expires_at: null
evidence:
  - sha256:...
```

治理规则：

- Fact 必须有来源，模型推断先进入 `proposed`；
- Preference 必须来自用户明确表达；
- Failure 必须关联失败案例、复现或人工确认；
- 作用域支持 workspace、agent、skill、rule；
- 不把全部记忆无差别注入上下文；
- 删除、合并和过期均为显式变更；
- 本地向量索引只能作为可删除缓存，YAML 是事实源；
- V1 不静默扫描宿主历史；
- 记忆由用户手动添加，或由宿主提出候选后经确认写入。

## 13. 谱系与证据

每个候选对应内部 branch/worktree。提交元数据包含父代、算子、Contract、指标摘要和证据哈希。

```text
.singularity/runs/<run-id>/
├── contract.yaml
├── baseline.json
├── proposals.json
├── candidates/
├── evaluations/
├── decision.md
├── mutation.patch
├── install-preview/
└── replay.yaml
```

文本资产、记忆和决策说明进入内部 Git。大型日志进入内容寻址存储，Git 保存哈希引用。

`decision.md` 只陈述可追溯证据，包括触发原因、分析范围、候选比较、最终选择、Diff、实际结果、剩余风险和回放方式。

## 14. 宿主适配

正式支持：

- Claude Code
- Codex
- TRAE
- OpenCode
- OpenClaw

适配器契约：

```text
detect → capabilities → preview → render → validate → apply/rollback
```

适配器必须声明：

```yaml
host: claude
supports:
  agent: native | mapped | unsupported
  skill: native | mapped | unsupported
  rule: native | mapped | unsupported
```

规则：

- 原生支持时生成原生格式；
- 映射时必须说明语义损失；
- 无法可靠表达时拒绝安装；
- 宿主格式变化由适配器版本处理；
- 标准源不包含宿主生成内容。

### 14.1 事务式安装

```bash
sge apply skill:code-review --to current
```

执行流程：

1. 检测宿主和版本；
2. 生成临时输出；
3. 校验格式、引用、权限和冲突；
4. 展示 Diff 与语义损失；
5. 用户确认；
6. 备份目标文件；
7. 原子写入；
8. 记录 install manifest；
9. 执行 smoke test；
10. 失败时恢复备份。

## 15. 模型与隐私

运行模式：

| 模式 | 智能来源 | 数据位置 |
|---|---|---|
| 独立 CLI | 用户配置的远程 API 或本地模型 | 本地 |
| 宿主 Skill | 当前宿主模型 | 本地 |
| 严格离线 | Ollama 或兼容本地模型 | 本地 |

安全规则：

- API Key 只从系统 Keychain、环境变量或外部凭证命令读取；
- 凭证不写入 YAML、日志、证据或上下文；
- 远程调用前生成 Data Manifest；
- 默认排除 `.env`、密钥、证书、Git 凭证和敏感路径；
- 支持 `network: deny/local/remote`；
- 模型输出必须经过 Schema、路径、权限和内容校验。

“本地数据优先”表示所有持久化数据默认在本机，不表示远程模型调用永远禁止。

## 16. 分层沙箱

| 后端 | 用途 | 默认权限 |
|---|---|---|
| Content | Prompt、Rule、Memory 变异 | 不执行代码 |
| WASI | 内置评测器、可移植脚本 | 无网络、限定目录 |
| Process | 项目原生测试 | 临时工作树、资源限制、路径白名单 |
| Container | 高风险实现变异 | 网络默认关闭 |

风险分级：

- 低风险：Prompt、输出格式、验证策略；
- 中风险：工具选择、记忆 Schema、上下文策略；
- 高风险：Skill 拆分/合并、工具实现、权限扩张。

中高风险变异必须展示影响面。高风险变异强制使用独立分支、最强可用沙箱和人工审查，禁止自动安装。

## 17. 工程架构

```text
singularity-engine/
├── crates/
│   ├── sge-cli
│   ├── sge-domain
│   ├── sge-protocol
│   ├── sge-store
│   ├── sge-evolution
│   ├── sge-eval
│   ├── sge-sandbox
│   ├── sge-provider
│   ├── sge-adapter
│   └── sge-security
├── adapters/
│   ├── claude
│   ├── codex
│   ├── trae
│   ├── opencode
│   └── openclaw
├── skill/
├── schemas/
├── fixtures/
└── packages/npm/
```

技术选型：

| 层级 | 技术 |
|---|---|
| 核心与 CLI | Rust、clap、tokio |
| 序列化与 Schema | serde、serde_yaml、schemars |
| 内部 Git | git2/libgit2 |
| 派生索引 | SQLite |
| HTTP Provider | reqwest |
| WASI | wasmtime |
| 日志 | tracing + 本地 JSON |
| 分发 | npm bootstrapper + Cargo |

YAML/Markdown 是用户可读事实源；SQLite 仅保存可重建索引和查询数据。

## 18. 测试与质量门禁

| 层级 | 验证内容 |
|---|---|
| 单元测试 | 状态机、算子、评分、路径和权限 |
| Schema 测试 | 协议兼容、迁移、未知字段 |
| Property Test | Diff、回滚、序列化、预算不变量 |
| Golden Test | 五宿主输出格式与语义映射 |
| Integration | Provider、Git、沙箱、Journal |
| E2E | 导入、进化、证明、安装、撤销 |
| Fault Injection | 超时、断电、磁盘满、模型失败、测试崩溃 |
| Security | Prompt 注入、路径穿越、命令注入、密钥泄漏 |
| Live Compatibility | 定期验证真实宿主和模型 API |

CI 不依赖实时 LLM。普通 CI 使用固定响应、录制样本和确定性评测。真实模型测试作为受控夜间任务，不直接阻塞普通贡献。

## 19. V1.0 发布策略

项目取消 MVP 概念。首个公开版本为 V1.0。

### 19.1 交付阶段

| 阶段 | 预计周期 | 交付内容 | 状态 |
|---|---:|---|---|
| P0 协议冻结 | 4–6 周 | Artifact、Contract、Evidence、Adapter、Memory 协议 | 不发布 |
| P1 垂直闭环 | 6–8 周 | 三类资产基础链路、内部 Git、评测、回滚、单宿主 | Internal |
| P2 能力全景 | 8–10 周 | 14 算子、五宿主、统一 Skill、完整命令面 | Tech Preview |
| P3 强化验证 | 8–12 周 | 沙箱、安全、恢复、兼容矩阵、黄金评测集 | Private Beta |
| P4 发布收口 | 4–6 周 | 文档、安装器、迁移、性能、RC 修复 | RC |
| V1.0 | 总计约 8–10 个月 | 首个公开稳定版本 | Public Stable |

周期是基于 4 人核心团队的估算，不是固定承诺。

### 19.2 V1.0 必须包含

- Agent、Skill、Rule 三类资产；
- 14 个变异算子；
- 五宿主适配器；
- CLI 和统一源 Skill；
- 本地数据、内部 Git、记忆和证据；
- 指定方向与预选方向；
- 多维评测、停止条件、解释和回滚；
- 四级执行后端；
- 安装、导出、链接和撤销；
- macOS、Linux、Windows 分发。

### 19.3 V1.0 发布门禁

- Stable 算子有黄金评测集、回归测试和失败注入测试；
- 高风险 Preview 算子具备沙箱、评测、门禁和回滚；
- 五宿主有版本兼容矩阵；
- 中断、模型超时、测试失败不会破坏工作区；
- 高风险操作不能绕过人工门禁；
- 大型日志具备保留和清理策略；
- 协议具备向前迁移能力；
- 所有公开演示均可通过 Replay 本地复现。

## 20. 后续迭代

### V1.1：体验与诊断

- Dry Run、分步确认和恢复中断任务；
- Skill 依赖图与冲突解释；
- 宿主提交上下文生成记忆候选；
- Eval 模板库和项目类型识别。

### V1.5：高级进化

- 多目标 Pareto 选择；
- 自动建议新 Skill；
- 负向进化保护；
- 上下文与成本优化；
- 算子插件 SDK。

### V2：本地实验室

- 本地 Arena；
- Agent/Skill 交叉组合；
- 本地批量实验调度；
- 谱系可视化报告；
- 可选 Tauri 桌面壳，CLI 仍为核心。

### V3：去中心化生态

- Git Source 与本地 Skill Catalog；
- 签名资产包和来源证明；
- 证据包导入、复现和验证；
- PR 式进化评审；
- 组织内私有 Git 资产源。

## 21. 品牌与体验

正式品牌：`SINGULARITY ENGINE by ZJB.DEV`

命令保持工程直白。科幻感放在信息架构和报告标题中：

| 能力 | 视觉表达 |
|---|---|
| 诊断 | Signal Map |
| 候选生成 | Mutation Chamber |
| 多维评测 | Proof Matrix |
| 历史谱系 | Lineage |
| 记忆 | Memory Strata |
| 安装预览 | Graft Preview |
| 回滚 | Temporal Rewind |

视觉约束：

- 深空黑灰底、冷紫主色、少量青色强调；
- 科幻元素必须映射真实状态和证据；
- 支持 `NO_COLOR`、纯文本和无动画；
- 不使用无意义的加载动画或虚构指标。

## 22. 首发传播

### 22.1 90 秒核心演示

```bash
sge import ./broken-code-review-skill
sge scan
sge evolve skill:code-review
sge test skill:code-review
sge explain
sge apply --to claude
sge apply --to codex
```

演示必须展示：

1. 原 Skill 稳定漏报真实漏洞；
2. 系统基于失败证据给出多个方向；
3. 用户选择目标、预算和约束；
4. 候选在同一任务集竞争；
5. 胜者通过回归和安全门禁；
6. 标准资产安装到两个宿主；
7. 第三方可以下载证据并 Replay。

### 22.2 社区资产

- 一个可复现的 before/evolve/prove/apply 仓库；
- 代码审查 Skill、测试生成 Agent、项目规则集三个种子资产；
- Threat Model 和“我们不声称什么”页面；
- Evidence Bundle 与 Replay 命令；
- Adapter 开发指南和模板；
- “Evolved with SGE” Badge；
- Git-based Skill Pack 示例。

### 22.3 发布节奏

- T-30：协议草案、威胁模型和技术文章；
- T-14：可复现 Demo 与适配器预览；
- T-7：邀请开发者提交真实失败 Skill；
- Launch Day：仓库、安装包、视频和证据同步发布；
- T+7：公布复现结果、失败案例与修复；
- T+30：发布 V1.1 路线和社区适配器。

## 23. 开源与商业边界

- 核心、协议、CLI、五宿主适配器和统一 Skill 使用 Apache-2.0；
- 品牌名和视觉标识保留商标权；
- 后续商业化围绕企业支持、定制适配、安全审计和私有资产治理；
- 本地核心能力不转为闭源。

## 24. 主要风险与应对

| 风险 | 应对 |
|---|---|
| “变强”不可证明 | Contract、用户任务集、多维指标、Replay |
| 14 算子质量参差 | 成熟度分级和独立发布门禁 |
| 宿主格式频繁变化 | 版本探测、黄金样本、能力矩阵 |
| LLM Judge 偏差 | 只作辅助，优先确定性证据 |
| 沙箱能力被高估 | 明确后端边界，不做过度安全承诺 |
| 记忆污染 | 来源、状态、作用域、过期和确认 |
| 远程模型泄露数据 | Data Manifest、默认排除和脱敏 |
| V1 周期失控 | 协议冻结、阶段门禁、子系统独立验收 |
| Git 与工作区损坏 | 独立 repo、隔离 worktree、事务写入 |

## 25. 最终决策记录

1. 产品形态为 CLI + Skill，不提供 Web UI。
2. CLI 前缀为 `sge`，子命令使用工程直白型命名。
3. 产品不包含完整 Agent Chat Runtime。
4. 独立模式允许可选模型 API；宿主模式复用宿主模型。
5. 数据持久化本地优先，不等同于永不联网。
6. Skill 使用统一源并由适配器自动转换。
7. 安装结果必须预览并经人工确认。
8. 进化谱系使用独立内部 Git，不污染业务 Git。
9. 记忆由手动输入或宿主提出后确认写入。
10. V1 覆盖 Agent、Skill、Rule、14 个算子和五宿主。
11. 取消 MVP，首个公开版本为完整 V1.0。
12. 核心采用 Apache-2.0，品牌权利保留。

