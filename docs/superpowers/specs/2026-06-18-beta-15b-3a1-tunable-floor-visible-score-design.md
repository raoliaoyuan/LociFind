# BETA-15B-3 簇 A-1 续：语义相似度下限可调 + 分数可见（调参辅助）

> 类型：**生产增量（调参辅助）**。承接 BETA-15B-3 簇 A-1（语义臂相似度下限，已落地 v0.3.0）。
> 起因：v0.3.0 Windows 真机手测——暖机/解耦/进度 UI 全部验通，但 **0.30 下限偏松、不相关项仍漏网**。Windows 发版每轮 ~24min CI + 重装，盲猜调阈值代价高。本切片把「调阈值」从重建循环里解放出来：让分数可见 + 阈值进设置 live-read，用户现场调到满意，再 bake 默认。
> 边界：只加分数显示 + 阈值可配置，不动召回算法/融合/评测集；阈值默认仍 0.30（未设置的用户零变化）。

## 1. 背景与目标

簇 A-1 给语义臂加了 cosine 相似度下限（`SIMILARITY_FLOOR = 0.30` 硬编码常量）。v0.3.0 真机反馈：0.30 偏松，同语言不相关文档（Qwen3-Embedding 同语言基线 cosine 高于 BETA-26 记录的 0.18 无关基线）仍越过 0.30、被打「按意思找到」徽标。BETA-26 早有结论：阈值须据真实分数定，不能盲调。但 Windows 发版循环慢（~24min/轮）。

**唯一目标**：① 语义结果显示其 cosine 分数（看清不相关 vs 命中各多少分）；② 相似度下限可在设置页调整、live-read 即时生效（改后重搜即生效、免重启、免重建）。让用户一次构建后自助收敛到合适阈值。

## 2. 范围护栏（YAGNI）

| 本切片做 | 不做 |
|---|---|
| 语义结果显 cosine 分数（前端） | 其它匹配类型的分数 / 排序可视化 |
| 相似度下限进 `AppSettings` + 设置页控件 | held-out 评测自动定阈值（簇 A 剩余，隐私门） |
| 后端 `floor_provider` 闭包 live-read | 按 query 自适应路由 / FTS 置信度路由（簇 A 剩余） |
| clamp [0,1] + 默认 0.30 fallback | 阈值的 per-backend / per-query 细分 |

## 3. 架构与组件

### 3.1 分数可见（`apps/desktop/src/SearchView.tsx`）

`ALL_COLUMNS` 的 `match` 列 render：semantic 结果当前只渲染「按意思找到」徽标。改为徽标后附 cosine 分数（`r.score`，2 位小数），如「按意思找到 · 0.42」。分数用淡灰小字。仅 `match_type === "semantic"` 时显示分数（其它类型 `score` 非 cosine 语义，不显）。`r.score` 可能为 null（防御）→ 无分数时只显徽标。

### 3.2 可调下限（后端 `packages/search-backends/semantic-index/src/lib.rs`）

- `SemanticIndexBackend` 加字段 `floor_provider: Arc<dyn Fn() -> f32 + Send + Sync>`；`new` 加第三参。
- `search_results`：调 `(self.floor_provider)()` 取当前下限，传给已有 `filter_rank_topk(scored, floor, TOP_K)`。
- **移除后端 `SIMILARITY_FLOOR` 常量**（不再被使用——下限统一由 provider 供给）。**单一默认源**：默认值 0.30 移到 desktop 侧命名常量 `DEFAULT_SIMILARITY_FLOOR`（§3.4 闭包 fallback），全仓只此一处。后端测试传常量闭包（如 `Arc::new(|| 0.30_f32)`），不依赖该常量。
- 后端**不依赖** desktop `AppSettings`——只收闭包，解耦。

### 3.3 设置字段（`apps/desktop/src-tauri/src/settings.rs`）

`AppSettings` 加 `semantic_similarity_floor: Option<f32>`（`#[serde(default)]` 结构级已在 → 旧 settings.json 缺此字段照常解析；`Default` impl 补 `None`）。`None` = 用默认 0.30。

### 3.4 desktop 接线（`apps/desktop/src-tauri/src/main.rs`）

- `build_registry` 加参 `settings_path: Option<PathBuf>`（setup 调用点传 `settings::settings_file_path(...)`；测试调用点传 `None`）。
- 定义 `const DEFAULT_SIMILARITY_FLOOR: f32 = 0.30;`（全仓单一默认源）。
- 构造 `floor_provider` 闭包：读 `settings_path` 的 settings.json → 解析 `AppSettings` → `semantic_similarity_floor` → `.unwrap_or(DEFAULT_SIMILARITY_FLOOR)`，**clamp 到 [0.0, 1.0]**；任何读/解析失败或值非有限 → fallback `DEFAULT_SIMILARITY_FLOOR`。闭包每次查询执行（live-read，镜像 `EmbeddingModelHandle::resolved_model_path` 的每次读 settings.json）。传给 `SemanticIndexBackend::new`。clamp 逻辑抽小函数 `resolve_floor(raw: Option<f32>) -> f32` 便于单测。

### 3.5 前端控件（`apps/desktop/src/SettingsPage.tsx`）

`AppSettings` TS 接口加 `semantic_similarity_floor: number | null`。设置页加数值输入框「语义相似度下限（0–1，越高越严，默认 0.30）」+ 简短说明（越高越严、过滤更多低相关项）。改后经既有 `update_settings` 写 settings.json。下次查询 live-read 生效（无需重启）。空/未设 → 后端用 0.30。

## 4. 数据流

- **查询**：query → 语义臂 `embed` → cosine 全候选 → `filter_rank_topk(scored, (floor_provider)(), TOP_K)`（floor 实时取自 settings.json）→ SearchResult（带 cosine score）→ 融合 → UI（match 列显「按意思找到 · 0.XX」）。
- **调阈值**：用户在设置页改数值 → `update_settings` 写 settings.json → 回搜索框重搜 → 闭包读到新值 → 过滤随之变化（免重启）。

## 5. 错误处理

- 阈值读失败 / 解析失败 / 未设 → fallback 0.30（与今天一致）。
- 阈值越界（<0 或 >1 或 NaN）→ clamp 到 [0,1]（NaN → clamp 取默认侧，实现用 `clamp` 前先 `is_finite` 判，否则 0.30）。
- `r.score` 为 null → 前端只显徽标、不显分数。
- 不碰 parser / 无模型 / feature 关路径 → 行为不变。

## 6. 测试

- **后端**：
  - 现有 `SemanticIndexBackend::new` 调用点（`semantic_query_ranks_by_cosine` / `no_embedder_is_unavailable_and_empty` / `semantic_floor_filters_low_relevance`）改三参，传常量闭包（如 `Arc::new(|| 0.30_f32)`）。
  - 新增 `floor_provider_controls_filtering`：同一组候选，provider 返回高下限（如 0.95）→ 过滤更多 / 返回 0.0 → 全保留，证明阈值经 provider 实时生效。
  - `filter_rank_topk` 纯函数单测不变（已覆盖）。
- **设置**：`AppSettings` 旧 json（无 `semantic_similarity_floor`）解析 ok、字段 `None`（向后兼容，扩现有 `old_settings_without_model_path_parses_ok` 同型测试）。
- **clamp**：desktop 侧 floor 解析的 clamp 逻辑抽小函数单测（越界/NaN/正常 → 期望值）。
- **前端**：`tsc --noEmit` 净。
- **回归（硬门）**：evals v0.5=473/v0.9=726 byte-equal（不碰 parser）；`cargo test --workspace` 零失败；clippy `-D warnings` 0；fmt 净。
- **真机手测（登记，留用户）** → manual-test-scenarios 簇 A-1 续节：语义结果显分数；设置页调高下限→重搜→低分项消失、徽标只留高分命中；调到满意值记录之。

## 7. 平台

与平台无关，macOS + Windows 同一份代码。本轮主要服务 Windows 真机 tuning。

## 8. 验收标准

1. 语义结果在「匹配方式」列显「按意思找到 · 0.XX」（cosine 2 位小数）。
2. 设置页可改语义相似度下限；改后重搜即生效（免重启），过滤随之变化。
3. 未设/读失败 → 0.30（未碰设置的用户行为不变）；阈值 clamp [0,1]。
4. evals byte-equal；全 workspace test / clippy / fmt / tsc 全绿。
5. 真机手测登记（用户据可见分数把 0.30 调到合适值，反馈数值供 bake 默认）。

## 9. 未尽 / 后续

- 用户调出的合适阈值 → 后续 bake 为新默认常量（一行）。
- 阈值控件 / 分数显示是否长期保留（vs 收进高级 / 调好后移除）——调参完成后再定。
- 簇 A 剩余（held-out 评测自动定阈值 + 权重调优 + 原始 query 入 schema）仍待，隐私门 + 低 ROI，按需。
