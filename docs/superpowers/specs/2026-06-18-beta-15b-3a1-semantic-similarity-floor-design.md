# BETA-15B-3 簇 A-1 设计：语义臂相似度下限

> 类型：**生产增量**，单一聚焦改动。BETA-15B 旗舰语义召回层的精度补强。
> 关系：BETA-15B-3 簇 A（数据驱动精度核心）含四项强耦合改动；按「价值 / 依赖」重新分层后，**相似度下限是唯一高价值、无隐私门、byte-equal 安全的一项**，单独先做（本 spec）。其余三项——加权 RRF 权重调优（BETA-26 §4.6 判低 ROI、margins 噪声内、须 held-out 重调）、held-out 评测扩量（隐私门：gitignored 真实语料）、原始 query 入 schema（跨 schema/harness + byte-equal 风险）——留后续，不在本 spec。
> 边界：只改 `semantic-index` 后端的候选过滤，不碰 parser / 融合权重 / schema / 评测集。

## 1. 背景与目标

BETA-15B-1 真机手测发现 (a)：语义臂 `search_results` **无条件吐 top-10**（`semantic-index/src/lib.rs:100-101` 排序后直接 `truncate(TOP_K)`），无相似度下限。在**小语料**下，即便相关度极低的候选（cosine ≈ 0.1~0.2）也会凑进 top-10、进入 RRF 融合、被打上旗舰「按意思找到」徽标——拉低精度、让差异化徽标廉价。

**唯一目标**：给语义臂加一个 **cosine 相似度下限**，把明显不相关的候选挡在语义召回之外（不进结果、不打徽标），只让有真实语义相关性的命中浮现。feature 关 / 无模型行为不变；evals byte-equal 不动（不碰 parser）。

## 2. 范围护栏（YAGNI）

| 本切片做（簇 A-1） | 留后续 / 不做 |
|---|---|
| 语义臂 cosine 下限过滤（命名常量 + 纯函数） | 加权 RRF 权重调优（BETA-26 低 ROI，须 held-out）→ 簇 A 后续 |
| 阈值取 BETA-26 数据支撑的保守默认 0.30 | held-out 评测基础设施（隐私门）→ 簇 A 后续 |
| — | 原始 query 入 schema（跨切面 + byte-equal 风险）→ 簇 A 后续 |
| — | 阈值暴露到设置页 / 按 query 自适应路由（YAGNI，常量足够，待评测再调） |

## 3. 架构与组件

### 3.1 下限过滤（`packages/search-backends/semantic-index/src/lib.rs`）

现状（`search_results`，:95-106）：候选逐个算 cosine → `sort_by` 降序 → `truncate(TOP_K)` → 映射 `SearchResult`。

改动：在排序/截断处插入相似度下限过滤，抽成纯函数便于单测：

```rust
/// 语义臂相似度下限：低于此 cosine 的候选视为不相关，不进结果（不打旗舰徽标）。
/// BETA-26 `embed()` 证伪闸门实测：相关文本 cosine ≈ 0.75、无关 ≈ 0.18；0.30 稳高于无关基线、
/// 远低于真实相关（含 crosslang 命中），只挡明显噪声。命名常量：待簇 A held-out 评测落地后据数据精调。
const SIMILARITY_FLOOR: f32 = 0.30;

/// 按相似度下限过滤 + 降序排序 + 截断 topK（纯函数，可单测）。
/// 全部候选低于 floor → 返回空（语义臂空，整链优雅降级 FTS-only）。
fn filter_rank_topk(mut scored: Vec<(f32, String)>, floor: f32, k: usize) -> Vec<(f32, String)> {
    scored.retain(|(s, _)| *s >= floor);
    scored.sort_by(|a, b| b.0.total_cmp(&a.0));
    scored.truncate(k);
    scored
}
```

`search_results` 改为：算完 `scored` 后 `let scored = filter_rank_topk(scored, SIMILARITY_FLOOR, TOP_K);` 再映射 `vector_hit_to_result`。原地的 `sort_by` + `truncate(TOP_K)` 两行被 `filter_rank_topk` 取代。

### 3.2 阈值取值

`SIMILARITY_FLOOR = 0.30`（用户 2026-06-18 确认，保守档）。依据：BETA-26 备忘记录的 `embed()` 证伪闸门——相关文本 cosine ≈ 0.75、无关 ≈ 0.18。0.30 落在两者之间偏保守侧：高于无关基线（不被噪声触发），远低于真实相关命中（不误杀边缘相关 / 跨语言项）。**命名常量**，簇 A held-out 评测基础设施落地后用真实分桶数据精调（一行改），本切片不据无评测的数字过度精调（YAGNI）。

## 4. 数据流

- **查询**：query → 语义臂 `embed(query)` → 暴力 cosine 全候选 → **`filter_rank_topk`（下限过滤 + 降序 + topK）** → 映射 `SearchResult`（带 cosine score + `MatchType::Semantic`）→ 进 RRF 融合（仅 ≥floor 的命中）→ ranker → UI（仅高相关项打「按意思找到」徽标）。
- 全候选低于 floor → 语义臂返回空 → fanout 里 FTS 臂兜底（既有优雅降级路径，不变）。

## 5. 错误处理

- 无嵌入器 / 无查询文本 / db 不存在 → 既有早返回空（不变）。
- 全候选低于 floor → 空结果、降级 FTS-only（与「无语义命中」同路径，无需特殊处理）。
- 下限是纯比较，无新增失败模式。

## 6. 测试

- **纯函数 `filter_rank_topk` 单测**：混合分数候选 → 仅 `>= floor` 存活、降序、截断 K；全低于 floor → 空；空输入 → 空。
- **改现有 `semantic_query_ranks_by_cosine`**：当前 dog 向量 `[0,1]` 与查询 `[1,0]` 正交（cosine 0）会被下限挡掉、破坏「两条都返回」断言。改测试向量为两者都过下限但有序——cat `[1, 0.2]`（cosine ≈ 0.98）、dog `[1, 1]`（cosine ≈ 0.71），保住「按 cosine 排序」的测试目的 + 两条都返回 + cat 排前。
- **新增下限测试 `semantic_floor_filters_low_relevance`**：一个候选 cosine 明显低于 floor（如正交 `[0,1]` 对查询 `[1,0]` = 0.0）+ 一个高于 floor → 断言低相关项被过滤、不在结果中，高相关项保留。
- **回归（硬门）**：
  - evals v0.5=473 / v0.9=726 byte-equal（不碰 parser，天然守住）。
  - 全 workspace `cargo test` 零失败、`clippy -D warnings` 0、`fmt --check` 净。
  - 15B-1 跨语言命中（cosine 高）不受 0.30 下限影响——由真机手测覆盖（登记）。
- **真机手测（登记，留用户）** → `docs/manual-test-scenarios.md` 加簇 A-1 节：小语料下不相关查询不再返回凑数语义项 / 不打徽标；跨语言真实命中仍正常浮现 + 打徽标。

## 7. 平台

与平台无关，macOS + Windows 同一份代码。

## 8. 验收标准

1. 语义臂只返回 cosine ≥ 0.30 的候选；低相关项不进结果、不打徽标。
2. 全候选低于下限 → 语义臂空、降级 FTS-only。
3. evals byte-equal；全 workspace test / clippy / fmt 全绿。
4. 15B-1 跨语言命中不回退（真机手测登记）。

## 9. 未尽 / 后续交接

- **簇 A 后续（强耦合，单独评估）**：held-out 评测基础设施（先决 gitignored 真实语料的隐私方案）→ 据数据精调 `SIMILARITY_FLOOR` + 加权 RRF 权重 + FTS 置信度阈值路由；原始 query 入 schema（修语义臂 keywords 拼接近似，跨 schema/harness + byte-equal 风险）。BETA-26 §4.6 已判权重调优低 ROI（margins 噪声内），优先级低。
- 阈值若真机手测显示 0.30 仍偏松/偏紧，可一行调整（命名常量）。
