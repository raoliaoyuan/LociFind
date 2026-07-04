# BETA-05 Ranker（多源结果排序）— 设计

> 状态：draft（待用户 review）
> 关联：ROADMAP §3.3 B2 BETA-05；承接 BETA-04 Result Normalizer；PROJECT 架构「Result Normalizer + Ranker」
> ID：BETA-05

## 1. 背景与目标

BETA-04 把多源结果归一化合并为 `MergedResult` 列表，但**保持首现序**（各后端结果交错，无全局排序）。`common::sort_results` 只处理用户显式排序（时间/大小/名称），`RelevanceDesc`（默认）是空操作 → **fan-out 合并集在默认相关性下完全没有排序**。

BETA-05 Ranker 填这个缺口：对合并集做**相关性排序**（默认）+ 让**用户显式 sort 跨源生效**。让命中文件名的、多源一致的、近期的结果排前面。

## 2. Brainstorming 决策（已与用户对齐）

| # | 决策 | 选择 |
|---|---|---|
| ① | BM25 范围 | **纯启发式**：name-match + 多源一致 + match-type 透明加权，跨源可比、可测。不接 FTS5 bm25（跨语料 BM25 不可比、FTS bm25 仅局部、系统后端无 score）；BM25 留未来细化 |
| ② | 应用范围 | **仅 fan-out 路径**：ranker 排 fan-out 合并集；fallback 单后端维持后端自身排序（改动小、零回归） |

## 3. 架构

新 crate `packages/ranker`（`locifind-ranker`），依赖 `locifind-result-normalizer`（`MergedResult`）+ `locifind-search-backend`（`SortOrder` / 意图类型 / `intent_sort_order`）。

```rust
pub struct RankContext {
    /// 查询关键词（小写，用于 name-match 相关性）。
    pub keywords: Vec<String>,
    /// 用户显式排序；None / RelevanceDesc → 走相关性启发式。
    pub sort: Option<SortOrder>,
}

impl RankContext {
    /// 从扩展意图提取：关键词（base intent keywords/artist/title/album + keyword_groups 全词）
    /// + `intent_sort_order(&expanded.base)`。
    pub fn from_expanded(expanded: &ExpandedSearchIntent) -> Self;
}

/// 对合并集排序。显式 sort（时间/大小/名称）→ 跨源生效；否则相关性降序。
/// 把相关性分写入每条 `result.score`（UI 可展示）。
pub fn rank(results: Vec<MergedResult>, ctx: &RankContext) -> Vec<MergedResult>;
```

### 3.1 相关性启发式（sort = RelevanceDesc / None）

每条 `MergedResult` 算一个 `[0,1]` 相关性分，三信号加权（无需 clock / 跨集归一化，纯函数）：

| 信号 | 计算 | 权重 |
|---|---|---|
| **name_match** | 命中 `ctx.keywords` 的比例（文件名小写子串匹配）；无 keyword → 0 | 0.5 |
| **match_type_weight** | `match_types` 取最大：Filename 1.0 / Metadata 0.85 / Content 0.7 / Ocr 0.6 | 0.3 |
| **source_boost** | `min(sources.len()-1, 2) / 2`（单源 0，3+ 源 1） | 0.2 |

`score = 0.5·name_match + 0.3·match_weight + 0.2·source_boost`，写入 `result.score`。

排序：score 降序；**tiebreak**：`modified_time` 降序（新→前）→ `name` 升序（稳定确定性）。

> 设计取舍：name_match 主导（文件名命中最强相关信号，跨所有源可比）；match_type 次之；多源一致再次。recency 仅作 tiebreak（不进主分，避免无 clock 的跨集归一化）。无 keyword 查询（纯类型/扩展名经 fan-out 少见）→ name_match=0，退化为 match_type + source + recency 排序，仍合理。

### 3.2 显式排序（sort = Modified/Created/Accessed/Size/Name 各 Asc/Desc）

按 `result.metadata` 对应字段排序（语义对齐 `common::sort_results`，但作用于 `MergedResult`）。缺失字段排末尾（`compare_option` 同款）。**不写 relevance score**（显式排序不需要）。

### 3.3 集成（desktop fan-out 路径）

`run_fanout_search` 当前流式逐条 `on_result` 发出。改为：**收齐 → rank → 发出**——
- `run_fanout_merge` 的 `on_result` 把 `MergedResult` 收进 `Vec`（内部本就先收齐各后端再合并，无流式损失）；
- `rank(merged, &RankContext::from_expanded(&expanded))`；
- 按排序后顺序逐条发 `SearchEvent::Result`（带 sources）+ `record`。

fallback 路径不动。

## 4. 验收 / 验证门

1. **ranker 单测**（纯函数）：
   - 相关性：name 命中关键词的排在前；match_type Filename > Content；多源 > 单源；score 写入 [0,1]；tiebreak modified_time 新→前、再 name；无 keyword 退化合理；空输入。
   - 显式排序：ModifiedDesc 新→前、SizeDesc 大→前、NameAsc 字典序；缺失字段排末尾。
   - `RankContext::from_expanded`：提取 keywords（FileSearch/MediaSearch + keyword_groups）+ sort。
2. **desktop**：fan-out 路径收齐→rank→发；既有 desktop 测试零回归；fan-out 合并测试断言排序后顺序（如 name 命中关键词者排首）。
3. **零回归**：fallback 路径不动；evals / harness / 全 workspace test 全过（除 platform-macos 预存 Windows 失败）；fmt + clippy `-D warnings`。
4. **文档**：ranker README + ROADMAP BETA-05 done + STATUS。无新外部依赖。

## 5. 非目标（YAGNI）

- 不做真 BM25 / 跨源 IDF（不可得；FTS bm25 仅局部，留未来）。
- 不排 fallback 路径（仅 fan-out）。
- 不做可配置权重 / 学习排序（权重为文档化常量）。
- 不做 OCR 相关性（BETA-03 未接；match_type Ocr 权重已预留）。
- 不做分页 / limit 调整（沿用 intent.limit，ranker 只排序不截断；截断由后端/调用方）。

## 6. 风险与缓解

| 风险 | 缓解 |
|---|---|
| 收齐再排打破流式 | fan-out 本就先收齐各后端再合并，无额外延迟；结果集小（≤limit×源数） |
| 权重不合理致排序差 | 权重文档化 + 单测锚定典型场景；name 主导是稳健默认；后续可调 |
| 无 keyword 查询退化 | name_match=0 时退化为 match_type+source+recency，仍合理；纯类型查询经 fan-out 少见 |
| 显式排序与 ranker 重复 common 逻辑 | 仅复刻 compare 语义于 MergedResult，contained；不动 common |
