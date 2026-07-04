# BETA-15B-3 簇 A-3：FTS 置信度路由 设计 spec

> 「语义召回质量做到顶」线下一刀。前置 = BETA-15B-3 A-2 weight 调优（`DEFAULT_SEMANTIC_WEIGHT=10.0` baked、`baseline.json` 锁新水位、合 main 2026-06-22）。本 cycle 焦点 = **FTS 命中弱时跳过 FTS 臂**，目标拉近 A-2 已触达的 weight 调优天花板（OVERALL nDCG 0.854 → ≥ 0.864；crosslang 0.649 → ≥ 0.700）。范围严格限定为**只做硬路由**，不做软降权 / 截断 / UI 暴露（YAGNI 防御）。

## 1. 背景与动机

A-2 weight 调优实测（详 [baseline 报告调优记录节](../../reviews/semantic-recall-quality-baseline.md)）：

| W | exact-name HYB_R | OVERALL HYB_N | crosslang HYB_N |
|---|---|---|---|
| 2.0 (旧) | 1.000 | 0.832 | 0.582 |
| **10.0 (baked)** | **1.000** | **0.854** | **0.649** |
| 20.0 | 1.000 | 0.850 ↓ | 0.662 |

**关键发现（A-2 诚实边界）**：纯抬 weight 存在天花板。
- OVERALL nDCG 0.854 距纯向量 0.864 差 0.010；W=20.0 反退到 0.850。
- crosslang nDCG 0.649 距纯向量 0.726 差 0.077；W=20.0 最大也仅 0.662 < 目标 0.700。
- 失败模式（baseline 报告结论 2/4 + 调优记录路由必要性节）：crosslang 桶 `HYB_R=0.923`——FTS 不是命不中，是**命中了同语言 g1 次级相关**污染 nDCG。VEC 排得更准（VEC_N=0.726 > HYB_N=0.582/0.649）。
- 纯抬 weight 摆脱不了这种「污染源进了 top-K」的结构性问题——污染源仍在 RRF 公式里贡献分子，只是权重压低后影响减弱。

**这是诚实的路由窗口**：当 FTS top-K 与 VEC top-K 几乎不重叠时，强信号说明 FTS 在乱命（cross-lang / 同语言 paraphrase 场景），跳过 FTS 让 hybrid 退化为纯向量，理论上能把 hybrid 拉到 VEC 水位。**同时严守 exact-name=1.0 不退化**——这种桶 FTS top-K 与 VEC top-K 必然高重叠（两臂都命中精确文件名），路由天然不触发。

**为什么不能直接砍 FTS 臂**：exact-name HYB=1.000 是硬约束（FTS 对同语言精确名查询仍重要）。路由是「按 query 信号自适应判定 FTS 是否帮倒忙」，不是去掉一臂。

## 2. 目标与验收

### 2.1 目标

- **拉近 weight 调优天花板**：hybrid_routed OVERALL nDCG ≥ 0.864（VEC 基准）；crosslang nDCG ≥ 0.700（A-2 spec 同款目标）。
- **不破红线**：exact-name HYBR_R = 1.000 不变（硬断言）。
- **不留遗物**：阈值 bake 进 `DEFAULT_FTS_JACCARD_THRESHOLD` + `baseline.json` 新增 HYBR 字段、HYB 旧字段保留作历史对照，回归门双守护。
- **API 对位**：`fuse_rrf` 保持纯 N 列表融合语义不动；新 wrapper `fuse_rrf_with_fts_routing` 显式区分 FTS/VEC 两臂——评测和生产共享同一融合路径。

### 2.2 验收红线（不可回归）

1. `cargo test --workspace` 0 failed；含回归门 `semantic_quality_gate` 用**新 baseline**（含 HYBR 字段）pass。
2. clippy `-D warnings` 0；fmt 净；前端 tsc + vite build 净。
3. **evals parser-only byte-equal 不变**（v0.5=473 / v0.9=877）——本刀不动 parser/索引/融合算法签名，路由是融合层加法。
4. **新 baseline 实测**：
   - (4a) **exact-name HYBR_R = 1.000**（硬红线，所有阈值都必须满足）
   - (4b) **HYBR 各桶 nDCG ≥ HYB baseline 同桶**（synonym 0.905 / concept 0.819 / content-not-name 0.930 / OVERALL 0.854）——路由不让任何单桶退步
   - (4c) **HYBR crosslang nDCG ≥ 0.700**（A-2 同款 spec 目标；未达走 §5 异常分支降级）
   - (4d) **HYBR OVERALL nDCG ≥ 0.864**（A-2 同款 spec 目标；未达走 §5 异常分支降级）
5. **wrapper 防御**：阈值 ∉ [0, 1] → debug_assert + clamp 到 [0, 1]；空 list 输入良定义返回。

## 3. 范围（含主动 YAGNI）

### 3.1 In-scope

- `result-normalizer` 加 `RouteVerdict` struct + `jaccard_overlap_by_path` 工具函数 + `fuse_rrf_with_fts_routing` wrapper + 两个常量 `DEFAULT_FTS_JACCARD_THRESHOLD` / `DEFAULT_FTS_ROUTING_TOP_K`。
- `fuse_rrf` 本身**不动**（保持 N 列表融合纯语义）。
- 评测层 `semantic_quality::arms` 加 `hybrid_routed_rank` helper；`semantic_quality` binary 加 `--jaccard-threshold=<f64>` CLI flag；report / baseline.json schema 新增 `hybrid_routed_recall` / `hybrid_routed_ndcg` 字段；HYB 旧字段保留。
- sweep（候选阈值集 `{0.0, 0.10, 0.20, 0.30, 0.50, 1.0}`）人工选 t\*；W 固定 10.0。
- bake `DEFAULT_FTS_JACCARD_THRESHOLD = t*`（单一默认源）。
- 生产侧 `run_fanout_merge_rrf` 改调 wrapper（两臂明确区分）；`RouteVerdict` 透传到结果元数据作 BETA-15B-5 badge 槽位（**本 cycle 不画 UI**）。
- 写新 `baseline.json`（HYBR 字段 + HYB 保留）+ 回归门加 HYBR 断言（4a/4b/4c/4d）。
- 追加调优记录到 [baseline 报告](../../reviews/semantic-recall-quality-baseline.md)。

### 3.2 Out-of-scope（明确 YAGNI）

- **软降权 / 动态权重折扣**：硬跳过最简、调优旋钮唯一；若 sweep 后阈值附近震荡明显，记下 cycle 升级。
- **截断 FTS 列表**（保留 top-1/2）：crosslang 同语言 g1 往往 BM25 居前，截断救不了主要失败模式。
- **UI 暴露**（开关 / 阈值数字框）：路由是后端启发式，与 floor/weight 的"用户偏好"语义不同；BETA-15B-5 badge 槽位足以让用户感知。未来真有错杀场景再升级。
- **二维 sweep (t × W)**：W=10.0 是 A-2 刚 bake、本 cycle 焦点是路由是否能拉过天花板；W 联动是二阶问题，留下 cycle 数据指证再说。
- **原始 query 入 schema**（簇 A 另一子项）：另一刀，byte-equal 风险须 router 后置填充、与本刀融合层逻辑无依赖。
- **真机手测**：纯后端融合层加法 + 评测端到端覆盖，平凡，不安排手测剧本。
- **`RouteVerdict` 前端 badge 渲染**：本 cycle 只透传元数据槽位；BETA-15B-5 可解释 v1 已有 badge 框架，画 UI 留下 cycle 或独立小刀。

## 4. 架构

```
┌─ 评测路径 (semantic_quality binary) ─────────────────────────┐
│  arms::hybrid_routed_rank(fts_ids, vec_ids, t, W, k)         │
│         │ to_results(...)                                     │
│         ▼                                                     │
│  fuse_rrf_with_fts_routing(fts_list, vec_list, k, W, t)       │
│         │                                                     │
└─────────┼─────────────────────────────────────────────────────┘
          │
┌─ 生产路径 (run_fanout_merge_rrf) ─────────────────────────────┐
│  fanout: FTS 臂 → fts_list、VEC 臂 → vec_list                  │
│         ▼                                                     │
│  fuse_rrf_with_fts_routing(fts_list, vec_list,                │
│       k=DEFAULT_RRF_K, W=resolve_semantic_weight(),           │
│       t=DEFAULT_FTS_JACCARD_THRESHOLD)                        │
│         │                                                     │
└─────────┼─────────────────────────────────────────────────────┘
          ▼
   ┌─────────────────────────────────────────────┐
   │ result-normalizer::fts_routing             │
   │                                              │
   │   jaccard = jaccard_overlap_by_path(         │
   │       fts top-K, vec top-K, K=10)            │
   │                                              │
   │   if jaccard < t:                            │
   │       fuse_rrf([vec_list], k, W)             │
   │       verdict = { skipped_fts: true, ... }   │
   │   else:                                      │
   │       fuse_rrf([fts_list, vec_list], k, W)   │
   │       verdict = { skipped_fts: false, ... }  │
   │                                              │
   │   return (results, verdict)                  │
   └─────────────────────────────────────────────┘
```

**新模块组织**（lib.rs inline 即可，规模 ~80 行 + 单测）：

```rust
// packages/result-normalizer/src/lib.rs

/// FTS/VEC top-K Jaccard 路由的默认阈值（< 阈值时跳过 FTS 臂）。
/// A-3 sweep 选定（详 docs/reviews/semantic-recall-quality-baseline.md A-3 调优记录节）。
pub const DEFAULT_FTS_JACCARD_THRESHOLD: f64 = /* sweep 后 bake */;

/// 路由 Jaccard 计算的 top-K 截断窗口；与评测 TOP_K=10 一致。
pub const DEFAULT_FTS_ROUTING_TOP_K: usize = 10;

/// 路由判定副产物，便于评测/badge/调试消费。
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteVerdict {
    pub skipped_fts: bool,
    pub jaccard: f64,
    pub threshold: f64,
}

/// 计算两个有序 SearchResult 列表 top-K 的 path 集合 Jaccard 重叠度。
/// |A ∩ B| / |A ∪ B|；空集 ∪ 空集 = 0.0。
#[must_use]
pub fn jaccard_overlap_by_path(
    a: &[SearchResult],
    b: &[SearchResult],
    k: usize,
) -> f64 { ... }

/// 加路由的 RRF 融合 wrapper：FTS/VEC 两臂分别传入，
/// Jaccard 重叠 < threshold 时跳过 FTS 臂（hybrid 退化为纯向量）。
/// threshold ∉ [0, 1] → debug_assert + clamp。
#[must_use]
pub fn fuse_rrf_with_fts_routing(
    fts_list: Vec<SearchResult>,
    vec_list: Vec<SearchResult>,
    rrf_k: f64,
    semantic_weight: f64,
    jaccard_threshold: f64,
) -> (Vec<MergedResult>, RouteVerdict) { ... }
```

**单一默认源**：常量 `DEFAULT_FTS_JACCARD_THRESHOLD`（生产 + 评测 binary 默认）。binary `--jaccard-threshold` 不传则用常量。

**评测/生产对齐**：两边都调同一 wrapper、同一常量；不存在「评测的 hybrid_routed 与生产 hybrid_routed 实现漂移」的风险。

## 5. 调优工作流（spec 阶段执行）

```bash
# 步骤 0：在分支上加 RouteVerdict / jaccard_overlap_by_path / fuse_rrf_with_fts_routing
#         + arms::hybrid_routed_rank + binary --jaccard-threshold flag（task 1-3）

# 步骤 1：跑 sweep（用现有 vectors.json 缓存，零模型推理；W 固定 10.0）
for t in 0.0 0.10 0.20 0.30 0.50 1.0; do
  echo "=== threshold=$t ==="
  cargo run -p locifind-evals --bin semantic_quality -- \
    --jaccard-threshold=$t --json
done | tee /tmp/sweep-jaccard.log

# 步骤 2：人工读 sweep-jaccard.log，选满足以下四条的 t*：
#   ① exact-name HYBR_R = 1.000（所有 t 都应满足；不满足= bug）
#   ② OVERALL HYBR_N 最大（目标 ≥ 0.864 = 纯向量基准 / 当前 HYB 0.854）
#   ③ crosslang HYBR_N 最大（目标 ≥ 0.700 / 当前 HYB 0.649）
#   ④ 其他桶 HYBR_N ≥ HYB baseline 同桶（不退步）
# 控制对照：t=1.0 应使 HYBR ≡ HYB（总不跳过路由）；t=0.0 应使 HYBR ≡ VEC（总跳过）

# 步骤 3：bake t* 到 DEFAULT_FTS_JACCARD_THRESHOLD
# 步骤 4：写新 baseline.json（HYBR 字段加入；HYB 字段保留）
cargo run -p locifind-evals --bin semantic_quality -- --write-baseline

# 步骤 5：跑回归门验证
cargo test -p locifind-evals --test semantic_quality_gate
```

**透明度承诺**：sweep 表（6 个 t 各自 5 桶 × 2 指标 = 60 数字）+ 选定理由 + 控制对照（t=0.0 vs t=1.0 健全性核验）写入 [baseline 报告](../../reviews/semantic-recall-quality-baseline.md) 的「A-3 调优记录（2026-06-23）」节。

**异常分支**（与 A-2 §5 同款套路）：
- 如果**没有 t 能让 crosslang HYBR_N ≥ 0.700**：诚实说明天花板（可能需要更强信号——重叠 + 语种、或更大模型），但仍 bake 一个相对最优 t（不让任一桶退步且 OVERALL/crosslang 显著优于 HYB baseline），把目标降为「显著优于 A-2 baseline」，路由必要性证据已成立 + 下 cycle 抓手归"更强信号"。
- 如果**没有 t 能让 OVERALL HYBR_N ≥ 0.864**：同上降级。
- 如果**任何 t 都让 exact-name HYBR_R 跌破 1.0**：abort——硬约束被破坏说明 Jaccard 信号在 exact-name 场景反常（理论不应——精确名查询两臂必然高重叠），回到 brainstorming。
- 如果**任何 t 都让某非 exact 桶退步 HYB baseline**：bake 选最保守 t；记报告诚实暴露并归下 cycle。

## 6. 代码改动清单

| # | 层 | 文件 | 改动 | 估时 |
|---|---|---|---|---|
| 1 | result-normalizer | `packages/result-normalizer/src/lib.rs` | 加 `RouteVerdict` struct + `jaccard_overlap_by_path` + `fuse_rrf_with_fts_routing` wrapper + 两常量；fuse_rrf 不动；加单测（9+ 个） | 1.5h |
| 2 | 评测 arms | `packages/evals/src/semantic_quality/arms.rs` | 加 `hybrid_routed_rank(fts_ids, vec_ids, t, W, k)` helper（4 单测） | 0.5h |
| 3 | 评测 binary | `packages/evals/src/bin/semantic_quality.rs` | 加 `--jaccard-threshold=<f64>` clap 参数，默认 = `DEFAULT_FTS_JACCARD_THRESHOLD`；传入 `score_case` | 0.3h |
| 4 | 评测 report + baseline | `packages/evals/src/semantic_quality/report.rs` + `fixtures/semantic-recall/baseline.json` | report 数据结构加 `hybrid_routed_recall` / `hybrid_routed_ndcg`；baseline.json schema 加同字段；HYB 字段保留 | 0.5h |
| 5 | 调优工作流 | 本机 sweep，**不入仓** | shell loop 跑 binary + 人工读表 + 选 t* + 控制对照验证 | 0.5h |
| 6 | 生产默认 | `packages/result-normalizer/src/lib.rs` | bake `DEFAULT_FTS_JACCARD_THRESHOLD = t*` + 更新 doc-comment 引用调优记录节 | 0.1h |
| 7 | 生产 wiring | `desktop/.../run_fanout_merge_rrf` 及调用点 | 改调 `fuse_rrf_with_fts_routing` 替换 `fuse_rrf`（两臂明确分离）；`RouteVerdict` 透传到结果元数据（暂存字段，本 cycle 不画 UI） | 1.0h |
| 8 | 回归门 baseline | `packages/evals/fixtures/semantic-recall/baseline.json` | 用 `--write-baseline` 重写为含 HYBR 字段的新水位（HYB 字段保留） | 0.1h |
| 9 | 回归门断言 | `packages/evals/tests/semantic_quality_gate.rs` | 加 HYBR 各桶断言（4a exact-name=1.0 硬断言 / 4b 不退步 / 4c crosslang ≥ 0.700 或降级 / 4d OVERALL ≥ 0.864 或降级） | 0.3h |
| 10 | 报告 | `docs/reviews/semantic-recall-quality-baseline.md` | 追加「A-3 调优记录（2026-06-23）」节：sweep 表 + 选定 t* + 控制对照 + 与 A-2 baseline 对照 + 路由生效证据 + 是否触发下 cycle 抓手 | 0.5h |

**总估**：~5.3h 代码 + 评测/手验。

## 7. 验证 checklist（每 task 验证门必含 fmt + clippy + test）

- [ ] `cargo fmt --all -- --check` 净
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` 0
- [ ] `cargo test --workspace` 0 failed（含 result-normalizer 新单测、arms 新单测、semantic_quality_gate 新 HYBR 断言）
- [ ] `cargo test -p locifind-evals --test semantic_quality_gate` pass（新 baseline 含 HYBR 字段）
- [ ] `cargo run -p locifind-evals --bin semantic_quality` 实测：
  - exact-name HYBR_R = 1.000 ✅（硬红线）
  - HYBR 各桶 nDCG ≥ HYB baseline 同桶 ✅
  - OVERALL HYBR_N ≥ 0.864 ✅（或诚实说明天花板降级）
  - crosslang HYBR_N ≥ 0.700 ✅（或诚实说明天花板降级）
- [ ] 控制对照实测：
  - `--jaccard-threshold=1.0` 时 HYBR 等价 HYB（总不跳过）
  - `--jaccard-threshold=0.0` 时 HYBR 等价 VEC（总跳过）
- [ ] evals parser-only byte-equal：v0.5=473 / v0.9=877 不变（本刀不动 parser）
- [ ] desktop 前端 `tsc` + vite build 净（route_verdict 元数据透传不画 UI、不动 React 类型应净）
- [ ] desktop fanout wiring 测试：`run_fanout_merge_rrf` 调用 wrapper、`RouteVerdict` 字段填充正确（添加 1-2 个新单测）

## 8. 风险与已记忆教训

### 8.1 风险

1. **合成集 crosslang 桶设计「同语言 g1 次级相关」可能让 Jaccard 信号偏强**——FTS 命中 g1、VEC 命中 g3，两臂重叠确实低。这正是路由想拦的失败模式，但**真实负载 crosslang 比例 + g1 污染比例可能与合成集不同**。缓解：① 合成集 baseline 是融合层水位、非真实负载最终值；② 控制对照（t=0.0 vs t=1.0）能识别"信号是否真在传递"；③ BETA-26 真实集校准锤可周期性核对；④ 下 cycle 真实负载数据指证再调阈值。
2. **exact-name 桶 Jaccard 可能因 FTS top-K 与 VEC top-K 排序细节而短暂低于阈值**——理论上精确名查询两臂都命中目标文件，但 VEC 可能把语义近的其他文件排进 top-K 一两个位置。若 t* 太高可能误跳。**缓解**：(4a) exact-name HYBR_R=1.0 硬断言守护；sweep 表若任一 t 让 exact-name 跌破，abort 重审。
3. **`run_fanout_merge_rrf` 现签名假设接 N 列表 + 不区分臂源**——改 wrapper 要明确传 fts_list / vec_list 两个参数。需查现状决定签名重构范围。如果 fanout 已按 BackendKind 聚类列表，按 source 拆分简单；如果是 flat list-of-results 再融合，需中间一层重构。task 7 估时 1.0h 含此查证。
4. **`RouteVerdict` 透传槽位字段未消费**——本 cycle 透到 `SearchResult.metadata` 或新增字段都是死代码（`#[allow(dead_code)]`）。**缓解**：明确加注释「BETA-15B-5 badge 槽位预留」+ 下 cycle 或独立小刀画 UI 时消费；避免「rustc 告警驱动滥删」。

### 8.2 已记忆教训对照

- [[project-evals-coverage-pipeline-drift]]：本刀不动 coverage/shards，无 drift 风险。
- [[project-evals-reporter-nondeterministic]]：`semantic_quality` binary 输出已用 BTreeMap 有序；baseline.json 新增 HYBR 字段后 diff 仍须 by-key 比，不能裸 diff。
- [[feedback-per-task-verify-include-fmt]]：每 task 完成必跑 fmt-check + clippy + test 三件套。
- [[project-stale-hybrid-fallback]]：本刀不动 fallback/hybrid 模型层，无 keyword/title 回声风险；路由完全在 result-normalizer 融合层，模型相关零接触。
- [[project-rrf-weight-tuning-ceiling]]：本刀正是该记忆点对应的下 cycle 抓手；A-2 已锁 W=10.0、本刀做路由是天花板诊断后的针对性下一步。
- [[feedback-baseline-lock-red-line-pattern]]：本刀完工时必做收口三件套——锁新 baseline.json（HYBR 字段）+ 不可破红线硬断言（exact-name HYBR_R=1.0 + 各桶不退步）+ 调优记录追加报告。

## 9. 链接

- 起点 [baseline 报告 A-2 调优记录节](../../reviews/semantic-recall-quality-baseline.md)（路由必要性证据）
- 前置 [BETA-15B-3 A-2 weight 调优 spec](2026-06-22-beta-15b-3a2-semantic-weight-tuning-design.md)
- 同簇前刀 [BETA-15B-3 A-1 floor + visible score spec](2026-06-18-beta-15b-3a1-tunable-floor-visible-score-design.md)
- 前前置 [BETA-15B-6 持久化评测设施 spec](2026-06-21-beta-15b-6-semantic-recall-quality-eval-design.md)
- 战略上下文 STATUS / ROADMAP（2026-06-22 BETA-15B-3 A-2 done 段 + 下一步「FTS 置信度路由」指向）
