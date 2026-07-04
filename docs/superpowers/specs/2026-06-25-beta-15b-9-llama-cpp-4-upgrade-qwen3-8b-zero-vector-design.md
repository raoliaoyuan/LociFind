# BETA-15B-9：llama-cpp-4 升级 / qwen3-embedding-8b 全零向量 bug 排查设计 spec

> 承接 BETA-15B-7 v4 cycle 双 Branch IV-infra 诊断中 model-runtime 子项的另一半（BETA-15B-8 解了 bge-m3 pooling type detection 问题）。本 cycle 修 Branch IV-B = qwen3-embedding-8b 全零向量 bug（推断 llama-cpp-4 0.3.0 + 8B 推理路径 bug）。
>
> **执行哲学**：按 v4 baseline 报告 hypothesis 排序 systematic 排查、early-exit 任一 win、与 BETA-15B-8 数据收集 cycle 同款节奏（infra 修复 + 拿真水位 + 留 bake 决策给 follow-up cycle BETA-15B-7-v2）。
>
> **范围哲学**：纯 llama-cpp-4 版本升级 + qwen3-8b 端到端验证。生产 wiring / `DEFAULT_EMBEDDING_MODEL_PATH` / `baseline.json` / `gate.rs` / desktop wiring **全部不动**。bake 决策推到 follow-up cycle。
>
> **与 BETA-15B-7-v2 / BETA-15B-8 的关系**：BETA-15B-7 v4 cycle 在 baseline 报告 line 707-712 / 751-752 登记三独立下 cycle 抓手——本 cycle 是 placeholder **BETA-15B-Y 的具体化（ID 落定为 BETA-15B-9）**。BETA-15B-7-v2（bake 决策）依赖本 cycle + BETA-15B-8 完成；BETA-15B-7-v2 不在本 cycle 范围。

## 1. 背景与动机

### 1.1 v4 cycle 暴露的 Branch IV-B（qwen3-8b 全零）

BETA-15B-7 v4 cycle 用「评测纯探针」路线测两条独立的「更大 / 更强 embedding 模型」轴。qwen3-embedding-8b（同族最大档轴）实测全零向量、202 个序列每条全零、L2 norm = 0、总推理时间 ~27s（vs 估算 13-40 分钟、提示早期短路）。

v4 baseline 报告（line 685-693）列出 4 hypothesis 排序（按可能性）：

1. **llama-cpp-4 0.3.0 对 qwen3-8b 4096-dim embedding tensor 处理 bug**（最高优、未证）
2. **GPU layer offload 配置问题**：`gpu_layers=99` 可能与 8b 36 层有边界 case
3. **`context_size=2048` 对 8b 边界不够**：0.6b 工作良好不代表 8b 同 context 工作
4. **8b 特殊层结构未支持**：Gated Delta Net / SWA 等（最低可能、GGUF metadata 未直接看到、纯推测）

**v4 cycle 主动放弃 8b 模型能力结论**：「**不能下『qwen3-8b 模型差』结论**——8b 根本没被有效推理过」（baseline 报告 line 697）。

### 1.2 本 cycle 的命题

> **按 v4 hypothesis 排序 systematic 排查 qwen3-8b 全零根因、拿真水位、解 bake 决策的「同族最大档真水位未知」盲点。**

修复后 cycle 产出：

1. **infra 层闭环**：升 llama-cpp-4 0.3.0 → 0.3.2（latest patch、SemVer 兼容、bundled llama.cpp upstream commit 紧）
2. **诊断完整性**：4 hypothesis 全验证、任一 win 即拿真水位、若全 fail 则 file upstream issue 留 trace
3. **bake 决策依据**：qwen3-8b vs bge-m3 vs qwen3-0.6b 三方对照、推荐 bake 候选给 BETA-15B-7-v2

### 1.3 为什么本 cycle 优先于「直接 bake bge-m3」

bge-m3 vs qwen3-0.6b 在 BETA-15B-8 后是真二维 trade-off（OVERALL +0.013 vs crosslang -0.032、LociFind 头号卖点反退）、不是纯 win。**qwen3-embedding-8b 真水位未知**——若 8b 在本 cycle 拿出 OVERALL > bge-m3 且 crosslang 守住 0.717、那是**无 trade-off 的更好 bake 候选**（同族缩放天然 crosslang 强、Qwen3 系列多语言训练更充分）。

本 cycle 是 bake 决策的**前置数据收集**、避免「带着信息空白拍 bake」。

**零回归约束**：qwen3-0.6b 重 embed 后 `vectors.json` 语义等价（接受 SemVer-compatible 上游 Metal kernel 数值微调）、不要求 byte-equal。原计划红线为字节不动、Task 1 实测发现 llama-cpp-4 0.3.2 升级带来 ~1e-4 量级数值微调（cosine ≥ 0.9999、max abs ≤ 1e-3），coordinator 拍方向 1 放宽红线为语义等价闸。详见 §2.2 (7) + (8)。

## 2. 目标与验收

### 2.1 目标

- `cargo update -p llama-cpp-4 --precise 0.3.2`（patch bump、SemVer 兼容、Cargo.lock 必变）
- workspace test / clippy / fmt 三件套验证升级后零回归
- **qwen3-0.6b 重 embed 后 `vectors.json` 语义等价**（cosine similarity ≥ 0.9999 + max abs ≤ 1e-3、接受 SemVer-compatible 上游 Metal kernel 数值微调；BETA-15B-9 Task 1 实测 cos min=0.999999 / mean=1.000000 / max abs=2.5125e-04 全过闸）
- 按 v4 hypothesis 排序排查 qwen3-8b 全零（早 exit 策略）：
  1. 升 0.3.2 后重 embed qwen3-8b → 健康性验
  2. 若 fail：临时改 `gpu_layers=0` CPU 跑 1 条样本 → 健康性验
  3. 若 fail：临时改 `context_size=4096` 跑 1 条样本 → 健康性验
  4. 若全 fail：file upstream issue（含复现脚本、模型 metadata、Mac 环境、本机 attempt 摘要）
- 任一 hypothesis win 后：跑 9 阈值 sweep × qwen3-8b 拿真水位
- vectors-qwen3-8b.json（win 路径覆盖入仓 / fail 路径不动）
- baseline 报告追加 v4-fixup2 节：含 4 hypothesis 排查路径 + 三方对照表（qwen3-0.6b vs bge-m3 vs qwen3-8b、若 win）+ bake 推荐意见 + 下 cycle 抓手优先级

### 2.2 验收红线（不可回归）

(1) 全工程 `cargo test --workspace` 0 failed（升 version 后、不允许引入回归）
(2) `cargo clippy --workspace --all-targets -- -D warnings` 0 warning
(3) `cargo clippy -p locifind-model-runtime --features llama-cpp --all-targets -- -D warnings` 0 warning
(4) `cargo fmt --all --check` 净
(5) `cargo test -p locifind-evals --test semantic_quality_gate` 1 passed（baseline.json 不动、本 cycle 不改）
(6) **evals parser-only byte-equal 不变**：v0.5=473 / v0.9=877（本 cycle 不动 parser / coverage）
(7) **`packages/evals/fixtures/semantic-recall/vectors.json` 语义等价**（qwen3-0.6b 重 embed 后与升级前 cosine similarity ≥ 0.9999、max abs diff ≤ 1e-3、向量 schema / dim / 个数完全一致；接受 SemVer-compatible 上游 llama.cpp Metal kernel 数值微调；若 cos < 0.9999 或 max abs > 1e-3 = 升级引入了非预期的大幅副作用、Branch IV 异常、阻止合并）。本 cycle Task 1 实测：cosine min=0.999999 / mean=1.000000 / max abs=2.5125e-04、均满足语义等价闸。
(8) `vectors-qwen3-0.6b.json` 与 (7) 同步更新（v4 cycle 入仓的 0.6b copy、本 cycle 一并重 embed 保持与 vectors.json 同款数值、两文件 SHA256 一致）
(9) `vectors-bge-m3.json` 字节不动（BETA-15B-8 入仓的 CLS pooling 版、本 cycle 不动）
(10) `vectors-qwen3-8b.json`：win 路径必变（v4 全零版被新非零版覆盖、SHA256 必不同）；fail 路径必不动（仍是 v4 全零版、cycle 间状态一致）
(11) `Cargo.lock` 必变（llama-cpp-4 0.3.0 → 0.3.2 + 依赖传递 llama-cpp-sys-4 0.3.0 → 0.3.2）
(12) Mac Metal 真用确认（重 embed 时 log 见 MTL0 layer offload、非 CPU fallback）

### 2.3 GO / 异常判定

| 情景 | 解读 | 行动 |
|---|---|---|
| **GO（任一 hypothesis win）** | qwen3-8b 真水位拿出、可作三方对照 + bake 决策依据 | 合并、baseline v4-fixup2 节落、推荐 bake 候选给 BETA-15B-7-v2、vectors-qwen3-8b.json 覆盖入仓 |
| **GO（4 hypothesis 全 fail、file upstream issue）** | qwen3-8b 在 llama-cpp-4 0.3.2 + 任一参数组合下仍全零 = 确定上游 bug | 合并、cycle 末报告完整排查路径 + upstream issue 链接、bake 决策只能走 bge-m3（或等 upstream fix）、vectors-qwen3-8b.json 不动 |
| **异常** | 升 0.3.2 后任一现有 vectors 文件 SHA256 改变 / workspace test fail / clippy fail | **不合并**：升级引入未预期副作用、调查根因（最可能：llama-cpp-4 0.3.2 行为不向下兼容 qwen3-0.6b / bge-m3）、必要时 revert 升级、考虑只升 0.3.1 中间档 |

**注**：本 cycle 不强求拿出 qwen3-8b 真水位才合并。4 hypothesis 全 fail + file upstream issue 也是合规的 cycle 收口（infra 调查穷尽 + 留追溯路径、bake 决策仍可走 bge-m3）。

## 3. 范围

### 3.1 In-scope

- `cargo update -p llama-cpp-4 --precise 0.3.2`（含依赖传递）
- workspace test / clippy / fmt 验证升级后零回归
- qwen3-0.6b 重 embed → vectors.json 语义等价验证（cosine ≥ 0.9999 + max abs ≤ 1e-3、升级回归红线）+ vectors-qwen3-0.6b.json 同步入仓
- qwen3-8b embed × 1-3 次（按 hypothesis 走、early-exit）
- 临时改 `ModelLoadParams.gpu_layers=0`（test-only / 不入仓、hypothesis 2）
- 临时改 `context_size=4096`（test-only / 不入仓、hypothesis 3）
- 任一 win 后：9 阈值 sweep × qwen3-8b（与 BETA-15B-8 同款 sweep 流程）
- vectors-qwen3-8b.json 覆盖入仓（win 路径）
- file upstream issue（4 hypothesis 全 fail 路径）
- baseline 报告追加 v4-fixup2 节
- STATUS / ROADMAP doc-sync BETA-15B-9 done

### 3.2 Out-of-scope（主动 YAGNI）

- ❌ **不动 `DEFAULT_EMBEDDING_MODEL_PATH`**：bake 决策留 BETA-15B-7-v2 独立 cycle
- ❌ **不动 `baseline.json` / `gate.rs` 红线**：生产仍走 qwen3-0.6b、v3 锚不破
- ❌ **不重跑 bge-m3、不动 bge-m3 vectors**：BETA-15B-8 已落地、`vectors-bge-m3.json` 字节不动。qwen3-0.6b 在 Task 1 Phase 0 因 llama-cpp-4 升级被动重 embed 通过语义等价闸（cos ≥ 0.9999 + max abs ≤ 1e-3）、`vectors.json` + `vectors-qwen3-0.6b.json` SHA256 一并变到 `0315b8d0...`（详 §2.2 (7)+(8)）。本 cycle 不主动重跑 qwen3-0.6b 评测、仅 cargo run --embed 一次取 0.3.2 升级后语义等价基线。
- ❌ **不补 qwen3-embedding-4b 中间档**：v4 cycle 主动 YAGNI 排除、bake 决策不需要、留 BETA-15B-7-v2
- ❌ **不升 llama-cpp-4 0.3.x → 0.4.x**：patch bump 内 SemVer 兼容、major bump 风险大、超本 cycle 范围
- ❌ **不动 desktop UI / Tauri wiring**：生产 qwen3-0.6b 走原路径
- ❌ **不固化 hypothesis 2/3 的 test-only 改动**：`gpu_layers=0` / `context_size=4096` 是排查时临时改动（local diff）、不入仓
- ❌ **不重写 BETA-15B-8 v4-fixup 节**：本 cycle 追加 v4-fixup2 与 v4-fixup 累加并存

## 4. 设计

### 4.1 执行流水（早 exit）

```
Phase 0 — 升 version + 升级回归保护
  cargo update -p llama-cpp-4 --precise 0.3.2
  cargo build -p locifind-model-runtime --features llama-cpp
  cargo test --workspace
  cargo clippy ... -- -D warnings (workspace + model-runtime --features llama-cpp)
  cargo fmt --all --check
  cp vectors.json /tmp/baseline.json
  cargo run --bin semantic_quality --features metal -- --embed --model models/qwen3-embedding-0.6b-q8_0.gguf
  python3 cosine-check.py /tmp/baseline.json vectors.json   # 必须 cosine ≥ 0.9999 + max abs ≤ 1e-3
  cp vectors.json vectors-qwen3-0.6b.json                    # 同步 0.6b 双文件

Phase 1 — hypothesis 1: 升 version 是否直接修 8b
  cargo run --bin semantic_quality --features metal -- \
    --embed --model models/qwen3-embedding-8b-q8_0.gguf \
    --vectors-file vectors-qwen3-8b.json
  python3 health-check.py vectors-qwen3-8b.json
  ↓ win (L2≈1, nonzero≥95%): goto §4.2 sweep
  ↓ fail (L2=0, nonzero=0): goto Phase 2

Phase 2 — hypothesis 2: GPU layers=0 CPU 跑
  临时改 LlamaLoader::load 中 ModelLoadParams.gpu_layers=0
  （或 evals binary 加临时 cli flag、最小侵入）
  embed × 1 条样本 (8b)
  ↓ win: 推断 GPU offload bug、记录 + 还原 gpu_layers=99 + goto Phase 1 sweep（用 99 跑）
  ↓ fail: goto Phase 3

Phase 3 — hypothesis 3: context_size=4096
  临时改 LlamaLoader::load 中 context_size=4096
  embed × 1 条样本 (8b)
  ↓ win: 推断 context size 边界 bug、记录 + 还原 + goto Phase 1 sweep（用 2048 跑）
  ↓ fail: goto Phase 4

Phase 4 — 4 hypothesis 全 fail
  file upstream issue (https://github.com/eugenehp/llama-cpp-rs/issues/new)
    含：复现脚本 / Mac Metal 环境 / GGUF metadata dump / 已 attempt 摘要
  collect upstream issue URL
  goto Phase 5（无 sweep、vectors-qwen3-8b.json 不动）

Phase 5 — cycle 收口（无论 win/fail）
  baseline 报告追加 v4-fixup2 节
  STATUS / ROADMAP doc-sync BETA-15B-9 done
  PR + 合 main
```

### 4.2 9 阈值 sweep（win 路径、与 BETA-15B-8 同款）

```bash
mkdir -p /tmp/beta-15b-9-sweep
for t in 0.0 0.30 0.45 0.60 0.70 0.80 0.90 0.99 1.01; do
  cargo run --bin semantic_quality --features metal -- \
    --vectors-file vectors-qwen3-8b.json \
    --semantic-weight 10.0 --cosine-threshold $t \
    | tee -a /tmp/beta-15b-9-sweep/qwen3-8b.log
done
```

数据提取：6 桶 nDCG（exact-name / synonym / concept / crosslang / content-not-name / OVERALL）× 9 阈值、控制对照 T=0.0 HYBR≈VEC + T=1.01 HYBR≡HYB。

### 4.3 baseline 报告 v4-fixup2 节模板（含 win + fail 两路径变体）

**节标题**：`### v4-fixup2 数据集节 — llama-cpp-4 升级 + qwen3-embedding-8b 真水位（BETA-15B-9）`

**节内容固定块**：
- 4 hypothesis 排查路径（每个 phase win/fail 结论）
- 升级影响（Cargo.lock diff、workspace test 数）
- vectors.json 语义等价验证（cosine + max abs、升级回归保护）

**win 路径补充**：
- qwen3-8b 9 阈值 sweep 表
- 三方对照表（qwen3-0.6b vs bge-m3 vs qwen3-8b、对 best OVERALL / crosslang / content-not-name）
- bake 推荐意见（按数据走、推荐 OVERALL+crosslang 二维都好的）

**fail 路径补充**：
- file 的 upstream issue 链接 + 标题 + 关键复现细节
- 「bake 决策只能走 bge-m3 / qwen3-0.6b」结论 + 等 upstream fix 才能再评 qwen3-8b
- vectors-qwen3-8b.json 不动说明（仍是 v4 全零版、cycle 间状态一致）

**通用收尾**：
- 下 cycle 抓手优先级（基于实际数据走、不预设）
- 链接 spec / plan / pooling.rs / 4 hypothesis 排查产物

## 5. 测试策略

### 5.1 单测

无新增（本 cycle 无新代码、仅升 version + Phase 2/3 临时 test-only 改动）。

### 5.2 集成手验（cycle 内按 Phase 走）

- Phase 0：cargo test/clippy/fmt 三件套 + qwen3-0.6b 语义等价红线（cosine ≥ 0.9999 + max abs ≤ 1e-3）
- Phase 1-3：每 Phase 后 Python health-check.py 跑 8b vectors 抽样
- Phase 4：file upstream issue（GitHub 浏览器手动 / gh CLI）
- Phase 5（win）：9 阈值 sweep 数据收集 + extracted.txt

### 5.3 Health-check 脚本

与 BETA-15B-8 Task 3 同款 Python 脚本、复用即可：

```python
# 抽样 vectors-qwen3-8b.json 前 3 doc + 3 query
# 期望（win）：dim=4096、nonzero ≥ 95%、L2 norm = 1.0000 ± 0.001
# 失败（v4 实测）：dim=4096、nonzero = 0、L2 norm = 0
```

## 6. 风险与缓解

| 风险 | 概率 | 影响 | 缓解 |
|---|---|---|---|
| 升 0.3.2 后 qwen3-0.6b vectors.json 语义不等价（cos < 0.9999 或 max abs > 1e-3）| 低 | 高（验收 (7) 红线、阻止合并）| Phase 0 计算 cosine + max abs、若不过闸则 BLOCKED 报 coordinator；本 cycle Task 1 实测 cos min=0.999999 / max abs=2.5e-04 全过闸（升级带来 ~1e-4 量级 Metal kernel 微调、SemVer-compatible 可接受、补算 cosine 验证后入仓） |
| 升 0.3.2 后 workspace test fail | 低 | 高 | Phase 0 三件套先验、不通过则 revert + 调查 |
| llama-cpp-4 0.3.2 API 微调（虽 SemVer 兼容也可能 minor break）| 极低 | 中 | cargo build 立即暴露、若 break 则 revert 0.3.1 试 |
| 4 hypothesis 全 fail + upstream issue 半年没回应 | 中 | 低 | bake 决策走 bge-m3、qwen3-8b 暂搁、文档明示「等 upstream fix」、follow-up cycle 监控 issue |
| Phase 2/3 test-only 改动忘还原入仓 | 中 | 中 | Phase 5 前 `git status` 验 + `git diff` review 确认仅 vectors-qwen3-8b.json + baseline 报告 + STATUS/ROADMAP 改动 |
| qwen3-8b 推理巨慢（修后单条 ~30s × 202 序列 ≈ 1.5h）| 高 | 低 | 接受、Mac Metal M5 Pro 实测、sweep 9 阈值复用同 vectors 文件、不重 embed |

## 7. 真机手测剧本

本 cycle 是 model-runtime infra 升级 + 数据收集、**桌面用户行为零变化**（生产 wiring 不动 / DEFAULT_EMBEDDING_MODEL_PATH 不动 / baseline.json 不动）。按 superpowers `verification-before-completion`：

- Phase 0 workspace test + qwen3-0.6b vectors.json 语义等价（cosine ≥ 0.9999 + max abs ≤ 1e-3）已构成 infra 升级回归证据链
- 桌面 UI / 索引 / 搜索行为路径未触达
- **不安排额外桌面真机手测剧本**

例外：若用户希望验证「桌面侧加载 qwen3-0.6b 升 0.3.2 后行为字节等价」、可走一次 BETA-15B-2 暖机剧本（[docs/manual-test-scenarios.md](../../manual-test-scenarios.md) 对应节）、对比首查询时延 + semantic 命中行为。

## 8. 节奏与回看

参照 BETA-15B-8 cycle 节奏（同款数据收集 cycle）：

| 阶段 | 内容 | 工具 / 工序 |
|---|---|---|
| brainstorming | Q1-Q2 + design sections 7 节 + user ack | 本 cycle 已完成 |
| writing-plans | task 分解（预估 3-5 task、视早 exit 路径）| superpowers writing-plans |
| subagent-driven | 每 task implementer + 双 reviewer（spec / code-quality）| superpowers subagent-driven-development |
| final integration review | general-purpose subagent 通读 main..HEAD diff | 集成审 |
| 集成手验 | Phase 0-5 流水 | 手动 |
| PR + 收口 | 合 main + STATUS / ROADMAP doc-sync | 本仓 |

**预估 cycle 时长**：0.5-1d（取决于 hypothesis 哪步 win + 是否需 file upstream issue）。

## 9. 链接

- 上 cycle 落点：[BETA-15B-7 v4 spec](2026-06-24-beta-15b-7-embedding-model-scaling-probe-design.md) + [BETA-15B-8 spec](2026-06-25-beta-15b-8-model-runtime-pooling-type-detection-design.md)
- v4 cycle qwen3-8b 全零诊断：[baseline 报告 v4 节](../../reviews/semantic-recall-quality-baseline.md#v4-数据集节--embedding-模型跨族--同族最大档探针beta-15b-7) + line 685-693
- BETA-15B-8 v4-fixup 节（bge-m3 真水位）：[baseline 报告 v4-fixup 节](../../reviews/semantic-recall-quality-baseline.md)
- 现 model-runtime：[packages/model-runtime/Cargo.toml:20](../../../packages/model-runtime/Cargo.toml#L20)（`llama-cpp-4 = "0.3.0"`）
- llama-cpp-4 upstream repo：https://github.com/eugenehp/llama-cpp-rs
- ROADMAP 锚：[ROADMAP §3.3 BETA-15B 系列](../../../ROADMAP.md)（本 cycle = BETA-15B-9 待 STATUS / ROADMAP doc-sync 落定）
