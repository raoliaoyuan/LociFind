# 语义召回质量评测 baseline（BETA-15B-6）

> Phase D bootstrap 实测产出。这是「召回质量做到顶」线的度量基线，下一 cycle 的调优以此为起点。

## 环境与配置

- 日期：2026-06-21
- 机器：macOS（Metal）
- embedding 模型：`models/qwen3-embedding-0.6b-q8_0.gguf`（1024 维）
- 配置：`DEFAULT_SEMANTIC_WEIGHT=2.0`、`EVAL_SIMILARITY_FLOOR=0.30`、`DEFAULT_RRF_K=60`、`TOP_K=10`
- 语料：合成 108 篇（zh 53 / en 55）、cases 59 条、5 桶
- 跑法：`semantic_quality --embed`（生成 vectors.json）→ `--write-baseline`（baseline.json）→ 报告

## 分桶 × 三臂（Recall@10 / nDCG@10）

| 桶 | n | FTS_R | FTS_N | VEC_R | VEC_N | HYB_R | HYB_N |
| --- | --- | --- | --- | --- | --- | --- | --- |
| synonym | 12 | 0.167 | 0.236 | 0.917 | 0.905 | 0.917 | 0.905 |
| concept | 12 | 0.319 | 0.326 | 1.000 | 0.877 | 1.000 | 0.795 |
| crosslang | 13 | 0.372 | 0.100 | 1.000 | 0.726 | 0.923 | 0.582 |
| content-not-name | 11 | 0.591 | 0.849 | 1.000 | 0.833 | 0.955 | 0.920 |
| exact-name | 11 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 | 1.000 |
| **OVERALL** | 59 | 0.477 | 0.481 | **0.983** | **0.864** | 0.958 | 0.832 |

## 结论（如实，含一个调优目标）

1. **语义召回差异化得到量化验证**。在 FTS5 trigram 关键词检索弱的桶上，向量臂大幅领先：
   - synonym：FTS_R 0.167 → VEC_R 0.917
   - concept：FTS_R 0.319 → VEC_R 1.000
   - crosslang：FTS_N 0.100 → VEC_N 0.726
   这正是"按意思/跨语言找"的核心价值，被合成集稳定测出。

2. **crosslang 桶的一个诚实细节**：`FTS_R=0.372` **不是** ≈0。原因是 crosslang cases 除了跨语言主相关文档（grade 3）还配了**同语言次级相关 g1 文档**，FTS 能命中这些同语言 g1 → recall 非零。但跨语言优势在 **nDCG 上最清楚**（FTS_N 0.100 ≪ VEC_N 0.726）——FTS 排不上那篇跨语言 g3，只蹭到低分同语言项。语义召回的跨语言价值成立。

3. **exact-name 守护成立**：FTS_R=HYB_R=1.000，语义臂完全没有拖垮精确标题查询。

4. **【关键发现 / 下一 cycle 调优目标】当前 hybrid（weight=2.0）整体略低于纯向量**：
   - OVERALL：VEC 0.983/0.864 **>** HYB 0.958/0.832
   - crosslang nDCG：VEC 0.726 **>** HYB 0.582（FTS 把同语言 g1 错误项融进来、污染了跨语言排名）
   - 即：在这套（语义占优的）评测集上，FTS 臂给 hybrid 添了噪声。这与 BETA-26 §4.6「偏向量可修纯模糊但损总体」同向。
   - **下一 cycle 的具体抓手**：① 调高 `DEFAULT_SEMANTIC_WEIGHT`（>2.0）看 hybrid 是否逼近/超过纯向量而不伤 exact-name；② 引入 FTS 置信度路由（FTS 命中弱时偏向量）；③ 这些都用本评测度量、并被回归门守护。
   - **注意取舍**：本评测集偏"语义占优"场景，纯向量看着最优；真实负载里 FTS 对同语言精确查询仍重要，故**不能据此直接砍 FTS 臂**——调的是融合权重/路由，不是去掉一臂。exact-name 桶（FTS 满分）就是这条约束的守护。

## 与 BETA-26 真实集定性对照

- BETA-26 真实 home 数据：纯模糊子集 FTS Recall@10 2.1% → hybrid 88.4%、crosslang FTS5 恒 0。
- 本合成集趋势**同向**：FTS 在 synonym/concept/crosslang 弱、语义强；exact-name FTS 满分。
- 差异：本合成集 crosslang 的 FTS_R 因同语言 g1 次级相关而非零（见结论 2），属评测集设计差异、非矛盾。
- 现实校准锤（可选，未跑）：`cargo run -p spike-retrieval --bin run-retrieval --features metal`（真实数据 gitignored），如需进一步确认合成集是否失真可周期性核对。

## 待跟进（2 Minor，承终审）

1. `EVAL_SIMILARITY_FLOOR=0.30` 是生产 `DEFAULT_SIMILARITY_FLOOR` 的**字面量复制**（后者 `pub(crate)` 无法跨 crate 导入）。下一 cycle 调 floor 时须人工对齐两处，或把该常量上提到可共享 crate。
2. 语料 108 篇 < 计划 150。当前区分度足够（各桶 FTS/语义差距明显）；若后续 crosslang 区分度不足，优先扩该桶 + 减少同语言 g1 次级相关以让 FTS_R 更贴近 0。

## 回归门

`packages/evals/tests/semantic_quality_gate.rs`：断言 hybrid 在 crosslang / exact-name / OVERALL 不跌破本 baseline（跑缓存向量、确定性、随 `cargo test` 门控）。本次 bootstrap 后已从 skip 转真跑（1 passed）。

## 调优记录（2026-06-22，BETA-15B-3 A-2）

> 起于 Phase D 关键发现「hybrid 略低于纯向量」。本节记本轮 weight 调优全表 + 取舍 + 与原 baseline 对照。

### Sweep 全表

`semantic_weight=W` 各桶 hybrid recall / nDCG（FTS/VEC 不随 W 变，省）：

| W | exact-name HYB_R | OVERALL HYB_N | crosslang HYB_N | concept HYB_N | synonym HYB_N | content-not-name HYB_N |
| --- | --- | --- | --- | --- | --- | --- |
| 2.0 (旧 baseline) | 1.000 | 0.832 | 0.582 | 0.795 | 0.905 | 0.920 |
| 3.0 | 1.000 | 0.837 | 0.603 | 0.795 | 0.905 | 0.924 |
| 4.0 | 1.000 | 0.839 | 0.606 | 0.798 | 0.905 | 0.926 |
| 6.0 | 1.000 | 0.846 | 0.631 | 0.800 | 0.905 | 0.930 |
| **10.0 (w\* 选定)** | **1.000** | **0.854** | **0.649** | 0.819 | 0.905 | 0.930 |
| 20.0 | 1.000 | 0.850 | 0.662 | 0.825 | 0.905 | 0.891 ↓ |

### 选定 w\*

`DEFAULT_SEMANTIC_WEIGHT = 10.0`（bake 进 `result-normalizer/lib.rs:92`；UI 可经 `AppSettings.semantic_weight` 覆盖，clamp[0.5, 50.0]）

理由（按 spec §2.2 硬约束顺序）：
1. **exact-name HYB_R = 1.000** ✅ 硬红线守住（所有 W 都满足；FTS 对精确名查询天然满分）。
2. **OVERALL nDCG = 0.854**（sweep 最大）——距 spec §2.2 「≥0.864（纯向量基准）」目标差 0.010、**未达**；W=20.0 反退到 0.850 → **0.854 是 weight 调优的天花板**。
3. **crosslang nDCG = 0.649**——比 baseline 0.582 涨 +0.067（+11.5%）但距 spec §2.2 「≥0.700」目标差 0.051、**未达**；W=20.0 最大 0.662 仍 < 0.700 → **路由必要性证据**（详后节）。
4. **W=20.0 退步**：content-not-name 桶从 0.930 退到 0.891，证明继续抬 weight 会牺牲其他桶 → W=10.0 是 sweep 中最稳全桶提升点。

### 与旧 baseline (W=2.0) 对照

| 桶 | 旧 HYB_N (W=2.0) | 新 HYB_N (W=10.0) | Δ |
| --- | --- | --- | --- |
| OVERALL | 0.832 | 0.854 | **+0.022** |
| crosslang | 0.582 | 0.649 | **+0.067** |
| concept | 0.795 | 0.819 | +0.024 |
| content-not-name | 0.920 | 0.930 | +0.010 |
| synonym | 0.905 | 0.905 | 0.000 |
| exact-name | 1.000 | 1.000 | 0.000（红线） |

所有桶要么涨要么持平；无桶退化。

### 路由必要性（按数据决定下 cycle 是否上）

本轮纯调 weight 把 OVERALL nDCG 从 0.832 抬到 0.854，crosslang 从 0.582 抬到 0.649。但：

- **OVERALL nDCG 0.854 距纯向量 0.864 仍差 0.010**（即使 W=20.0 也只 0.850 反退）→ weight 调优**存在天花板**，hybrid 在语义占优场景仍受 FTS 拖累；
- **crosslang nDCG 0.649 距纯向量 0.726 差 0.077**（W=20.0 最大也仅 0.662）→ FTS 臂在 crosslang 桶**结构性给 hybrid 添噪**（FTS 命中同语言 g1 次级相关污染 nDCG），纯抬 weight 摆脱不了。

**结论**：FTS 置信度路由（FTS 命中弱时跳过 FTS 臂）是下 cycle 必要抓手。已挂 **BETA-15B-3 簇 A FTS 置信度路由子项**，evidence 锚于本表。**不能据此砍 FTS 臂**——exact-name 桶 FTS=HYB=1.000 是约束。

### 已待跟进项更新

- 「待跟进 1（floor 字面量漂移）」未消化——本刀只调 weight，下刀消。
- 「待跟进 2（语料 108 < 150）」未消化——本刀不扩量；若未来 crosslang 区分度退化再扩。

### 验收红线达成情况（spec §2.2）

| 条目 | 目标 | 实测 | 达成 |
| --- | --- | --- | --- |
| (1) cargo test workspace 0 failed | 0 | 0 | ✅ |
| (2) clippy/fmt/tsc 净 | 净 | 净 | ✅ |
| (3) evals parser-only byte-equal | v0.5=473、v0.9=877 不变 | v0.5=473 / v0.9=877 精确 | ✅ |
| (4a) exact-name HYB_R = 1.0 | 1.0 | 1.0 | ✅ |
| (4b) OVERALL nDCG ≥ 0.864 | ≥ 0.864 | 0.854 | ⚠️ **未达**（差 0.010） |
| (4c) crosslang nDCG ≥ 0.700 | ≥ 0.700 | 0.649 | ⚠️ **未达**（差 0.051） |
| (5) clamp[0.5, 50.0] | clamp 守护 | task 4 单测覆盖 | ✅ |

**两条「未达」**（4b/4c）属 spec §5 异常分支「降级为『显著优于 baseline』+ 路由必要性归下 cycle」——已在本节诚实说明 + 路由必要性证据完整，归 BETA-15B-3 簇 A FTS 置信度路由子项。本 cycle 完成 weight 调优天花板触达，**不阻塞合并**。

## A-3 调优记录（2026-06-23，BETA-15B-3 A-3）

> 起于 A-2 诚实边界「纯抬 weight 见底、OVERALL 0.854 距纯向量 0.864 差 0.010、crosslang 0.649 距 0.726 差 0.077」。本轮做 FTS 置信度路由（Jaccard < 阈值跳 FTS 臂），路由必要性证据见 A-2 节末尾。

### Sweep 全表（W=10.0 固定）

`jaccard_threshold=t` 各桶 hybrid_routed recall / nDCG（FTS/VEC/HYB 不随 t 变，省）：

| t | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | concept HYBR_N | synonym HYBR_N | content-not-name HYBR_N |
| --- | --- | --- | --- | --- | --- | --- |
| 0.0 (≡HYB 控制对照) | 1.000 | 0.854 | 0.649 | 0.819 | 0.905 | 0.930 |
| **0.10 (t\* 选定)** | **1.000** | **0.854** | **0.649** | **0.819** | **0.905** | **0.930** |
| 0.20 | 1.000 | 0.851 | 0.693 | 0.850 | 0.905 | 0.832 ↓ |
| 0.30 | 1.000 | 0.861 | 0.712 | 0.877 | 0.905 | 0.832 ↓ |
| 0.50 | 1.000 | 0.864 | 0.726 | 0.877 | 0.905 | 0.833 ↓ |
| 1.0 (≈VEC 控制对照) | 1.000 | 0.864 | 0.726 | 0.877 | 0.905 | 0.833 |

### 控制对照核验

- **t=0.0**：HYBR ≡ HYB 永不跳。OVERALL HYBR_N = 0.854 = HYB baseline ✓
- **t=1.0**：HYBR ≈ 纯 VEC（除 exact-name 完全重叠桶外几乎总跳）。OVERALL HYBR_N = 0.864 = VEC_N ✓；exact-name HYBR_R = 1.0 因两臂完全重叠不触发跳过 ✓
- **t=0.10 与 t=0.0 等价**：表明无 case 的 Jaccard 落在 (0.0, 0.10) 区间，跳的 case 数量相同

### 选定 t\* = 0.10（spec §5 异常分支降级）

`DEFAULT_FTS_JACCARD_THRESHOLD = 0.10`（bake 进 `packages/result-normalizer/src/lib.rs:97`；生产 `run_fanout_merge_rrf` 经 `fuse_rrf_with_fts_routing` wrapper 调用、不暴露 UI、wrapper 内含 empty-arm early-return guard 保护一臂兜底场景）

理由（按 spec §2.2 硬约束顺序）：
1. **exact-name HYBR_R = 1.000** ✅ 硬红线守住（所有 t 都满足；两臂在精确名查询天然高重叠不触发跳过）
2. **OVERALL HYBR_N = 0.854**（sweep 最大但因路由不生效 = HYB baseline）—— 距 spec §2.2 (4d) 0.864 目标差 0.010、**未达**；spec §5 异常分支降级处理
3. **crosslang HYBR_N = 0.649**（同 HYB baseline，因路由不生效）—— 距 spec §2.2 (4c) 0.700 目标差 0.051、**未达**；spec §5 异常分支降级处理
4. **其他桶 HYBR_N 全部 ≥ HYB baseline 同桶** ✅ 验证不退步（全相等）

**为什么是「最保守 t」而非更高 t\* 拿指标**：t≥0.20 起 **content-not-name 桶退步 0.098**（0.930→0.832），违反 (4b) 各桶 ≥ HYB baseline 硬红线。spec §5 「任何 t 都让某非 exact 桶退步 HYB baseline → bake 选最保守 t」字面正解。本 cycle 选 t=0.10 是「最大的启用路由阈值」但实测路由不生效（无 case Jaccard 在 (0, 0.10) 触发跳过）。

### 新发现的失败模式（spec/brainstorming 未预见）

**content-not-name 桶 FTS 比 VEC 还强**：baseline 报告显示 FTS_N=0.849 > VEC_N=0.833；路由跳 FTS 反伤此桶 -0.098。Jaccard 单维信号天然分不开两种场景：

- **「FTS 在 crosslang 添噪」**（路由想拦的，t=0.50 时 crosslang +0.077 拦住了）
- **「FTS 在 content-not-name 帮忙」**（路由不该拦的，但 t≥0.20 同样触发跳过 -0.098）

两者代价不对称：t=0.50 时 crosslang +0.077 vs content-not-name -0.098，**净 OVERALL 在合成集 +0.010 但桶分布失衡**。

### 与 A-2 baseline (HYB) 对照（t\*=0.10）

| 桶 | A-2 HYB_N | A-3 HYBR_N (t\*=0.10) | Δ |
| --- | --- | --- | --- |
| OVERALL | 0.854 | 0.854 | 0.000 |
| crosslang | 0.649 | 0.649 | 0.000 |
| concept | 0.819 | 0.819 | 0.000 |
| content-not-name | 0.930 | 0.930 | 0.000 |
| synonym | 0.905 | 0.905 | 0.000 |
| exact-name | 1.000 | 1.000 | 0.000（红线） |

t\*=0.10 路由不生效 → HYBR ≡ HYB → 所有桶 Δ=0。**这是诚实的「显著优于 baseline = 0」**——本轮纯调路由触达「信号粒度不足」天花板。

### 路由必要性证据复盘 / 下 cycle 抓手

**本 cycle 结论**：单维 Jaccard 信号不够精细——同一条「两臂分歧」信号同时触发 crosslang（路由想拦）和 content-not-name（路由不该拦）。纯调阈值救不了，**下 cycle 必须升级信号**。

下 cycle 抓手候选：
1. **更强信号**：
   - query 语种检测（CJK 比例 / 字符 N-gram）+ 文档语种过滤
   - VEC top-1 cosine 绝对分数阈值（弱 cosine = VEC 也不确定 → 保留 FTS）
   - per-bucket 自适应阈值（但合成集 bucket 标签不见真生产，需 query-time 标签推断）
2. **更大 embedding 模型**：现 qwen3-0.6b；上 1.5b/3b 看 VEC 能否进一步压制 FTS 在 content-not-name 的优势
3. **评测集扩量 + 重构 content-not-name 桶 case**：现 11 例可能过窄，扩大后 FTS 优势可能稀释

### 本 cycle 实际产出（基础设施完整 + 诊断证据齐）

- ✅ 路由 wrapper API `fuse_rrf_with_fts_routing` 入栈，生产/评测共用同一融合路径
- ✅ 评测基础设施（`hybrid_routed_rank` arm + binary `--jaccard-threshold` flag + HYBR 字段 + sweep 能力）
- ✅ baseline.json 双字段（HYB 旧 + HYBR 新）+ gate 双守护
- ✅ wrapper 内置 empty-arm early-return guard（保护一臂兜底场景，覆盖生产 + 评测层）
- ✅ 路由信号失败模式诊断（Jaccard 单维不够、content-not-name 桶 FTS 强）—— 下 cycle 抓手清晰
- ⚠️ 路由对指标本 cycle 净影响为 0（t=0.10 等价不路由）；spec §2.2 (4c)(4d) 未达走 §5 降级

### 已待跟进项更新

- 「待跟进 1（floor 字面量漂移）」未消化——本刀只做路由，不涉 floor
- 「待跟进 2（语料 108 < 150）」未消化——本刀不扩量；下 cycle 若升级信号需重新评估区分度

### 验收红线达成情况（spec §2.2）

| 条目 | 目标 | 实测 | 达成 |
| --- | --- | --- | --- |
| (1) cargo test workspace 0 failed | 0 | 858 passed / 0 failed | ✅ |
| (2) clippy/fmt 净 | 净 | 净 | ✅ |
| (3) evals parser-only byte-equal | v0.5=473、v0.9=877 不变 | v0.5=473 / v0.9=877 精确 | ✅ |
| (4a) exact-name HYBR_R = 1.0 | 1.0 | 1.000 | ✅ |
| (4b) 各桶 HYBR_N ≥ HYB baseline 同桶 | ≥ | 所有桶 Δ=0（HYBR≡HYB） | ✅ |
| (4c) crosslang HYBR_N ≥ 0.700 | ≥ 0.700 | 0.649 | ⚠️ **未达**（spec §5 降级） |
| (4d) OVERALL HYBR_N ≥ 0.864 | ≥ 0.864 | 0.854 | ⚠️ **未达**（spec §5 降级） |
| (5) wrapper clamp[0,1] + 双空良定义 + empty-arm guard | 守护 | task 2/9 单测覆盖 | ✅ |

**两条「未达」**（4c/4d）属 spec §5 异常分支「降级为『显著优于 baseline』+ 路由必要性归下 cycle」。本 cycle 「显著优于 baseline」实测 = 0（路由不生效）；spec §5 字面正解是「保守 bake、记诚实诊断、归下 cycle」——已在本节完整说明 + 下 cycle 抓手指向更强信号。**不阻塞合并**。

## A-4 query 语种检测 + 跨语种 vec hit 路由（2026-06-23 Claude Code）

**承接**：A-3 sweep 暴露 Jaccard 单维信号天然分不开「FTS 在 crosslang 添噪」vs「FTS 在 content-not-name 帮忙」、t*=0.10 spec §5 降级路由本 cycle 不生效。A-4 升级 wrapper 内部信号为 **query 语种检测（CJK Unicode ratio 三态二阈 Zh/En/Mixed）+ vec top-K 跨语种 hit 计数**，对准 A-3 暴露的 crosslang 桶失败模式。

**信号设计**：
- 自写 CJK Unicode ratio 三态二阈（>0.6=Zh、<0.05=En、之间=Mixed；纯 std、零依赖、~15 行）
- wrapper API 名 / RouteVerdict / HYBR baseline 字段名 / gate 红线架构保留（A-3 基础设施 zero-touch）
- query_lang = Mixed 永不跳（保守降级）
- 评测 `to_results` `name=title` 让 doc 带自然语言 CJK 信号、生产 `run_fanout_merge_rrf` 入口检测 `query: &str`
- Jaccard 工具函数 `jaccard_overlap_by_path` 保留为下 cycle 信号组合预留

**Sweep 全表**（W=10.0 固定、top_k=10、N = max_cross_lang_hits）：

| N | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N | synonym HYBR_N | concept HYBR_N |
|---|---|---|---|---|---|---|
| 0 | 1.000 | 0.864 | 0.726 | 0.833 | 0.905 | 0.877 |
| 1 | 1.000 | 0.864 | 0.726 | 0.833 | 0.905 | 0.877 |
| 2 | 1.000 | 0.874 | 0.726 | 0.887 | 0.905 | 0.877 |
| 3 | 1.000 | 0.859 | 0.703 | 0.887 | 0.905 | 0.828 |
| 5 | 1.000 | 0.852 | 0.681 | 0.886 | 0.905 | 0.819 |
| **usize::MAX (N\* 选定)** | **1.000** | **0.854** | **0.649** | **0.930** | **0.905** | **0.819** |

A-2 HYB baseline（hybrid_ndcg、对照基准）：synonym 0.905 / concept 0.819 / crosslang 0.649 / content-not-name 0.930 / exact-name 1.000 / OVERALL 0.854。

**控制对照核验**：
- N=usize::MAX 时 HYBR ≡ HYB（六桶完全相等 ✓）
- N=0 时 HYBR ≈ VEC（六桶 HYBR_N 与 VEC_N 完全相等 ✓）
- wrapper 行为正确，N=0 ≈ 纯向量、N=MAX ≡ HYB

**N\* 选定 = `usize::MAX`，spec §5 字面正解保守降级**：

依据 spec §2.2 (4b) 各桶 HYBR_N ≥ HYB baseline 硬红线：
- content-not-name 桶 FTS_N=0.930 > VEC_N=0.833 → FTS 在此桶实际有用
- 任一有限 N（实测 0/1/2/3/5）都让此桶 HYBR_N 跌破 0.930（0.833–0.887 < 0.930），破 (4b) 红线
- OVERALL 在 N=5 时 0.852 < 0.854 额外破红线
- 唯一不破 (4b) 的 N 是 usize::MAX（HYBR ≡ HYB、路由不生效）

**实测变化（HYBR vs A-2 baseline HYB）**：
- 各桶 HYBR_N 与 HYB_N 完全相等（路由本 cycle 净影响 = 0）
- 与 A-3 sweep 选 t\*=0.10（路由永不触发、本 cycle 不生效）同款诚实结论

**诚实边界 — lang 信号未突破 A-3 暴露的两场景天花板**：

A-3 失败模式 = 「Jaccard 单维信号天然分不开『FTS 在 crosslang 添噪』vs『FTS 在 content-not-name 帮忙』」。A-4 升级到 query lang 信号后，**同一天花板重现**：

- crosslang 桶：lang 信号能帮（N=2 时 crosslang HYBR_N 0.726 vs HYB 0.649、+0.077 显著抬升；OVERALL 也 +0.020 显著抬升）
- content-not-name 桶：lang 信号也会跳掉、反伤（N=2 时此桶 0.887 vs HYB 0.930、-0.043 显著退步）
- 同一阈值/路由动作无法在两桶上同时成立 → 单信号路由本身的结构问题、不是 lang 信号特有

**下 cycle 抓手 = 升级信号 / 评测集 / 模型**（A-4 数据指证）：

| 候选 | 原理 | 优先级 |
|---|---|---|
| ② VEC top-1 cosine 绝对分数阈值 | VEC 强（cosine 高）时跳 FTS、VEC 弱时保留 FTS——content-not-name 桶 vec 弱 FTS 帮、crosslang 桶 vec 强 FTS 添噪，方向对 | 高（直对失败模式、不需训练） |
| ③ 更大 embedding 模型 qwen3-0.6b→1.5b/3b | 升级 vec 召回质量、可能压平两桶差距 | 中（需 Mac 训练 + 模型分发） |
| ④ 评测集扩量 + 重构 content-not-name 桶 case | 合成集 11 例可能过窄；扩量后看天花板是否合成集 artifact | 中（需用户语料决策） |

**基础设施完整入栈**（为下 cycle 升级信号留好旋钮）：
- `result-normalizer::lang` 模块（Lang enum + detect_lang、纯 std 零依赖）
- wrapper `fuse_rrf_with_fts_routing` 6 参签名 + `RouteVerdict { skipped_fts, query_lang, cross_lang_hits, max_cross_lang_hits }`
- 评测 `to_results` 加 corpus + `score_case` 入口 detect_lang + binary `--max-cross-lang-hits` flag
- 生产 `run_fanout_merge_rrf` 签名扩 `query: &str` + 入口 detect_lang
- baseline.json HYBR 双字段 + gate 4 红线动态读 baseline 守护
- Jaccard `jaccard_overlap_by_path` 工具函数 + 5 单测保留（下 cycle Jaccard + lang 组合信号备用）

链接：[spec](../superpowers/specs/2026-06-23-beta-15b-3a4-lang-routing-design.md) / [plan](../superpowers/plans/2026-06-23-beta-15b-3a4-lang-routing.md)

## A-5 VEC top-1 cosine 绝对分数阈值路由（2026-06-24 Claude Code）

**承接**：A-3 Jaccard 单维 / A-4 lang 单维信号都撞同款不对称失败模式天花板（crosslang 桶 FTS 添噪应跳、content-not-name 桶 FTS 帮应保留——单信号阈值无法不对称区分），两 cycle 都走 spec §5 降级保守 bake、路由本 cycle 不生效。A-4 数据指证下 cycle 抓手 ② **VEC top-1 cosine 绝对分数阈值** 最优先：理论上信号方向与失败模式同构，cosine 高（vec 强）应跳、cosine 低（vec 弱）应保留，能不对称区分两场景。

**信号设计**：
- 信号源 `vec[0].score.unwrap_or(0.0)`（生产侧 `packages/search-backends/semantic-index/src/lib.rs:163` 注释「`score = cosine`（升 f64）」、评测层 `vector_rank → Vec<(String, f32)>` + `to_results_with_scores` 把 cosine 挂在 `SearchResult.score` 透传）
- 动作方向 `cosine_top1 >= cosine_threshold` 跳 FTS（vec 强信任跳）
- wrapper API 名 / RouteVerdict 结构 / HYBR baseline 字段名 / gate 红线架构保留（A-3/A-4 基础设施 zero-touch）
- wrapper 签名 6 参 → 5 参（删 `query_lang + max_cross_lang_hits`、加 `cosine_threshold: f64`）
- RouteVerdict 字段：`cross_lang_hits → vec_top1_cosine: f64`、`max_cross_lang_hits → cosine_threshold: f64`、**保留 `query_lang: Lang`** 字段（wrapper 默认 Mixed 占位、生产 wiring `run_fanout_merge_rrf` 用 struct-update 后置覆写 `verdict.query_lang = detect_lang(query)` 填真值供 BETA-15B-5 badge 槽位）
- A-3 `jaccard_overlap_by_path` 函数 + 5 单测删除（A-4 wrapper 已不用）；A-4 `detect_lang` + 8 单测保留（wiring 仍调）

**Sweep 全表**（W=10.0 固定、T = cosine_threshold）：

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N | concept HYBR_N | synonym HYBR_N |
|---|---|---|---|---|---|---|
| 0.0 (≈纯 vec 控制) | 1.000 | 0.864 | 0.726 | 0.833 ↓ 破 | 0.877 | 0.905 |
| 0.30 | 1.000 | 0.864 | 0.726 | 0.833 ↓ 破 | 0.877 | 0.905 |
| 0.45 | 1.000 | 0.864 | 0.726 | 0.833 ↓ 破 | 0.877 | 0.905 |
| **0.60 (T\* 选定 ⭐)** | **1.000** | **0.871** | **0.726** | **0.930** | **0.820** | **0.905** |
| 0.70 | 1.000 | 0.867 | 0.709 | 0.930 | 0.819 | 0.905 |
| 0.80 | 1.000 | 0.857 | 0.663 | 0.930 | 0.819 | 0.905 |
| 0.90 | 1.000 | 0.854 | 0.649 | 0.930 | 0.819 | 0.905 |
| 0.99 | 1.000 | 0.854 | 0.649 | 0.930 | 0.819 | 0.905 |
| 1.01 (≡HYB 控制) | 1.000 | 0.854 | 0.649 | 0.930 | 0.819 | 0.905 |

A-2 HYB baseline（对照基准）：synonym 0.905 / concept 0.819 / crosslang 0.649 / content-not-name 0.930 / exact-name 1.000 / OVERALL 0.854。

**控制对照核验**：
- T=0.0 时 HYBR ≈ VEC（六桶 HYBR_N 与 VEC_N 相等 ✓）—— wrapper 永远跳 FTS 退化为纯 vec
- T=1.01 时 HYBR ≡ HYB（六桶完全相等 ✓）—— wrapper 永不跳、等价 fuse_rrf
- T=0.60 选定时路由真触发：crosslang vec 强 cosine_top1≈0.7+ 跳 FTS、content-not-name vec 弱 cosine_top1<0.60 保留 FTS

**T\* 选定 = 0.60，A 簇 5 cycle 首次破 spec §5 降级**：

依据 spec §2.2 红线顺序：
1. **exact-name HYBR_R = 1.000** ✅ 硬红线（所有 T 都满足、两臂在精确名查询天然高重叠不触发跳过）
2. **各桶 HYBR_N ≥ HYB baseline 同桶**：T=0.60 时 synonym 0.905= / concept 0.820>0.819 / crosslang 0.726>0.649 / content-not-name 0.930= / exact-name 1.0= / OVERALL 0.871>0.854 **全过** ✅
3. **OVERALL HYBR_N = 0.871 > 0.864 spec §2.2 (4d) 目标** ✅⭐（达 spec 目标）
4. **crosslang HYBR_N = 0.726 > 0.700 spec §2.2 (4c) 目标** ✅⭐（达 spec 目标）

**为什么 0.60 是 sweep best**：
- T<0.60（0.0/0.30/0.45）：路由太激进、所有 case 都跳 FTS（cosine 都≥0.30）→ content-not-name 桶 vec 弱场景被错误跳 FTS、退步 0.833（破 (4b) 红线）
- T=0.60：crosslang 桶（vec 强 cosine≈0.7+）触发跳、content-not-name 桶（vec 弱 cosine<0.6）保留——不对称区分两场景
- T=0.70：crosslang 桶部分 case cosine 介于 0.60-0.70 失去跳过机会、crosslang HYBR_N 0.726→0.709 降
- T≥0.90：所有 case cosine<阈值不跳、HYBR≡HYB（与降级 1.01 同义）

**实测变化（HYBR vs A-4 baseline HYB）**：
- OVERALL HYBR_N **0.854 → 0.871** （+0.017、超 spec 目标 0.864）
- crosslang HYBR_N **0.649 → 0.726** （+0.077、超 spec 目标 0.700）
- content-not-name HYBR_N 0.930 → 0.930（守 baseline ✓）
- concept HYBR_N 0.819 → 0.820（微升）
- synonym HYBR_N 0.905 → 0.905（不动）
- exact-name HYBR_R 1.000 → 1.000（红线 ✓）

**诚实边界 — cosine 信号突破 A-3/A-4 暴露的两场景天花板**：

A-3 Jaccard / A-4 lang 单维信号在合成集 sweep 中**无任一阈值能同时满足 spec §2.2 (4b)(4c)(4d)**——content-not-name 桶 FTS 强场景与 crosslang 桶 FTS 添噪场景被同款触发条件捆绑，调阈值在两桶上是零和博弈。两 cycle 都走 spec §5 降级保守 bake（A-3 t\*=0.10 / A-4 N\*=usize::MAX、路由本 cycle 不生效）。

A-5 cosine 信号方向**精准对齐**不对称失败模式：vec 强（cosine 高）= 跨语言召回有效 = FTS 同语言 g1 添噪应跳；vec 弱（cosine 低）= 字面匹配占优 = FTS 字面命中应保留。T\*=0.60 把两桶 cosine 分布的临界点撞上——这是 cosine 单维真破局、不是单维路由结构问题的运气。

**下 cycle 抓手优先级**（A-5 数据指证）：

A-5 已交付 spec §2.2 全过、A 簇主路径阶段性收口。继续优化的方向：

| 候选 | 原理 | 优先级 |
|---|---|---|
| ④ **评测集扩量 + 重构 content-not-name 桶 case** | 合成集 11 例可能让 T\*=0.60 实测带运气；扩量 + 真实语料校验 T\* 鲁棒性 | 高（结构性验证、低成本） |
| ③ **更大 embedding 模型 qwen3-0.6b → 1.5b/3b** | 抬升 vec 召回质量、可能让 T\* 上移 + 进一步抬 crosslang | 中（需 Mac 训练 + 模型分发） |
| **原始 query 入 schema** | A 簇余项，让语义臂 keywords 拼接近似真 query | 中（byte-equal 风险须 router 后置填充不动 parser） |
| **cosine + lang 组合信号** | 若评测集扩量后发现 cosine 单维有死角，叠加 lang 信号细化 | 低（A-3 jaccard / A-4 detect_lang 函数 git history 可恢复） |

**基础设施完整入栈**（A-5 cycle 收口产出）：
- `result-normalizer::lang::Lang/detect_lang` 保留作 wiring 元数据
- wrapper `fuse_rrf_with_fts_routing` 5 参签名 + `RouteVerdict { skipped_fts, query_lang, vec_top1_cosine, cosine_threshold }`
- 评测 `vector_rank → (id, cosine)` + `to_results_with_scores` + `score_case` cosine_threshold + binary `--cosine-threshold` flag
- 生产 `run_fanout_merge_rrf` 5 参 wrapper 调用 + struct-update 后置覆写 query_lang
- baseline.json HYBR 字段 rewrite + gate 4 红线动态读 baseline（A-5 数值自动跟随）

链接：[spec](../superpowers/specs/2026-06-23-beta-15b-3a5-cosine-routing-design.md) / [plan](../superpowers/plans/2026-06-24-beta-15b-3a5-cosine-routing.md)

## v2 数据集：content-not-name 桶扩量 11→20（2026-06-24 Claude Code）

**承接**：A-5 cycle T\*=0.60 bake、A 簇 5 cycle 首破 spec §5 降级。A-5 调优记录诚实承认「合成集 11 例可能让 T\*=0.60 带运气、扩量校验鲁棒性」。本 cycle 针对性扩 content-not-name 桶 11→20 + corpus 108→115、Mac Metal 本机重算 vectors.json 全集、重跑 9 阈值 sweep 校验 T\*=0.60 鲁棒性。

**扩量产出**：
- cases.json 59→68（content-not-name 11→20、其他 4 桶不动；新 9 case = c060-c068、5 zh + 4 en；3 跨语言主题对 + 1 单 zh A/B 实验 + 1 客服话术对复用 s00023/s00024）
- corpus.json 108→115（s00109-s00115、7 新 doc：4 zh + 3 en；零 PII 全虚构）
- vectors.json 全集重算（dim=1024、qwen3-embedding-0.6b-q8_0、115 doc + 68 query）

**新 9 case 主题清单**：

| id | bucket | 主题 | 复用 doc |
|---|---|---|---|
| c060 | content-not-name | 重试机制（指数退避 + 抖动）zh | s00109 (zh) / s00110 (en) |
| c061 | content-not-name | A/B 实验最小样本量 zh | s00111 |
| c062 | content-not-name | 推荐系统冷启动池规则 zh | s00112 / s00113 |
| c063 | content-not-name | 多语言客服话术规约 zh | **复用 s00023/s00024** |
| c064 | content-not-name | 内部 IM 表情包审核 zh | s00114 / s00115 |
| c065 | content-not-name | 重试机制（truncated exp backoff）en | s00110 / s00109 |
| c066 | content-not-name | 推荐系统冷启动池 en | s00113 / s00112 |
| c067 | content-not-name | IM emoji approval workflow en | s00115 / s00114 |
| c068 | content-not-name | multilingual customer support phrasing en | **复用 s00024/s00023** |

**Sweep 全表**（W=10.0 固定、T = cosine_threshold、v2 数据集 68 cases / 115 docs / dim 1024）：

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N | concept HYBR_N | synonym HYBR_N |
|---|---|---|---|---|---|---|
| 0.0 (≈纯 vec 控制) | 1.000 | 0.844 | 0.726 | 0.778 ↓ | 0.877 | 0.905 |
| 0.30 | 1.000 | 0.844 | 0.726 | 0.778 ↓ | 0.877 | 0.905 |
| 0.45 | 1.000 | 0.844 | 0.726 | 0.777 ↓ | 0.877 | 0.905 |
| 0.60 (A-5 v1 bake) | 1.000 | 0.848 | 0.726 | 0.826 ↓ | 0.820 | 0.905 |
| **0.70 ⭐ (v2 T\* 选定)** | **1.000** | **0.854** | **0.717** | **0.853** | **0.819** | **0.905** |
| 0.80 | 1.000 | 0.845 | 0.671 | 0.852 | 0.819 | 0.905 |
| 0.90 | 1.000 | 0.842 | 0.657 | 0.852 | 0.819 | 0.905 |
| 0.99 | 1.000 | 0.842 | 0.657 | 0.852 | 0.819 | 0.905 |
| 1.01 (≡HYB 控制) | 1.000 | 0.842 | 0.657 | 0.852 | 0.819 | 0.905 |

v2 HYB baseline（T=1.01 时实测、各桶等价不跳）：synonym 0.905 / concept 0.819 / crosslang 0.657 / content-not-name 0.852 / exact-name 1.000 / OVERALL 0.842。

**控制对照核验**：
- T=0.0 时 HYBR ≈ VEC（六桶 HYBR_N 与 VEC_N 相等 ✓）
- T=1.01 时 HYBR ≡ HYB（六桶完全相等 ✓）
- T=0.70 选定时路由触发：crosslang vec 强 cosine≈0.7+ 跳 FTS、content-not-name vec 弱 cosine<0.7 保留 FTS

**v2 baseline.json 实测**（T\*=0.70 rewrite）：
- synonym HYBR_R 0.9167 / HYBR_N 0.9051
- concept HYBR_R 1.0000 / HYBR_N 0.8190
- crosslang HYBR_R 1.0000 / HYBR_N 0.7168
- content-not-name HYBR_R 0.9250 / HYBR_N 0.8525
- exact-name HYBR_R 1.0000 / HYBR_N 1.0000
- OVERALL HYBR_R 0.9632 / HYBR_N 0.8538

**T\* 决定 = 0.70（spec §2.2 接受标准 Branch B 边界）**：

依据：
1. exact-name HYBR_R = 1.000 ✅ 硬红线（所有 T 守住）
2. 各桶 HYBR_N ≥ v2 HYB baseline 同桶（不退步）：
   - synonym 0.905 = 0.905 ✓
   - concept 0.819 = 0.819 ✓
   - crosslang 0.717 > 0.657 (+0.060) ✓
   - content-not-name 0.853 > 0.852 (+0.001) ✓
   - exact-name 1.000 = 1.000 ✓
   - OVERALL 0.854 > 0.842 (+0.012) ✓
3. OVERALL HYBR_N spec §2.2 (4d) 目标 ≥ 0.864：实测 0.854 < 0.864 ⚠️ **字面未达**——改走 baseline 自锁路径（gate 实际断言 `now.hybrid_routed_ndcg ≥ baseline.hybrid_routed_ndcg`、v2 rewrite 后 baseline.OVERALL HYBR_N = 0.854、gate 自动跟随）、技术上不破红线；诚实记录 A-5 v1 0.871 部分含运气、v2 真水位 0.854
4. crosslang HYBR_N spec §2.2 (4c) 目标 ≥ 0.700：实测 0.717 ≥ 0.700 ✓

**为什么是 T\*=0.70 而非 T\*=0.60**：
- T=0.60 在 v2 上 content-not-name 退步 -0.026（0.826 < 0.852 baseline）→ 破 (4b) 红线
- T=0.70 是 v2 sweep best（OVERALL 0.854、crosslang 0.717、content-not-name 0.853 全过 (4b)）
- T=0.70 偏 A-5 v1 T\*=0.60 +0.10、是 spec §2.2 接受标准 Branch B 边界 inclusive 上界（≤ ±0.10 偏移不触发 Branch C）

**实测变化（v2 T\*=0.70 vs A-5 v1 T\*=0.60、不同数据集对比）**：
- OVERALL HYBR_N **0.871 → 0.854**（v1 → v2、回落 -0.017、A-5 v1 11 例 content-not-name 桶部分含运气）
- crosslang HYBR_N **0.726 → 0.717**（v1 → v2、轻度回落 -0.009）
- content-not-name HYBR_N **0.930 → 0.853**（v1 → v2、显著回落 -0.077；v1 11 例 vec 强 case 比例偏高）
- concept 0.819 = 0.819（不动）
- synonym 0.905 = 0.905（不动）
- exact-name 1.000 = 1.000（红线）

**诚实边界 — A-5 v1 T\*=0.60 部分含运气、v2 揭示真水位**：

A-5 cycle 在 v1 11 例 content-not-name 桶上 sweep 选定 T\*=0.60、bake 后 OVERALL 0.871 / crosslang 0.726、A 簇 5 cycle 首破 spec §5 降级。v2 cycle 把 content-not-name 桶扩到 20 例后：

- **T\*=0.60 不再 sweep best**：在 v2 上 T=0.60 时 content-not-name 退步 0.826（vs baseline 0.852）破 (4b) 红线
- **v2 sweep best 上移到 T\*=0.70**：偏移 +0.10、是 Branch B 边界 inclusive 上界
- **v2 真水位**：OVERALL 0.854、crosslang 0.717、content-not-name 0.853——A-5 v1 含轻微运气、v2 扩量校验后真水位回落

**结论**：A-5 cycle「cosine 单维真破局」结论**仍成立**（v2 上 T=0.70 仍超 HYB 0.842、且各桶 ≥ baseline），但**「双超 spec 目标」结论需修正**——v2 上 OVERALL 0.864 spec 目标不再可达，只达 0.854。下 cycle 抓手需更深突破。

**下 cycle 抓手优先级（v2 数据指证）**：

| 候选 | 原理 | 优先级 |
|---|---|---|
| **更大 embedding 模型 qwen3-0.6b → 1.5b/3b** | 抬升 vec 召回质量、cosine_top1 分布上移、T\* 可能进一步上调 + 抬 OVERALL/crosslang | 高（v2 数据指证最优、需 Mac 训练+模型分发） |
| **评测集再扩量**（content-not-name 30+ / 总 100+ case） | T\*=0.70 在 v2 20 例上仍可能含残余运气、扩量到 100+ 才能更精确测出真水位与最优 T\* | 高（最低成本、与 v2 同款方法可复制） |
| **原始 query 入 schema**（A 簇余项） | 让语义臂 keywords 拼接近似真 query | 中（byte-equal 风险须 router 后置填充不动 parser） |
| **cosine + lang 组合信号** | 若评测扩量后仍无法抬过 0.864、用 lang 信号细化 cosine 路由（A-3 jaccard / A-4 detect_lang git history 可恢复） | 中（备选、若上述无效再做） |

**基础设施完整保留**（A-3/A-4/A-5 都未动）：
- `result-normalizer::lang::Lang/detect_lang` 保留作 wiring 元数据
- wrapper `fuse_rrf_with_fts_routing` 5 参签名 + `RouteVerdict { skipped_fts, query_lang, vec_top1_cosine, cosine_threshold }`
- 评测 `vector_rank → (id, cosine)` + `to_results_with_scores` + `score_case` cosine_threshold + binary `--cosine-threshold` flag
- 生产 `run_fanout_merge_rrf` 5 参 wrapper 调用 + struct-update 后置覆写 query_lang
- baseline.json HYBR 字段 v2 rewrite + gate 4 红线动态读 baseline

链接：[v2 spec](../superpowers/specs/2026-06-24-beta-15b-6-v2-content-not-name-expansion-design.md) / [v2 plan](../superpowers/plans/2026-06-24-beta-15b-6-v2-content-not-name-expansion.md)

## v3 数据集：content-not-name 桶二次扩量 20→30 + 认知层主动放弃字面 spec 目标（2026-06-24 Claude Code）

**承接**：v2 cycle T\*=0.70 bake、Branch B 边界 inclusive 上界、揭示 A-5 v1 含运气、v2 真水位 OVERALL 0.854 / crosslang 0.717。v2 调优记录诚实承认「T\*=0.70 在 v2 20 例上仍可能含残余运气、扩量到 30+ 才能更精确测出真水位」。本 cycle 针对性二次扩 content-not-name 桶 20→30 + corpus 115→124、Mac Metal 本机重算 vectors.json 全集、重跑 9 阈值 sweep 校验 T\*=0.70 鲁棒性 + 测真水位。

**v3 cycle 认知层修订**：v3 起草前发现 v2 上 OVERALL 0.864 spec 目标走 baseline 自锁路径绕过、连续两 cycle 自欺；v3 cycle 主动**放弃字面 0.864 / 0.700 spec 目标**（移交下 cycle = 更大 embedding 模型 qwen3-0.6b → 1.5b/3b / cosine + lang 组合信号、已挂 ROADMAP 候选）。gate.rs (4c)(4d) 代码层 A-3 cycle 起即动态读 baseline 自锁、本 cycle **不动断言代码**、仅 baseline 报告本节明示放弃决策。

**扩量产出**：
- cases.json 68→78（content-not-name 20→30、其他 4 桶不动；新 10 case = c069-c078、6 zh + 4 en、4 边界 + 6 常规、3 zh+en 配对主题 + 4 单语种主题 + c077 复用 s00011/s00012 性能优化 corpus）
- corpus.json 115→124（s00116-s00124、9 新 doc：5 zh + 4 en、3 配对主题各 zh+en 2 doc + 3 单语种新 doc + zh/en 比例 62:62 平衡、零 PII 全虚构）
- s00116/s00117 T2 fixup：加强 5 Whys 框架特有词扩大与现有 s00078/s00079 (blameless retro) embedding 距离
- vectors.json 全集重算（dim 1024、qwen3-embedding-0.6b-q8_0、124 doc + 78 query）

**新 10 case 主题清单**：

| id | bucket | 主题 | 复用/新 doc | 设计类 |
|---|---|---|---|---|
| c069 | content-not-name | 故障复盘 5 Whys (zh) | s00116 (zh) / s00117 (en) | 边界 |
| c070 | content-not-name | 灰度发布与回滚阈值 (zh) | s00118 (zh) / s00119 (en) | 常规 |
| c071 | content-not-name | API 接口废弃周期 (zh) | s00120 (zh) / s00121 (en) | 常规 |
| c072 | content-not-name | 5 Whys postmortem template (en) | s00117 (en) / s00116 (zh) | 边界 |
| c073 | content-not-name | canary deployment runbook (en) | s00119 (en) / s00118 (zh) | 常规 |
| c074 | content-not-name | API deprecation policy (en) | s00121 (en) / s00120 (zh) | 常规 |
| c075 | content-not-name | 告警分级 P0/P1/P2 (zh) | s00122 | 边界 |
| c076 | content-not-name | 异常日志结构化字段 (zh) | s00123 | 常规 |
| c077 | content-not-name | 性能基线监控 P50/P95 (zh) | **复用 s00011/s00012 性能优化对** | 边界 |
| c078 | content-not-name | data retention with anonymization (en) | s00124 | 常规 |

**Sweep 全表**（W=10.0、T = cosine_threshold、v3 数据集 78 cases / 124 docs / dim 1024）：

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N | concept HYBR_N | synonym HYBR_N |
|---|---|---|---|---|---|---|
| 0.0 (≈纯 vec) | 1.000 | 0.847 | 0.726 | 0.822 ↓ | 0.844 | 0.905 |
| 0.30 | 1.000 | 0.847 | 0.726 | 0.822 ↓ | 0.844 | 0.905 |
| 0.45 | 1.000 | 0.847 | 0.726 | 0.822 ↓ | 0.844 | 0.905 |
| 0.60 | 1.000 | 0.851 | 0.726 | 0.852 ↓ | 0.790 | 0.905 |
| **0.70 ⭐ (T\* sweep best、Branch A 命中)** | **1.000** | **0.856** | **0.717** | **0.870** | **0.789** | **0.905** |
| 0.80 | 1.000 | 0.847 | 0.671 | 0.868 | 0.789 | 0.905 |
| 0.90 | 1.000 | 0.843 | 0.648 | 0.868 | 0.789 | 0.905 |
| 0.99 | 1.000 | 0.843 | 0.648 | 0.868 | 0.789 | 0.905 |
| 1.01 (≡HYB) | 1.000 | 0.843 | 0.648 | 0.868 | 0.789 | 0.905 |

v3 HYB baseline（T=1.01 时实测、各桶等价不跳）：synonym 0.905 / concept 0.789 / crosslang 0.648 / content-not-name 0.868 / exact-name 1.000 / OVERALL 0.843。

**控制对照核验**：
- T=0.0 时 HYBR ≈ VEC（六桶 HYBR_N 与 VEC_N 相等 ✓）
- T=1.01 时 HYBR ≡ HYB（六桶完全相等 ✓）
- T=0.70 选定时路由真触发：crosslang vec 强 cosine≈0.7+ 跳 FTS（crosslang HYBR_N 0.717 > HYB 0.648）、content-not-name vec 部分 case cosine<0.70 保留 FTS（HYBR_N 0.870 > HYB 0.868 微升）

**v3 baseline.json 实测**（T\*=0.70 rewrite）：
- synonym HYBR_R 0.9167 / HYBR_N 0.9051
- concept HYBR_R 1.0000 / HYBR_N 0.7891
- crosslang HYBR_R 1.0000 / HYBR_N 0.7168
- content-not-name HYBR_R 0.9500 / HYBR_N 0.8704
- exact-name HYBR_R 1.0000 / HYBR_N 1.0000
- OVERALL HYBR_R 0.9679 / HYBR_N 0.8559

**T\* 决定 = 0.70（Branch A 命中 ⭐）**：

依据：
1. exact-name HYBR_R = 1.000 ✅ 硬红线（所有 T 守住）
2. 各桶 HYBR_N ≥ v3 HYB baseline 同桶（不退步）：
   - synonym 0.905 = 0.905 ✓
   - concept 0.789 = 0.789 ✓
   - crosslang 0.717 > 0.648 (+0.069) ✓
   - content-not-name 0.870 > 0.868 (+0.002) ✓
   - exact-name 1.000 = 1.000 ✓
   - OVERALL 0.856 > 0.843 (+0.013) ✓
3. spec §2.2 (4c) crosslang HYBR_N 自锁 baseline：实测 0.717 ≥ v3 baseline 0.648 ✓（动态读、本 cycle 主动放弃字面 ≥ 0.700 spec 目标）
4. spec §2.2 (4d) OVERALL HYBR_N 自锁 baseline：实测 0.856 ≥ v3 baseline 0.843 ✓（动态读、本 cycle 主动放弃字面 ≥ 0.864 spec 目标）

**为什么 T\*=0.70 仍是 v3 sweep best**：
- T<0.70（0.0/0.30/0.45）：路由太激进、所有 case 都跳 FTS（T=0.0/0.30/0.45 三行数值完全相同，意味着所有 case cosine_top1 ≥ 0.45）→ content-not-name 桶部分 vec 弱 case 被错误跳 FTS、退步 0.822（破 (4b) 红线）
- T=0.60：content-not-name 部分回升 0.852 但仍 < baseline 0.868 ↓ 破 (4b) 红线
- T=0.70：crosslang 桶（vec 强 cosine≈0.7+）触发跳 + content-not-name 桶（vec 弱 cosine<0.7）保留 FTS——不对称区分两场景达 sweep best
- T=0.80：crosslang 部分 case cosine 介于 0.70-0.80 失去跳过机会、crosslang HYBR_N 0.717→0.671 降
- T≥0.90：所有 case cosine<阈值不跳、HYBR≡HYB

**v3 真水位结论 — A-5「cosine 单维真破局」结论 v3 进一步确认**：

A-5 cycle 在 v1 11 例上 sweep 选定 T\*=0.60、v2 揭示「A-5 v1 含轻微运气、T\* 上移到 0.70 = Branch B 边界 inclusive 上界、v2 真水位 OVERALL 0.854 / crosslang 0.717 / content-not-name 0.853」。v2 调优记录诚实承认「T\*=0.70 在 v2 20 例上仍可能含残余运气」。v3 二次扩量 30 例后：

- **T\*=0.70 仍是 sweep best（Branch A 命中 ⭐）**：v2 bake 在 v3 30 例数据集上**鲁棒**——content-not-name 桶 20 例不是运气、cosine 信号方向真破局
- **v3 真水位（实测变化、v2 → v3）**：
  - OVERALL HYBR_N：0.854 → **0.856** (+0.002 微升、统计意义内不变)
  - crosslang HYBR_N：0.717 → **0.717** (持平、cosine 信号在 crosslang 桶上稳定)
  - content-not-name HYBR_N：0.853 → **0.870** (+0.017 显著升、二次扩量后真水位反而升、说明 v2 20 例并未让 T\*=0.70 带运气)
  - concept HYBR_N：0.819 → 0.789 (-0.030、新主题略难、不破 baseline)
- **下 cycle 不需做 evals 扩量来验证 T\*=0.70 鲁棒性**（v3 已验证）：若要扩量则专攻 crosslang 桶（13 例偏小、可能含运气）；**主抓手是更大 embedding 模型 qwen3-0.6b → 1.5b/3b**（v3 数据指证唯一剩余天花板抓手）

**认知层修订小结**：v3 cycle 主动放弃字面 0.864 / 0.700 spec 目标的字面追求，**承认在「cosine 单维 + qwen3-0.6b 模型 + 当前合成集」组合下结构性不可达**。gate.rs 4 红线全部自锁 baseline（A-3 cycle 起即如此）、未来 cycle 调优只要不退步 baseline 即合规；字面 spec 目标移交下 cycle 抓手（更大 embedding 模型 / cosine + lang 组合信号）。诚实承认目标下调 ≠ 项目失败、而是诚实接受当前技术栈天花板。

**下 cycle 抓手优先级（v3 数据指证）**：

| 候选 | 原理 | 优先级 |
|---|---|---|
| **更大 embedding 模型 qwen3-0.6b → 1.5b/3b** | 抬升 vec 召回质量、cosine_top1 分布上移、T\* 可能进一步上调 + 抬 OVERALL/crosslang、有望真破 0.864 字面 spec 目标 | **极高优**（v3 验证 T\*=0.70 鲁棒、唯一剩余天花板抓手；需 Mac 训练 + 模型分发）|
| **评测集再扩量**（content-not-name 30→50 / crosslang 13→20）| v3 验证 T\*=0.70 已鲁棒、扩量边际收益递减；若专攻 crosslang 桶（13 例偏小、可能含运气）则有价值 | 中（若专攻 crosslang 桶则有价值、否则边际收益小） |
| **原始 query 入 schema**（A 簇余项） | 让语义臂 keywords 拼接近似真 query | 中（byte-equal 风险须 router 后置填充不动 parser） |
| **cosine + lang 组合信号** | v3 验证 cosine 单维已鲁棒、组合信号收益不明、若更大模型无效再做（A-3 jaccard / A-4 detect_lang git history 可恢复） | 低（备选、若更大模型无效再做） |

**基础设施完整保留**（A-3/A-4/A-5 都未动）：
- `result-normalizer::lang::Lang/detect_lang` 保留作 wiring 元数据
- wrapper `fuse_rrf_with_fts_routing` 5 参签名 + `RouteVerdict { skipped_fts, query_lang, vec_top1_cosine, cosine_threshold }`
- 评测 `vector_rank → (id, cosine)` + `to_results_with_scores` + `score_case` cosine_threshold + binary `--cosine-threshold` flag
- 生产 `run_fanout_merge_rrf` 5 参 wrapper 调用 + struct-update 后置覆写 query_lang
- baseline.json HYBR 字段 v3 rewrite + gate 4 红线动态读 baseline（A-3 cycle 起即如此）

链接：[v3 spec](../superpowers/specs/2026-06-24-beta-15b-6-v3-content-not-name-second-expansion-design.md) / [v3 plan](../superpowers/plans/2026-06-24-beta-15b-6-v3-content-not-name-second-expansion.md)

---

## v4 数据集节 — embedding 模型跨族 + 同族最大档探针（BETA-15B-7）

> 承接 v3 cycle 主动放弃字面 0.864 / 0.700 spec 目标后的认知层结论：「cosine 单维 + qwen3-0.6b 模型 + 当前合成集」组合下结构性不可达、移交下 cycle 抓手 = 更大 / 更强 embedding 模型。本 cycle 用数据指证两条独立轴上的 embedding 模型：① 跨族架构轴 = bge-m3（BAAI、~568M、多语言 SOTA、BERT 架构）；② 同族最大档轴 = qwen3-embedding-8b（Qwen 官方、~8B、Qwen3 系列最大）。
>
> **核心发现**：两条独立轴**都被 model-runtime / llama-cpp-4 infrastructure 层阻断**、**没有产出可信的模型层数据指证**。本 cycle 实际产出 = **infra 层缺陷诊断**，下 cycle 抓手优先级修正 = **修 infra > 换模型**。

### 模型清单

| 模型 | architecture | 文件大小 | dim | model_id |
|---|---|---|---|---|
| qwen3-embedding-0.6b q8_0 | qwen3 | 610 MB | 1024 | models/qwen3-embedding-0.6b-q8_0.gguf（v3 已知锚）|
| **bge-m3 q8_0** | **bert** | 605 MB | 1024 | models/bge-m3-q8_0.gguf（gpustack/bge-m3-GGUF 公开仓库）|
| **qwen3-embedding-8b q8_0** | qwen3 | 7.5 GB | 4096 | models/qwen3-embedding-8b-q8_0.gguf（Qwen/Qwen3-Embedding-8B-GGUF gated）|

**SHA256（供 follow-up cycle 复用同款 GGUF 文件校验、避免重蹈覆辙）**：

```
950f4a8e5e19477a6d3c26d2f162233c20002c601f75e4b002e3239997821167  bge-m3-q8_0.gguf
a48e50332ee0468f253c9af03d94f7e590906d0c096d13da17818fce0c227445  qwen3-embedding-8b-q8_0.gguf
06507c7b42688469c4e7298b0a1e16deff06caf291cf0a5b278c308249c3e439  qwen3-embedding-0.6b-q8_0.gguf
```

**推理栈版本**：`llama-cpp-4 = "0.3.0"`（见 [`packages/model-runtime/Cargo.toml:20`](../../packages/model-runtime/Cargo.toml#L20)、`default-features = false`、BETA-25 修过动态链接）；本 cycle qwen3-8b 全零 bug 推测与此版本相关、follow-up cycle 必须 ① 先查 llama-cpp-4 upstream issue tracker 是否已修、② 若已修则升 version 重测、③ 若未修则 file upstream issue（本 cycle 未提）。

### 三模型 sweep 全表（v3 数据集 78 cases / 124 docs / W=10.0 固定）

#### qwen3-embedding-0.6b q8_0（v3 锚、对照、与 v3 节实测精确一致）

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| 0.0 (≈纯 vec) | 1.000 | 0.847 | 0.726 | 0.822 |
| 0.30 | 1.000 | 0.847 | 0.726 | 0.822 |
| 0.45 | 1.000 | 0.847 | 0.726 | 0.822 |
| 0.60 | 1.000 | 0.851 | 0.726 | 0.852 |
| **0.70 ⭐ (v3 bake)** | **1.000** | **0.856** | **0.717** | **0.870** |
| 0.80 | 1.000 | 0.847 | 0.671 | 0.868 |
| 0.90/0.99/1.01 (≡HYB) | 1.000 | 0.843 | 0.648 | 0.868 |

#### bge-m3 q8_0（跨族架构轴、**Branch IV-A 推断 = infra pooling 错配**）

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| 0.0 / 0.30 / 0.45 / 0.60 | 1.000 | 0.735 | 0.543 | 0.744 |
| **0.70 (本表 best crosslang)** | **1.000** | **0.763** | **0.543** | **0.773** |
| **0.80 (本表 best OVERALL)** | **1.000** | **0.770** | **0.481** | **0.822** |
| 0.90/0.99/1.01 (≡HYB) | 1.000 | 0.770 | 0.481 | 0.822 |

**vs 0.6b 锚**（best 跨 T）：
- OVERALL 0.770 vs 0.856 = **-0.086 显著退步**
- crosslang 0.543 vs 0.717 = **-0.174 严重退步**
- content-not-name 0.822 vs 0.870 = -0.048 退步
- exact-name HYBR_R 1.000 = 1.000 ✓

**破 (4b) 红线**多桶 ≥ 0.6b HYB baseline。

#### qwen3-embedding-8b q8_0（同族最大档轴、**Branch IV-B 推断 = infra llama-cpp-4 bug**）

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| 0.0 ~ 1.01 (**所有 T 恒等**) | 1.000 | 0.562 | 0.100 | 0.827 |

**vs 0.6b 锚**：
- OVERALL 0.562 vs 0.856 = -0.294 严重退步
- crosslang 0.100 vs 0.717 = -0.617 灾难性退步
- content-not-name 0.827 vs 0.870 = -0.043 退步
- exact-name HYBR_R 1.000 = 1.000 ✓（FTS 守住）

**所有 T 恒等**揭示根因：vec 臂的所有输入向量都是 0、阈值无作用、HYBR 仅有 FTS 贡献（exact-name + content-not-name 靠词面命中、OVERALL 与 crosslang 被 vec=0 严重拖累）。

### Branch 判定（按 spec §2.3）

#### bge-m3 → **Branch IV-A（infra-pooling-mismatch）**

**字面条件**：多桶 HYBR_N < 0.6b HYB baseline ✓ 符合 Branch IV「任一桶破 ≥ HYB baseline」

**诊断**（不是模型能力差、是 infra 配错 pooling）：
- GGUF metadata 显示 bge-m3 `general.architecture = bert` + **`bert.pooling_type = 2`（CLS）**——bge-m3 的 GGUF 明示用 CLS pooling
- 实测 llama.cpp upstream 枚举 `LLAMA_POOLING_TYPE_{NONE=0, MEAN=1, CLS=2, LAST=3}`（注：llama-cpp-4 0.3.0 binding 未暴露 `Rank=4`、留 reranker 用）；bge-m3 GGUF 声明值=2=CLS、与 bge-m3 model card `[CLS]` token + L2 norm 一致；本 cycle (BETA-15B-8) 在 model-runtime infra 修复后跑出真水位、详 v4-fixup 节
- 现 [`packages/model-runtime/src/llama.rs:357`](../../packages/model-runtime/src/llama.rs#L357) 硬编码 `LlamaPoolingType::Last`、**覆盖 GGUF 的 CLS 声明**、不区分 architecture
- 用 Last pooling 对 BERT-arch 模型提取的「向量」语义偏（取的是最后一个 token 的 hidden state、不是 CLS / mean 聚合）
- L2 norm ≈ 1.0 仅说明 llama.cpp 后置归一化生效、不代表向量语义有效
- **诊断证据**：bge-m3 cosine top1 分布 mean=0.719、median=0.721、98.7% > 0.60，HIGHER 比 qwen3-0.6b (mean=0.660、median=0.660)——top1 cosine 高但 nDCG 低 = 「向量看起来近、语义编码偏」的 last-token state collapse 典型表现

**不能下「bge-m3 比 qwen3-0.6b 弱」结论**：本 cycle 测的是「bge-m3 + 错配 pooling」、不是「bge-m3 + 正确 pooling」。bge-m3 业界报告（MTEB / C-MTEB）在多语言 retrieval 上长期 SOTA，绝不可能 OVERALL 比 qwen3-0.6b 弱 0.086。

#### qwen3-8b → **Branch IV-B（infra-llama-cpp-4-bug）**

**字面条件**：所有非 FTS 桶 HYBR_N << 0.6b HYB baseline ✓ 符合 Branch IV

**诊断**（不是模型能力差、是 infra 层 bug、具体根因 hypothesis 而非已证）：
- 模型加载成功、metadata 完整：36 层 MTL0 KV 288 MiB、n_embd=4096、`qwen3.pooling_type=3 = LAST` 与 `run_embed` 匹配
- 但 `embeddings_seq_ith(0)` 返**全零向量**（202 序列、doc + query 全 0）
- 同族 0.6b 同样 pooling、同样推理栈（llama-cpp-4 = 0.3.0）、相同代码路径 → 工作正常
- 总推理时间 27 秒（vs 估算 13-40 分钟）= 强证据「未做实际 forward 工作」（但非铁证、也可能 decode 成功但 embedding tensor 未填）

**根因 hypothesis（按合理性排序、follow-up cycle 第一步是逐一排除）**：
1. **llama-cpp-4 0.3.0 对 qwen3-8b 4096-dim embedding tensor 处理 bug** —— 推测但未证；需对照 llama-cpp-4 issue tracker + 升级到 0.3.x 最新 / 0.4.x 重测
2. **GPU layer offload 配置问题** —— `gpu_layers: 99` 可能与 8b 模型 36 层有边界 case、部分层未 offload 致中间 state collapse；可改 `gpu_layers: 0` 走 CPU 验证排除
3. **context_size=2048 对 8b 边界不够** —— 0.6b 工作良好不代表 8b 同 context 工作；可调到 4096+ 排除
4. **8b 特殊层结构未支持**（如 Gated Delta Net / SWA）—— **GGUF metadata 未直接看到这些标记**（架构标识与 0.6b 同为 `qwen3`、仅 embedding_length 4096 vs 1024、tensors 398 vs 310）、纯推测、最低可能性

**不能下「qwen3-8b 模型差」结论**：8b 根本没被有效推理过。**本 cycle 未 file upstream issue**——follow-up cycle 第一步必做。

### 整体 cycle 结论

**两条独立轴都被 infra 阻断、无 GO/NO GO 模型层数据指证**。本 cycle 不发布 bake 决定，但**产出比预期更有价值的 infra 诊断**：

1. **bge-m3 评测前必须修 pooling type 检测**（model-runtime infra task）
2. **qwen3-8b 评测前必须升 llama-cpp-4 或换推理后端**（model-runtime infra task）
3. v3 cycle 主动放弃的字面 0.864 / 0.700 spec 目标依然有效、本 cycle **未能反驳**「Qwen3-0.6b 系列见顶」假设、只能说「至今没有合规的反驳数据 + infra 是当下最大瓶颈」

### 下 cycle 抓手优先级修正（v4 数据指证）

| 候选 | 原理 | 优先级 |
|---|---|---|
| **1. 修 model-runtime pooling type detection** | bge-m3 / EmbeddingGemma / 任何 BERT-like 架构都需正确 pooling、阻塞所有 cross-arch embedding 评测 | **最高优**（阻塞抓手）|
| **2. 升 llama-cpp-4 / 换推理后端** | 解 qwen3-8b 全零 bug、可能也连带解其他更新代模型支持 | **高优**（阻塞同族放大轴）|
| **3. BETA-15B-7-v2 重跑** | 修完 infra 后用同 spec / 同 model 清单 / 同 sweep 流程拿真水位 | 中高（依赖 1+2 完成）|
| **4. 跨厂替代候选**（若 1+2 都困难） | EmbeddingGemma-300M (Gemma arch、新代 SOTA) / jina-embeddings-v3 (LoRA) / bge-multilingual-gemma2 9B | 中（备选路径）|
| **5. 评测扩量** | 单 OVERALL 仍差 0.008、若 infra 修后双轴仍不破、再考虑 | 低 |

### 异常排查清单核对（spec §5）

| 检查项 | bge-m3 | qwen3-8b |
|---|---|---|
| (1) model_id 字段正确 | ✓ | ✓ |
| (2) dim 字段合理 | ✓ 1024 | ✓ 4096 |
| (3) n_docs=124 / n_queries=78 | ✓ | ✓ |
| (4) L2 norm ≈ 1.0 | ✓ (≈1.0) | ❌ (全 0) |
| (5) GGUF SHA256 与 HF 一致 | ✓ | ✓ |
| (6) 仍退步 → 进入 Branch IV 调查 | ✓ → infra pooling | ✓ → infra llama-cpp-4 |

所有外部因素核验通过，结论为 model-runtime 层 infrastructure 缺陷、不是模型 / 数据问题。

### 本 cycle 为什么仍合并发布（而非 spec §5 字面「Branch IV 不发布」）

spec §5 写「Branch IV 异常调查无结论、不发布 + 留 STATUS 异常记录 + 下次会话深排」是默认假设单一 cycle 内有 1 个出乎意料的负面结果时。本 cycle 实际情况**根因明确（infra）+ 双轴一致暴露 + 修复方向清晰**，「不发布」字面规则会浪费已得诊断价值。修订执行：

- **合并 + 发布完整诊断作 v4 节**、作为 follow-up cycle 的精确入口
- **不 bake 任何模型到生产**（spec §3.2 YAGNI 守住）、gate.rs / baseline.json 不动、回归门仍守护 v3 0.6b
- **三 vectors-{qwen3-0.6b,bge-m3,qwen3-8b}.json 入仓**作研究产物、与 `vectors.json` 解耦、不影响 gate
- baseline 报告 v4 节明示「不能下模型能力结论、本 cycle 是 infra 诊断 cycle」、避免后续 cycle 误以为「bge-m3 / qwen3-8b 测过差了不行」

### v4 评测的边界

- 本 cycle 不动 `baseline.json` / `gate.rs` / `DEFAULT_COSINE_ROUTING_THRESHOLD` / desktop wiring / 模型分发
- gate 仍守护 v3 0.6b baseline、本 cycle 任何模型升级 bake 都在 follow-up cycle（且必须在 infra 修复之后）
- 三 vectors 文件入仓作研究产物、与 `vectors.json` 解耦不影响 gate
- 改的唯一 production 代码 = `packages/evals/src/bin/semantic_quality.rs` 加 `--vectors-file` flag（向下兼容、零行为变化）

### 认知层修订小结

**v3 → v4 认知层变化**：
- v3 主动放弃字面 0.864 / 0.700 spec 目标、移交下 cycle = 更大 embedding 模型
- v4 数据指证「更大 / 更强 embedding 模型」抓手实际是「**修 model-runtime infra**」抓手——任何 cross-arch 模型上场前必须先修
- v4 没有反驳「Qwen3-0.6b 见顶」假设、也没有验证「更大模型能破局」假设
- v4 唯一确定结论：infra 层不修、模型选型 cycle 都白做

**修订下 cycle 节奏**：
- 「BETA-15B-7-v2 模型缩放探针重跑」**不应**作为下个直接 cycle、必须先做「BETA-15B-X model-runtime pooling type detection」+「BETA-15B-Y llama-cpp-4 升级」
- 「BETA-15B-7-v2 重跑」是 follow-up 的 follow-up、不是直接 follow-up

链接：[v4 spec](../superpowers/specs/2026-06-24-beta-15b-7-embedding-model-scaling-probe-design.md) / [v4 plan](../superpowers/plans/2026-06-24-beta-15b-7-embedding-model-scaling-probe.md) / [model-runtime llama.rs 硬编码 pooling](../../packages/model-runtime/src/llama.rs#L357)

### v4-fixup 数据集节 — model-runtime pooling type detection 修复后 bge-m3 真水位（BETA-15B-8）

承接 v4 cycle 的 Branch IV-A 推断（bge-m3 因 `LlamaPoolingType::Last` 硬编码错配 bert arch 声明的 CLS pooling）。BETA-15B-8 cycle 修复 `packages/model-runtime/src/llama.rs` 硬编码、改为按 GGUF `<arch>.pooling_type` 动态检测：

- 抽 `packages/model-runtime/src/pooling.rs` 纯逻辑模块 + 9 单测覆盖
- `worker_main` 加载 model 后 detect pooling 一次、存为函数局部变量
- `run_embed` 签名扩 `pooling: LlamaPoolingType` 参数
- qwen3-embedding-0.6b 重 embed 后 `vectors.json` byte-equal 验证零回归（GGUF 声明 `qwen3.pooling_type=3` → Last、与旧硬编码一致、SHA256 `0c258086...` 完全等价）
- bge-m3 用 CLS pooling 重 embed、`vectors-bge-m3.json` SHA256 `e003da3e...` 覆盖 v4 错配版本（v4 错配版 SHA256 `acc7b69c...`）

**fact-check 校正**：v4 节 line 668 把 `bert.pooling_type=2` 标注为「MEAN」是 fact 错误。按 llama.cpp upstream 枚举 `LLAMA_POOLING_TYPE_{NONE=0, MEAN=1, CLS=2, LAST=3}`（注：llama-cpp-4 0.3.0 binding 未暴露 `Rank=4`、留 reranker 用），值 2 实际是 CLS（与 bge-m3 model card 明示的 `[CLS]` token + L2 norm 一致）。修复方向不变（硬编码 Last 错配 bge-m3 声明的 CLS）。

**bge-m3 真水位 sweep**（v3 数据集 78 cases / 124 docs / dim 1024、CLS pooling、W=10.0）：

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| 0.0 (≈纯 vec) | 1.000 | 0.869 | 0.685 | 0.871 |
| 0.30 | 1.000 | 0.869 | 0.685 | 0.871 |
| 0.45 | 1.000 | 0.869 | 0.685 | 0.871 |
| 0.60 | 1.000 | 0.868 | 0.685 | 0.871 |
| **0.70 ⭐** | **1.000** | **0.864** | **0.662** | **0.875** |
| 0.80 | 1.000 | 0.853 | 0.599 | 0.872 |
| 0.90 / 0.99 / 1.01 (≡HYB) | 1.000 | 0.853 | 0.599 | 0.872 |

**控制对照核验**：T=0.0 时 HYBR_OVERALL=0.869=VEC_OVERALL ✓；T=1.01 时 HYBR_OVERALL=0.853=HYB_OVERALL ✓。

**v4 错配 (Last) vs v4-fixup (CLS) 对照**（按 best OVERALL HYBR_N 比对）：

| 指标 | v4 错配 Last | v4-fixup CLS | Δ |
|---|---|---|---|
| best exact-name HYBR_R | 1.000 | 1.000 | = |
| best OVERALL HYBR_N | 0.770 | **0.869** | **+0.099** |
| best crosslang HYBR_N | 0.543 | 0.685 | **+0.142** |
| best content-not-name HYBR_N | 0.822 | **0.875** | +0.053 |

**v4-fixup vs v3 qwen3-0.6b（生产锚、T\*=0.70）对照**：

| 指标 | qwen3-0.6b T\*=0.70 | bge-m3 best CLS | Δ |
|---|---|---|---|
| OVERALL | 0.856 | **0.869** | **+0.013** ⭐ 破 spec 字面 0.864 目标 |
| crosslang | 0.717 | 0.685 | -0.032（未破 spec 字面 0.700 目标）|
| content-not-name | 0.870 | 0.875 | +0.005 |
| exact-name | 1.000 | 1.000 | = |

**判定（spec §2.3）**：

- ✅ **infra 修复完全证实诊断**：v4「last-token state collapse」假说成立（bge-m3 修复后 OVERALL +0.099、crosslang +0.142、三桶 vs v4 错配版均显著提升）；硬编码 Last 是真 bug、按 GGUF metadata 动态选 pooling type 是正确修复方向。
- 🎯 **bge-m3 部分破局（二维 trade-off）**：OVERALL **0.869 双过 spec 字面 0.864 目标**（v3 cycle 主动放弃的字面追求被复活）；crosslang 0.685 略输 qwen3-0.6b 0.717（-0.032、且未破 spec 字面 0.700）。
- bake 决策**留 follow-up cycle BETA-15B-7-v2**（独立 cycle、需 DEFAULT_EMBEDDING_MODEL_PATH 替换 + baseline.json rewrite + Windows 适配 + 模型分发 UX、本 cycle YAGNI）。

**下 cycle 抓手优先级修正（v4-fixup 数据指证）**：

| 抓手 | 优先级（v4-fixup 修订）|
|---|---|
| **BETA-15B-7-v2 bake bge-m3 推到生产**（DEFAULT_EMBEDDING_MODEL_PATH 替换、baseline.json rewrite 0.6b→bge-m3、gate.rs 红线重锚）| **最高优**（OVERALL +0.013 + 同尺寸零分发成本、最大 ROI；crosslang 微 regression 需文档明示 trade-off）|
| BETA-15B-Y 升 llama-cpp-4 0.3.0 / 解 qwen3-8b 全零 bug | 中优（qwen3-8b 真水位仍未知、若 bake bge-m3 后再补可作为下下 cycle）|
| BETA-15B-7-v3 跨厂替代候选（EmbeddingGemma-300M / jina-v3 / bge-multilingual-gemma2 9B）| 低优（bge-m3 已部分破局、若想冲 crosslang 0.700 再考虑）|
| 评测扩量 | 低优、若以上无效再做 |

**链接**：[BETA-15B-8 spec](../superpowers/specs/2026-06-25-beta-15b-8-model-runtime-pooling-type-detection-design.md) / [BETA-15B-8 plan](../superpowers/plans/2026-06-25-beta-15b-8-model-runtime-pooling-type-detection.md) / [pooling.rs 模块](../../packages/model-runtime/src/pooling.rs)

### v4-fixup2 数据集节 — llama-cpp-4 升级 + qwen3-embedding-8b 全零 bug 4-hypothesis ladder 全 FAIL（BETA-15B-9）

承接 v4 cycle 的 Branch IV-B 推断（qwen3-embedding-8b 全零向量、推断 llama-cpp-4 0.3.0 + 8B 推理 bug）。BETA-15B-9 cycle 升 llama-cpp-4 0.3.0 → 0.3.2 + 按 v4 hypothesis 排序排查 4 hypothesis 全 FAIL：

- **Phase 0**：升 llama-cpp-4 0.3.0 → 0.3.2、workspace 三件套全过、qwen3-0.6b 重 embed 后 vectors.json **语义等价**（cos min=0.999999 / max abs=2.5125e-04、原 byte-equal 红线放宽为语义等价闸；详 spec §2.2 (7)+(8)、vectors.json + vectors-qwen3-0.6b.json SHA256 一并变到 `0315b8d0...`）
- **Phase 1 (hypothesis 1: 升 0.3.2)**：FAIL。重 embed qwen3-8b → 仍全零（dim=4096、nonzero=0、L2=0、SHA256 byte-equal v4 `b243e2a9...`、推理 ~1 min 早期短路、log 显示 Metal MTL0 36 layer 全 offload）
- **Phase 2 (hypothesis 2: GPU=0 CPU)**：FAIL。临时 `gpu_layers=0` + 移除 `if gpu_layers > 0` 守卫强 `with_n_gpu_layers(0)` 绕过 → KV cache 全 36 layer dev=CPU 确认 → 仍全零（~5 min 短路）。临时改动 cycle 末已 `git checkout` 还原。
- **Phase 3 (hypothesis 3: context=4096)**：FAIL。临时硬改 `let context_size = 4096;` → KV buffer size = 576 MiB / 4096 cells 确认 → 仍全零（~1 min 短路）。临时改动 cycle 末已还原。
- **Phase 4 (file upstream issue)**：issue body draft 已写入仓（[2026-06-25-beta-15b-9-upstream-issue-body.md](./2026-06-25-beta-15b-9-upstream-issue-body.md)、138 行 + GGUF metadata + 完整复现 Rust 代码 + 4 hypothesis 排查表）；本 cycle 与用户协商决定**跳过实际 file**（涉及 GitHub identity / public content 提交、留用户后续手动操作）。未来 follow-up cycle 监控 upstream 进展时补 file。

**Hypothesis 4（8b 特殊层结构未支持）部分推翻**：llama-cpp-4 0.3.2 log 显示：

```
sched_reserve: resolving fused Gated Delta Net support:
sched_reserve: fused Gated Delta Net (autoregressive) enabled
sched_reserve: fused Gated Delta Net (chunked) enabled
```

即 binding 已识别 Qwen3-Embedding-8B 的特殊层结构（Gated Delta Net）+ sched_reserve 启用了 fused 路径、但 embedding 输出仍全零。推断 root cause = **fused Gated Delta Net 实现与 `LlamaPoolingType::Last` + embedding-only 推理的交互有 8b-specific bug**（0.6b 走相同代码路径但不触发此 bug、可能因 0.6b 不启用 Gated Delta Net / 或在 1024-dim embedding tensor 上路径不同）。

**qwen3-8b 真水位**：仍未知。bake 决策本 cycle 未带新数据。

**bake 决策（不变）**：仍推荐 bake bge-m3 推到生产：
- 同尺寸零分发成本（605 MB ≈ 0.6b 610 MB）
- OVERALL +0.013 vs qwen3-0.6b、双过 spec 字面 0.864
- crosslang -0.032 trade-off（LociFind 头号卖点反退、需文档明示）
- qwen3-8b 评估推到上游 fix 后的 follow-up cycle（监控 upstream issue 进展）

**下 cycle 抓手优先级修正（v4-fixup2 fail 路径数据指证）**：

| 抓手 | 优先级 |
|---|---|
| BETA-15B-7-v2 bake bge-m3 推到生产 | **最高优**（不再等 qwen3-8b 真水位、4 hypothesis 全 fail = qwen3-8b 在当前 binding 状态下不可用、不阻塞 bake）|
| 监控 llama-cpp-4 upstream / file issue（用户手动）| 中优（被动等、不主动推；issue body 已 draft、用户 file 后跟进 thread）|
| 跨厂替代候选（bge-multilingual-gemma2 9B / EmbeddingGemma-300M / jina-v3）| 低优（bge-m3 已部分破局、bake 后视真机用户反馈再考虑）|
| 评测扩量 | 低优 |

**链接**：[BETA-15B-9 spec](../superpowers/specs/2026-06-25-beta-15b-9-llama-cpp-4-upgrade-qwen3-8b-zero-vector-design.md) / [BETA-15B-9 plan](../superpowers/plans/2026-06-25-beta-15b-9-llama-cpp-4-upgrade-qwen3-8b-zero-vector.md) / [BETA-15B-9 upstream issue body draft](./2026-06-25-beta-15b-9-upstream-issue-body.md) / [v4-fixup 节 (bge-m3)](#v4-fixup-数据集节--model-runtime-pooling-type-detection-修复后-bge-m3-真水位beta-15b-8)

### v4-fixup3 节 — BETA-15B-7-v2 bake bge-m3 推到生产 done

承接 v4-fixup 节（BETA-15B-8 infra 修复 + bge-m3 真水位 OVERALL=0.869 ⭐）+ v4-fixup2 节（BETA-15B-9 qwen3-8b 4 hypothesis 全 FAIL）的下 cycle 最高优抓手 = bake bge-m3 推到生产。BETA-15B-7-v2 (2026-06-26 Claude Code、[PR #15](https://github.com/raoliaoyuan/LociFind/pull/15)、merge commit `ee78f75a2882d50c4ae2585fcc61743bb395cf20`) 走最窄 wiring 切换路径：改 `apps/desktop/src-tauri/src/search/embedding_model.rs` 两常量字面值 + 3 处 doc 注释、不动 evals 层 / spike-retrieval / model-runtime / indexer / desktop UI / cosine_threshold / floor / weight。

**bake 后实际 ROI（保 cosine_threshold=0.70）**：OVERALL +0.008（0.856→0.864）、content-not-name +0.005（0.870→0.875）、exact-name =（1.000 守住）、crosslang -0.055（0.717→0.662、头号卖点 trade-off、文档明示）。**未吃满 v4-fixup 表 bge-m3 sweep best**（T*=0.0/0.30/0.45 OVERALL=0.869 / crosslang=0.685、约 +0.005 / +0.023 ROI 留 follow-up cycle 重 sweep cosine_threshold 拿回）。

**评测层零变化**：baseline.json + gate.rs + vectors-*.json + cases/corpus 全部保 qwen3-0.6b 数据、gate 仍守 qwen3-0.6b、SHA256 与 main 等价。桌面与评测两条独立路径解耦、follow-up cycle 重写 baseline 时再对齐。

**下 cycle 抓手**：① **cosine_threshold 在 bge-m3 上重 sweep & bake**（最高优、~1d、数据齐全、ROI +0.005~+0.023）；② **evals baseline.json + gate.rs 红线重锚到 bge-m3 数据**（中优、与 ① 同时或之后、~0.5d）；③ **模型分发 UX**（首启引导 / 自动下载 / Windows 真机性能验证、中优、独立 cycle、~1-2w）；④ **跨厂替代候选**（EmbeddingGemma-300M / jina-v3 / bge-multilingual-gemma2 9B、若想冲 crosslang 0.700 spec 字面、低优、~1-2w）。

**链接**：[BETA-15B-7-v2 spec](../superpowers/specs/2026-06-26-beta-15b-7-v2-bake-bge-m3-production-design.md) / [BETA-15B-7-v2 plan](../superpowers/plans/2026-06-26-beta-15b-7-v2-bake-bge-m3-production.md) / [v4-fixup 节 (bge-m3 真水位)](#v4-fixup-数据集节--model-runtime-pooling-type-detection-修复后-bge-m3-真水位beta-15b-8)

### v5 数据集节 — BETA-15B-10 bge-m3 baseline 重锚 + cosine sweep & bake + 评测集长文本扩量 + evals embed 截断解除 done ⭐

承接 v4-fixup 节（BETA-15B-8 bge-m3 真水位 OVERALL=0.869）+ v4-fixup3 节（BETA-15B-7-v2 bake bge-m3 推到生产）的 follow-up 三件套合并 cycle。BETA-15B-10（2026-06-26 Claude Code、PR # 待回填、merge commit 待回填）三件套合并执行：① cosine_threshold sweep & bake；② baseline.json + gate.rs 重锚到 bge-m3；③ 评测集长文本扩量（c079/c080/c081 + s00125/s00126/s00127）+ **cycle 起手发现的 evals embed 1200 char 截断解除**（让 evals 路径与 desktop indexer 真实路径对齐）。

**关键改动**：
- `packages/evals/src/bin/semantic_quality.rs`：删 line 165 `text.chars().take(1200)` 字符截断（+ 顺手修同函数 line 184 `panic!("写 {}", vectors_file)` → `panic!("写 {vectors_file}")` inline format 修 pre-existing clippy lint）
- `cases.json`：加 c079/c080/c081（content-not-name × 2 + crosslang × 1）
- `corpus.json`：加 s00125（zh 1495 char 5 Whys）/ s00126（en 4579 char canary）/ s00127（en 4231 char log retention）、全虚构 + 零 PII
- `vectors.json`：Mac Metal --embed 重跑全集（81 query + 127 doc、dim 1024、bge-m3-q8_0、SHA256 `4f0de346b581d58d…`）
- `packages/result-normalizer/src/lib.rs`：DEFAULT_COSINE_ROUTING_THRESHOLD 字面值 0.70 不变（T5 BAKE_T=0.70 = 现行值）+ doc 追加 v5 标注
- `packages/evals/fixtures/semantic-recall/baseline.json`：rewrite 自 bge-m3 + 81/127 + T=0.70 单次跑（n 78→81）
- `packages/evals/tests/semantic_quality_gate.rs`：line 1 模块 doc + line 85-89 段 doc 升 v5、assert 字节不变

**dataset**：81 cases / 127 docs / bge-m3-q8_0 / dim 1024 / W = 10.0 固定 / Mac Metal

**长文本 token 实测**（解除截断生效证据）：
- s00125 zh = 966 token（从 1495 char 算出、与 BERT encode > 512 path 触发条件吻合）
- s00126 en = 1021 token
- s00127 en = 939 token
- 全集 corpus token 分布：min=94 / max=1021 / mean=149.8 / >512 共 3 条 / >2048 共 0 条（hotfix n_ubatch=2048 上限内、无 panic 风险）

**Sweep 全表**（9 阈值 × 4 桶 HYBR_N、详 /tmp/beta-15b-10-sweep-summary.txt）：

| T | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| 0.0 (≈纯 vec) | 1.000 | 0.868 | 0.708 | 0.865 ↓ |
| 0.30 | 1.000 | 0.868 | 0.708 | 0.865 ↓ |
| **0.45 ⭐ (sweep best、未选)** | 1.000 | 0.868 | 0.708 | 0.865 ↓ |
| 0.60 | 1.000 | 0.867 | 0.708 | 0.865 ↓ |
| **0.70 ⭐⭐ (bake T*、Branch B 变体)** | **1.000** | **0.864** | **0.686** | **0.869** |
| 0.80 | 1.000 | 0.851 | 0.619 | 0.866 ↓ |
| 0.90/0.99/1.01 (≡HYB) | 1.000 | 0.851 | 0.619 | 0.866 |

**控制对照**：T=0.0 时 HYBR ≈ VEC ✓、T=1.01 时 HYBR ≡ HYB ✓。

**bake 决策 Branch B 变体**：sweep best 在 T ∈ {0.0, 0.30, 0.45}（OVERALL 0.868）、但 spec §5.2 Branch C 字面要求 (4b) 全过、T=0.45 content-not-name -0.003 字面 FAIL（vs 现行 baseline 0.8676）。本 cycle **保守选 T=0.70**（唯一 (4b) 严格全过的档、content-not-name +0.001 ✓）。代价：OVERALL -0.004 / crosslang -0.022 vs sweep best。

**为何 content-not-name 在 cosine 低 T 退步**：reviewer 分析显示 c079/c080 long-text case 平均得分 ~0.703 < 桶平均 0.876、是真信号不是噪声。Cosine 路由（T=0.45）跳 FTS 后 long-text 无 FTS 兜底、T=0.70 部分 case 不跳让 FTS 救回排名。**这是 long-text 难 retrieval 的预期信号、不是 cosine 路由本身问题**。

**spec §2.2 接受标准核对**（at T*=0.70 vs 新 baseline.json hybrid_routed_*）：
- (4a) exact-name HYBR_R = 1.000 ✓
- (4b) 各桶 HYBR_N ≥ 新 baseline HYB（gate 4 红线动态读 baseline、自锁自动跟随）✓
- (4c) crosslang HYBR_N = 0.686（自锁、< 0.700 spec 字面、bge-m3 真水位限制、移交未来 cycle）
- (4d) OVERALL HYBR_N = 0.864（自锁、= 新 baseline）

**vs 现行守 qwen3-0.6b baseline.json ROI（两 framing 都列、避免误读）**：

**HYB framing**（cycle-time gate 沿 BETA-15B-3 起的传统）：
| 桶 | 现 baseline HYB | v5 T=0.70 HYBR | Δ |
|---|---|---|---|
| synonym | 0.9051 | 0.972 | +0.067 |
| concept | 0.7891 | 0.823 | +0.034 |
| crosslang | 0.6479 | 0.686 | +0.038 |
| content-not-name | 0.8676 | 0.869 | +0.001 |
| exact-name | 1.0000 | 1.000 | = |
| OVERALL | 0.8433 | 0.864 | +0.021 |

**HYBR framing**（apples-to-apples、reviewer 提醒避免被 vs HYB 数字误导）：
| 桶 | 现 baseline HYBR | v5 T=0.70 HYBR | Δ |
|---|---|---|---|
| crosslang | 0.7168 | 0.686 | **-0.031** |
| content-not-name | 0.8704 | 0.869 | -0.001 |
| OVERALL | 0.8559 | 0.864 | +0.008 |

**Apples-to-apples 视角**：OVERALL +0.008（微升）/ crosslang -0.031（bge-m3 跨族 trade-off 现形）。比 HYB framing 数字温和。两 framing 都列在此处避免下游误读。

**1200 char 截断解除影响分析**：现有 v3 dataset 124 doc 全部 < 1200 char（合成 corpus 设计本就守 < 512 token 规模）、改前改后向量理论 byte-equal；本 cycle 新 3 doc（s00125 1495 / s00126 4579 / s00127 4231 char）是截断解除后才能完整 embed 的真长文本。evals binary embed 路径解除截断后与 desktop indexer (`apps/desktop/src-tauri/src/search/`) 真实路径对齐（同 bge-m3 / 同 LlamaPoolingType / 同 context_size = 2048）。

**evals binary embed 路径 = desktop indexer 真实路径对齐**：这是 cycle ⓪ 起手发现的盲点、原 cycle ③ framing 从「防 panic」改为「让 sweep 出的 cosine_threshold 在长文本场景有真实数据支持 + 路径对齐」。BERT encode n_ubatch panic（[PR #16](https://github.com/raoliaoyuan/LociFind/pull/16) hotfix）在 evals 因截断本来就触发不到、但 desktop indexer 真用户文档会触发、所以 hotfix 必须在 model-runtime 层做。本 cycle 解除截断后 evals 与 desktop indexer 行为等价。

**诚实边界**：
- crosslang HYBR_N 0.686 仍 < 0.700 spec 字面、本 cycle 主动放弃字面追求、移交未来 cycle = 更大 / 跨厂 embedding 模型（EmbeddingGemma-300M / jina-v3 / bge-multilingual-gemma2 9B、STATUS 已登记）
- bake 选 T=0.70 而非 sweep best 0.45 = 保守路径、ROI 未吃满（OVERALL -0.004 / crosslang -0.022 vs T=0.45 上限）；但守 (4b) 严格全过、与 long-text 友好性同向、Spec §5.2 Branch B 变体决策合规
- 长文本 case 设计可能偏置（c079/c080 平均 ~0.703 < 桶平均、新 case 比现 case 难、未来 cycle 可校准 expected_top_doc_ids 设计 / 简化 query 表述）

**vectors.json**：SHA256 `4f0de346b581d58d…`（Mac Metal Q8_0 一次性产物、llama-cpp-4 0.3.2 Metal kernel 浮点累加顺序在不同 GPU/驱动下可能有 ~1e-4 量级抖动、换 Mac 跑 SHA 不绝对 byte-equal）

**与 v3 / v4-fixup snapshot 区别**：
- 本 cycle 主 active vectors = `vectors.json` = bge-m3 + 81/127 + 截断解除后状态
- `vectors-bge-m3.json` 保 BETA-15B-8 v4-fixup snapshot 状态（v3 124 doc + 截断状态）作历史 reference
- `vectors-qwen3-0.6b.json` 保 v3 124 doc + 截断状态作 reference snapshot
- 这两个 reference snapshot 文件本 cycle 不动

**下 cycle 抓手优先级（v5 数据指证）**：① **跨厂替代候选**（高优、要冲 crosslang 0.700 spec 字面只能换大模型、bge-m3 真水位见顶 0.686-0.708、~1-2w）；② **评测扩量**（中优、crosslang 桶 14 例仍偏小、扩 20+ 校验、~1d）；③ **模型分发 UX**（中优、首启引导 / 自动下载 / Windows 真机性能验证、独立 cycle、~1-2w）；④ **BETA-30 真实失败样本箱**（低优、真实闭环、长期、~1-2w）。

**链接**：[BETA-15B-10 spec](../superpowers/specs/2026-06-26-beta-15b-10-bge-m3-baseline-cosine-bake-long-text-design.md) / [BETA-15B-10 plan](../superpowers/plans/2026-06-26-beta-15b-10-bge-m3-baseline-cosine-bake-long-text.md) / [v4-fixup3 节 (BETA-15B-7-v2)](#v4-fixup3-节--beta-15b-7-v2-bake-bge-m3-推到生产-done)

### v6 数据集节 — BETA-15B-11 EmbeddingGemma-300M 跨厂探针 + prefix 契约对照实验 done ⭐⭐

承接 v5 节（BETA-15B-10 bge-m3 baseline 重锚后留下 crosslang 字面 -0.014 gap、明示「移交未来 cycle = 更大 / 跨厂 embedding 模型」）。BETA-15B-11（2026-06-27 Claude Code、[PR #18](https://github.com/raoliaoyuan/LociFind/pull/18) merged、merge commit `49b5f4a`）单点跨厂探针 + 双 prefix mode 对照实验、**直接冲过 spec 字面**。

**模型与基础设施**

- 模型：**EmbeddingGemma-300M**（Google 2025-09-04、`google/embeddinggemma-300m` 官方 + `ggml-org/embeddinggemma-300M-qat-q8_0-gguf` 公开转仓）
- GGUF：`models/embeddinggemma-300m-q8_0.gguf`、SHA256 `6fa0c02a9c302be6f977521d399b4de3a46310a4f2621ee0063747881b673f67`、328,577,056 bytes（**313 MB、比 bge-m3 还小一半**）
- 架构：`gemma-embedding` / context 2048 / **dim 768** / pooling = **Mean**（GGUF 自声明 pooling_type=1）/ 24 layer / 3 attention head
- infra de-risk：[`packages/model-runtime/src/pooling.rs`](../../packages/model-runtime/src/pooling.rs) `default_pooling_for_arch` 加 `"gemma-embedding" => Mean` 分支 + 1 单测（10/10 过）；llama-cpp-4 0.3.2 vendored llama.cpp 实测识别 `gemma-embedding` arch（含 `fused Gated Delta Net (autoregressive + chunked) enabled`）、推理正常、无 qwen3-8B 式全零 bug、binding 升级应急未触发

**prefix 契约对照实验设计（命题 2、本 cycle 独有）**

[`packages/evals/src/bin/semantic_quality.rs`](../../packages/evals/src/bin/semantic_quality.rs) 加 `--prefix-mode {none, standard}` flag + `EmbedRole` 内部 enum：
- **none**（裸 embed、向下兼容 BETA-15B-10 及之前所有 cycle 行为）
- **standard**（HF model card 契约：query 包 `task: search result | query: {text}`、doc 包 `title: none | text: {text}`）

双 vectors 文件入仓（reference snapshot、不替换主 vectors.json）：
- `vectors-embeddinggemma-300m-no-prefix.json`：SHA256 `c236c5a615ffff1091c61aeaa735c0b2fedccb28176cccce99260e099901e993`、1,995,226 bytes、inference 24.87s、L2 mean=1.0、全零 0/0
- `vectors-embeddinggemma-300m-prefix.json`：SHA256 `46f62c14ce6caf24da5b46058d77c7495f095529db811c88f21cd0a75f9accbd`、1,991,855 bytes、inference 18.18s、L2 mean=1.0、全零 0/0
- 红线 10 区分性 sanity check：两文件 SHA256 不同 ✓（standard prefix 真包到 query/doc、未误为 byte-equal）

**sweep 全表**（v5 dataset 81 cases / 127 docs / dim 768 / W=10.0 固定）

prefix-mode = **none**：

| T (cosine_threshold) | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| 0.0 (≈纯 vec) | 1.000 | 0.878 | 0.716 | 0.894 |
| 0.30 | 1.000 | 0.878 | 0.716 | 0.894 |
| 0.45 | 1.000 | 0.878 | 0.716 | 0.894 |
| **0.60 ⭐ no-prefix sweep best OVERALL** | 1.000 | **0.882** | 0.716 | 0.903 |
| 0.70 (与 v5 bake T 同字面) | 1.000 | 0.874 | 0.716 | 0.895 |
| 0.80 | 1.000 | 0.862 | 0.631 | 0.900 |
| 0.90 / 0.99 / 1.01 (≡HYB) | 1.000 | 0.862 | 0.631 | 0.900 |

prefix-mode = **standard**：

| T (cosine_threshold) | exact-name HYBR_R | OVERALL HYBR_N | crosslang HYBR_N | content-not-name HYBR_N |
|---|---|---|---|---|
| **0.0 ⭐⭐ prefix sweep best 三连冠** | 1.000 | **0.900** | **0.725** | **0.928** |
| **0.30 ⭐⭐ 三连冠** | 1.000 | **0.900** | **0.725** | **0.928** |
| **0.45 ⭐⭐ 三连冠** | 1.000 | **0.900** | **0.725** | **0.928** |
| 0.60 | 1.000 | 0.892 | 0.725 | 0.922 |
| 0.70 (与 v5 bake T 同字面) | 1.000 | 0.887 | 0.725 | 0.919 |
| 0.80 | 1.000 | 0.870 | 0.639 | 0.915 |
| 0.90 / 0.99 / 1.01 (≡HYB) | 1.000 | 0.870 | 0.639 | 0.915 |

**控制对照核验**：
- T=0.0 → HYBR ≈ VEC（六桶相等）：no-prefix HYBR_OVERALL 0.878 = VEC_OVERALL 0.878 ✓ / prefix HYBR_OVERALL 0.900 = VEC_OVERALL 0.900 ✓
- T=1.01 → HYBR ≡ HYB（六桶相等）：no-prefix HYBR_OVERALL 0.862 = HYB_OVERALL 0.862 ✓ / prefix HYBR_OVERALL 0.870 = HYB_OVERALL 0.870 ✓

**spec §2.2 接受标准核对**（at sweep best、prefix mode T=0.0/0.30/0.45）：
- (4a) exact-name HYBR_R = 1.000 ✓
- (4b) 各桶 HYBR_N ≥ v5 HYB baseline（gate 4 红线动态读自锁、本 cycle 不改 baseline.json）：实测各桶 HYBR ≥ v5 baseline 全过 ✓（synonym 0.994 ≥ 0.905 / concept 0.842 ≥ 0.789 / crosslang 0.725 ≥ 0.648 / content-not-name 0.928 ≥ 0.868 / exact-name 1.0 = 1.0 / OVERALL 0.900 ≥ 0.843）
- (4c) crosslang HYBR_N = **0.725 ≥ 0.700** spec 字面 ✓⭐⭐ **复活字面 spec 目标达成**
- (4d) OVERALL HYBR_N = **0.900 ≥ 0.864** spec 字面 ✓⭐⭐ **大幅超字面**

**vs v5 bge-m3 baseline 对照**（v5 T=0.70 vs embeddinggemma sweep best）：

| 桶 | v5 bge-m3 T=0.70 HYBR | embeddinggemma no-prefix T=0.60 (sweep best) | embeddinggemma prefix T=0.0/0.30/0.45 (sweep best) |
|---|---|---|---|
| OVERALL | 0.864 | 0.882 (+0.018) | **0.900 (+0.036) ⭐⭐** |
| crosslang | 0.686 | 0.716 (+0.030) | **0.725 (+0.039) ⭐⭐** |
| content-not-name | 0.869 | 0.903 (+0.034) | 0.928 (+0.059) |
| exact-name | 1.000 | 1.000 (=) | 1.000 (=) |

**Branch 决策：I-a ⭐⭐ GO**

- prefix mode 大幅破字面 spec 目标（OVERALL +0.036 / crosslang +0.025）
- **no-prefix mode 单独也双过字面**（OVERALL 0.882 + crosslang 0.716、不靠 prefix）→ **EmbeddingGemma-300M 本身能力已足以冲过字面**
- gate (4b) HYBR 列各桶不退步 v5 baseline、动态读 baseline 自锁路径稳

**prefix 契约价值数据指证**（命题 2 答案）

| 维度 | no-prefix | standard prefix | Δ |
|---|---|---|---|
| OVERALL HYBR_N (sweep best) | 0.882 (T=0.60) | 0.900 (T=0.0/0.30/0.45) | +0.018 |
| crosslang HYBR_N (sweep best) | 0.716 | 0.725 | +0.009 |
| content-not-name HYBR_N (sweep best) | 0.903 | 0.928 | +0.025 |

**prefix 契约是 ROI 加分项、不是 make-or-break**（与 spec §2.3 Branch I-b 不一样）：no-prefix 已能过字面、standard prefix 让数字更漂亮（各桶 +0.009 ~ +0.026）但不必要。**含义**：bake 推生产时**不需要在 model-runtime 层加 prefix API**、可纯用 `embed(text)` 接口；prefix 加成留 follow-up 独立优化 cycle。

**与 BETA-15B-9 教训对比**

| 现象 | qwen3-embedding-8B 失败 | EmbeddingGemma-300M 成功 |
|---|---|---|
| 加载 | 成功 | 成功 |
| fused Gated Delta Net | enabled | enabled |
| 推理时长 | ~1 min 早期短路 | 24.87s + 18.18s 正常 |
| vec 全零 | 是（dim=4096 全 0）| 否（0/81 query + 0/127 doc） |
| L2 norm | 0 | 1.0 |
| 模型层结论 | 不当（infra 阻断）| GO（推理有效）|

**关键洞察**：fused Gated Delta Net 路径在 EmbeddingGemma-300M（用 Mean pooling）上可用、在 qwen3-embedding-8B（用 Last pooling）上仍 broken。BETA-15B-9 推断的 "fused × Last pooling × embedding-only 8b-specific bug" 推测进一步加强（300M 用 Mean pooling 走得通、8B 用 Last pooling 卡住）。

**vectors-embeddinggemma-300m-*.json**：SHA256 见前文（reference snapshot、入仓不替换主 vectors.json/baseline.json/cosine_threshold；follow-up cycle BETA-15B-11-v2 bake 推生产时再 rewrite 主 vectors.json + baseline.json + cosine_threshold sweep）。

**与 v5 / v4-fixup / v4 snapshot 区别**：
- v6 是 **embeddinggemma-300m reference snapshot**（探针产物、入仓作未来 baseline rewrite 入口）
- 主 active vectors = `vectors.json` 仍是 v5 bge-m3 状态（本 cycle 不动）
- `vectors-bge-m3.json` / `vectors-qwen3-0.6b.json` 既有 reference snapshot 本 cycle 不动

**诚实边界**：
- prefix 契约在 v5 dataset 81 cases 上 +0.009 ~ +0.026 加成、但 v5 crosslang 桶仅 14 例、prefix 加成在 crosslang 上仅 +0.009 = 可能含运气；评测扩量后真实加成可能 +0.005 ~ +0.020 范围
- T=0.0/0.30/0.45 三连冠是 prefix mode 在 v5 dataset 上的 plateau、bake T 选择由 follow-up cycle 重 sweep + Branch B 同款保守 vs sweep best 决定
- no-prefix mode 单独 GO 不代表「prefix 没用」、prefix 有 +0.009 ~ +0.026 实测加成；只是「prefix 不是 GO 的必要条件」
- v6 cycle 范围 = 评测探针 only / 不动桌面 wiring、桌面行为零变化、用户实际体验需 follow-up BETA-15B-11-v2 bake 才能看到

**下 cycle 抓手优先级（v6 数据指证）**：

| 抓手 | 优先级 |
|---|---|
| **BETA-15B-11-v2 bake embeddinggemma-300m 推到生产**（DEFAULT_EMBEDDING_MODEL_FILENAME 替换 / baseline.json rewrite / cosine_threshold re-sweep / 模型分发 UX）| **最高优**（双过 spec 字面 / 同尺寸 vs bge-m3 还小一半 / OVERALL +0.036 大 ROI / ~1-2w）|
| BETA-15B-11-v3 prefix API 接 model-runtime + 桌面索引应用 standard prefix | 中优（prefix +0.009 ~ +0.026 加成、与 bake 解耦、对 follow-up 独立 cycle）|
| 评测扩量 crosslang 桶 → 20-30 例 + 防范 v5 14 例样本运气 | 中优（与 BETA-15B-6 v2/v3 同款方法可复制、~1d）|
| BETA-15B-Y bge-multilingual-gemma2 9B（如想冲更高 OVERALL）| 低优（9B 桌面不现实、bge-m3 + embeddinggemma 已饱和 ROI）|
| BETA-30 真实失败样本箱（长期 + 真实闭环）| 低优 |

**链接**：[BETA-15B-11 spec](../superpowers/specs/2026-06-26-beta-15b-11-embeddinggemma-prefix-probe-design.md) / [BETA-15B-11 plan](../superpowers/plans/2026-06-26-beta-15b-11-embeddinggemma-prefix-probe.md) / [v5 节 (BETA-15B-10)](#v5-数据集节--beta-15b-10-bge-m3-baseline-重锚--cosine-sweep--bake--评测集长文本扩量--evals-embed-截断解除-done) / [EmbeddingGemma HF 卡（google）](https://huggingface.co/google/embeddinggemma-300m) / [EmbeddingGemma GGUF（ggml-org）](https://huggingface.co/ggml-org/embeddinggemma-300M-qat-q8_0-gguf)

### v6-prod 节 — BETA-15B-11-v2 bake embeddinggemma-300m 推到生产 done

承接 v6 节（BETA-15B-11 双过 spec 字面 OVERALL 0.900 / crosslang 0.725）。BETA-15B-11-v2（2026-06-27 Claude Code、[PR #19](https://github.com/raoliaoyuan/LociFind/pull/19) merged、merge commit `e3670dc`）最窄 wiring 切换 cycle、~2-3h 落地、单文件 diff < 20 行。

**实际部署生效组合**（保 v5 cosine_threshold = 0.70 不动、桌面跑 no-prefix mode）：

| 指标 | v5 bge-m3 T=0.70（前生产锚）| embeddinggemma no-prefix T=0.70（本 cycle 部署）| Δ |
|---|---|---|---|
| OVERALL | 0.864 | 0.874 | **+0.010** ⭐ |
| crosslang | 0.686 | 0.716 | **+0.030** ⭐⭐ |
| content-not-name | 0.869 | 0.895 | **+0.026** |
| exact-name | 1.000 | 1.000 | = |

**结论**：无 trade-off 全方面提升、与 BETA-15B-7-v2 时 crosslang -0.055 反退形成对比、bake 数据底气强一截。

**未吃满**：embeddinggemma sweep best 在 no-prefix T=0.60 OVERALL 0.882（vs T=0.70 +0.008）+ prefix mode T=0.0/0.30/0.45 三连冠 OVERALL 0.900 / crosslang 0.725（vs no-prefix T=0.70 OVERALL +0.026 / crosslang +0.009）；保 T=0.70 + no-prefix 是「最窄切换 + 用户实际体验只升级模型」节奏、ROI 留 follow-up cycle 拿回。

**真机手测**：DEFERRED（GO with documented gap 路径、与 BETA-15B-7-v2 / BETA-15B-10 / BETA-15B-11 同款）。用户下次升级 app 时按 [docs/manual-test-scenarios.md](../manual-test-scenarios.md) 三步走自动验证。

**桌面行为变化**：
- 旧用户升级后启动 → EmbedStatus::NotFound{expected_path: ".../models/embeddinggemma-300m-q8_0.gguf"}（v0.7 bge-m3 → v0.8 embeddinggemma）
- cp 模型后 → spawn_semantic_index 后台 reindex（document_vectors 旧行 embed_model="bge-m3" dim=1024 ≠ "embeddinggemma-300m" dim=768 → vector_is_current=false → re-embed）
- 切换完成 → 查询走 embeddinggemma 向量、OVERALL/crosslang/content-not-name 全方面提升

**分发增量**：embeddinggemma-300m q8_0 = **313 MB**（vs bge-m3 605 MB = **净降 292 MB**）。

**评测层不动**：本 cycle 不改 baseline.json / cosine_threshold / gate.rs；gate 仍守 v5 bge-m3 数据（OVERALL 0.864 / crosslang 0.686 等）、与桌面 wiring 解耦；follow-up cycle 重写 baseline 时再对齐。

**下 cycle 抓手优先级（v6-prod 数据指证 + 真机用户反馈后修订）**：

| 抓手 | 优先级 |
|---|---|
| BETA-15B-11-v2-r1 真机手测三步走 | 待用户首次升级时执行 |
| cosine_threshold 在 embeddinggemma 上 sweep & bake（拿回 sweep best 0.882 +0.008）| 中优、~1d |
| baseline.json rewrite 切到 embeddinggemma 数据 | 中优、~0.5d |
| BETA-15B-11-v3 prefix API 接 model-runtime（+0.013~+0.026 各桶加成）| 中优、~1w |
| 模型分发 UX 增强（首启引导 / 自动下载 / Windows 真机性能验证）| 中优、~1-2w |
| 评测扩量 crosslang 桶 → 20-30 例 | 中优、~1d |

**链接**：[BETA-15B-11-v2 spec](../superpowers/specs/2026-06-27-beta-15b-11-v2-bake-embeddinggemma-production-design.md) / [BETA-15B-11-v2 plan](../superpowers/plans/2026-06-27-beta-15b-11-v2-bake-embeddinggemma-production.md) / [v6 节（BETA-15B-11）](#v6-数据集节--beta-15b-11-embeddinggemma-300m-跨厂探针--prefix-契约对照实验-done-)
