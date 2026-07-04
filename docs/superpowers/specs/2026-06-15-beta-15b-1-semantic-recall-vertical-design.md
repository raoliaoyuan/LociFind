# BETA-15B-1 设计：语义召回纵切 MVP（旗舰语义召回层第一刀）

> 类型：**生产纵切（vertical slice）**，产出是落到真实桌面 app、用户可见的端到端语义召回链路。
> 关系：BETA-15B 旗舰化（"本地语义召回层"）已由用户 2026-06-15 拍板**进取档**（PROJECT.md 一句话定位 + 边界已同步修订）。旗舰化是跨多子系统的大工程，按 superpowers 分解为四个子项；本 spec 只覆盖**第一个子项 15B-1**。前置探针 [BETA-26](./2026-06-14-beta-26-semantic-retrieval-quality-spike-design.md) 结论 **GO**（纯模糊子集 FTS5 Recall@10 2.1%→hybrid 88.4%、crosslang +100pp、exact-name 守护 0/12 回退），方法学与评测锚点已就绪。
> 边界：进取档下，"按意思 / 跨语言模糊召回"是**差异化旗舰能力**，但仍守"不替代系统搜索"——语义召回是**叠加在 FTS5/系统搜索之上的一层**，不是另起全文引擎（详 PROJECT.md line 84）。

## 1. 背景与目标

BETA-26 已用数据回答了"质量够不够"=够，且差距是质变。但探针是在一次性丢弃 crate（`packages/spike-retrieval`）+ 离线 eval 里跑的，**从未碰真实 app 的索引/搜索/UI**。BETA-15B 旗舰化的第一刀，就是把这条已验证的链路**第一次落到产品主搜索流程**，让用户在桌面 app 里真正用上"按意思找文件"，尤其是跨语言这个 FTS5/Spotlight/Everything 都给不了的体感。

**唯一目标**：文档臂的语义召回端到端打通、接入桌面 app、用户可见、可手测；feature flag + 模型存在性双重门控，关闭时与今天行为逐字节一致。

**唯一生产代码遗产已就位**：`model-runtime` 的 `embed()`（Qwen3-Embedding-0.6B，dim 1024，last-token pooling + L2 归一化，探针期双审过）。本切片在其上建存储、backend、融合、UI。

## 2. 范围护栏（YAGNI——本切片不做什么）

旗舰化是大工程，本切片严格只做**最小完整纵切**，其余三子项各自走 spec→plan→实现：

| 本切片做（15B-1） | 留给后续子项 |
|---|---|
| 文档臂语义召回（office/pdf/md/txt 等已抽正文的文档） | 音乐/媒体臂、OCR 图片臂 → **15B-4** |
| BLOB 列 + Rust 暴力 cosine 持久化 | sqlite-vec + int8 量化 → **15B-4** |
| 约定目录 + 懒加载 embedding 模型（镜像 BETA-23） | 打包 / 首次运行自动下载分发 → 后续产品化 |
| 内联进现有文档增量索引 | 后台 / 批处理 / 防抖调度 → **15B-2** |
| 固定默认加权 RRF 融合 | held-out 评测扩量 + 权重/置信度路由调优 → **15B-3** |
| macOS 优先（Metal embed 已验证） | Windows embedding（CPU/Vulkan） → **15B-4** |

**锁定决策**（更改会作废 BETA-26 评测锚点或扩大范围，本切片不动）：embedding 模型固定 Qwen3-Embedding-0.6B / dim 1024；整篇首 1200 字截断、**不分块**（BETA-26 §4.5 实测分块在此类数据 wash-to-略负且成本 7.5×）。

## 3. 架构与组件

数据沿用现有多源融合骨架（BETA-04 fanout + result-normalizer + BETA-05 ranker），新增一条**语义召回臂**，与 FTS5 臂在 fan-out 里并列、加权 RRF 融合，全程 feature + 模型存在性双重门控。

### 3.1 Embedding 模型句柄（复用 `embed()`，新增懒加载装配）

- `model-runtime` 的 `embed()` 已就绪（`packages/model-runtime/src/llama.rs`，`#[cfg(feature = "llama-cpp")]`，worker 线程 `Request::Embed`，返回 L2 归一化 `Vec<f32>`）。**本切片不改 `embed()` 本身**。
- 新增一个**独立于生成/fallback 模型的 `EmbeddingModel` 懒加载句柄**，镜像 BETA-23 `model_fallback.rs` 的约定目录 + 懒加载 + 路径覆盖 + 单飞（BusyGuard）机制：
  - 约定目录放 embedding GGUF（与生成模型不同文件）；启动不加载，首次需要时加载。
  - 模型缺失 / 加载失败 → 语义臂静默禁用，退回今天的 FTS5-only 行为，**不报错**；状态在设置/隐私面板显示（镜像 fallback 模型的三态）。
- 与生成模型并存意味着两个 GGUF、两份常驻内存——MVP 接受（仅在用户放了 embedding 模型时才常驻），内存可控性属门槛③、本切片只记录实测值不优化。

### 3.2 向量存储（indexer，新表）

同一个 SQLite 文件（与 `documents`/`documents_fts` 同库）新增表：

```sql
CREATE TABLE IF NOT EXISTS document_vectors (
  doc_id        INTEGER PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
  dim           INTEGER NOT NULL,
  vector        BLOB NOT NULL,        -- f32 小端连续数组（dim × 4 字节）
  embed_model   TEXT NOT NULL,        -- 模型标识；换模型→旧向量视为陈旧
  source_hash   TEXT NOT NULL,        -- 截断正文的 hash，正文没变就跳过重嵌
  embedded_time INTEGER NOT NULL
);
```

- BLOB serde：f32 小端连续数组，读写各一个纯函数（单测 round-trip）。
- `ON DELETE CASCADE` 接 `documents` 主键：文档被 prune 删除时向量自动清。注意需开启 `PRAGMA foreign_keys = ON`（现有连接是否已开要核实，否则在 prune 路径显式 cascade delete）。
- `source_hash` 命中即跳过重嵌（增量友好）；`embed_model` 不匹配当前模型 → 视为陈旧、按需重嵌。

### 3.3 内联嵌入（接 `run_incremental_index`）

- 扩展现有文档增量索引的 upsert 路径（`packages/indexer/src/scan.rs::run_incremental_index` + `doc_db.rs`）：文档 upsert 正文后，**若 embedding 模型已加载**，对截断正文（首 1200 字、不分块）`embed()` → 存 `document_vectors`。`source_hash` 命中则跳过。
- 删除 / 陈旧由现有 prune（`IncrementalStore` 的 `delete_by_path` + `paths_under`）+ cascade 处理。
- **单篇 embed 失败 → 记日志、跳过该向量**（文档仍可被 FTS 搜到），**不中断整轮索引**（沿用现有 `catch_unwind` 的"一篇崩不拖垮全量"哲学）。
- 已知代价：embedding 慢（BETA-26 实测 5000 篇 / 5.2min，Metal），会拖长一次 reindex——仅在 opt-in（放了模型）时发生。批处理 / 后台化是 **15B-2**，本切片不优化。

### 3.4 语义 backend

- 新 `SemanticIndexBackend` 实现 `SearchBackend`（`packages/search-backends/` 下新 crate 或并入 local-index，实现时定）。
- 新增 `BackendKind::SemanticIndex` 枚举变体（**采纳新增枚举而非复用 `NativeIndex`**——路由 `backend_indexes_content()` 与源标注更清晰，代价是多动几处 match）。
- 新增 `MatchType::Semantic`。
- `search_expanded`：`embed(query)` → 加载候选向量 → 暴力 cosine → top-K（K、POOL 取 BETA-26 同款 10/50）→ 产出带 `MatchType::Semantic` 的 `SearchResult` 流，携带 cosine 相似度分。
- 接入 `intent_router` 的 `backend_indexes_content()` 路由，使其与 FTS 臂一同进 content fanout。

### 3.5 Hybrid 融合（本切片最可能有架构摩擦处）

- 语义臂参与 BETA-04 fanout，与 FTS 臂各出一个**有序**列表。
- **架构摩擦点**：现有 `result-normalizer::merge_results` 按路径合并（dedup + max score + 源并集），**不保留 per-backend rank**，无法直接做 RRF。需新增一个**保留每个 backend 排名 → 加权 RRF 融合 → 再交给 merge_results 去重**的融合层（落在 harness fanout 或 result-normalizer，实现时定）。
- 融合算法：从 `spike-retrieval` 移植 `rrf_fuse` / `weighted_rrf_fuse` 纯函数（带单测）。MVP 用**固定默认加权 RRF**，取 BETA-26 §4.6 验证过的"偏向量"权重作默认值（该实验显示偏向量能把纯模糊追到 0.925、exact-name 守护仍 1.0，但总体掉 −2.8pp——三目标有真实张力）。
- **明确推迟到 15B-3**：权重调优 + FTS 置信度阈值路由（BETA-26 发现"FTS5 零命中→向量"在 trigram 下形同虚设，须用 top BM25 分数阈值）。本切片用固定默认值，不据 BETA-26 样本内数字做精调（须 held-out 评测集，属 15B-3）。

### 3.6 Ranker 集成

- 语义结果携带 cosine 相似度分；融合后的 RRF 排名作为顺序进 BETA-05 ranker。
- 保持最小改动：RRF 融合后的顺序为主序；ranker 的显式排序（SortOrder）仍照常覆盖。语义分如何并入 `relevance_score()` 公式取最小侵入方案，实现时定（优先不破坏现有相关性公式的回归基线）。

### 3.7 UI 呈现

- 语义命中在结果上显式标注 **"按意思找到 / by meaning"**（用 `MatchType::Semantic`），让跨语言/模糊命中可解释——符合 PROJECT.md"可解释可控"原则，也是 crosslang 这个最硬差异化卖点的体感落点。
- 设置/隐私面板显示 embedding 模型状态（已就绪 / 未找到 + 放置提示），镜像 BETA-23 fallback 模型状态行。

### 3.8 Feature flag + 优雅降级

- 全部新代码在 `semantic-recall` feature 后（链 `model-runtime/llama-cpp`，metal 形态链 metal）。
- feature 关 **或** 模型缺 → 搜索行为与今天**逐字节一致**（现有 evals byte-equal 作硬门）。

## 4. 数据流

- **索引**：文件 → 抽正文（BETA-02）→ upsert `documents`/`documents_fts` →（模型在场且 `source_hash` 未命中）`embed(截断正文)` → 存 `document_vectors`。
- **查询**：query → fanout →〔FTS5 臂 → 有序表〕+〔语义臂：`embed(query)` → cosine topK → 有序表〕→ 加权 RRF 融合（保留 per-backend rank）→ `merge_results` 路径去重（源并集含 SemanticIndex）→ ranker → UI 带 provenance。

## 5. 错误处理

- **模型缺失 / 加载失败** → 语义臂静默禁用、FTS-only，无用户报错，状态在设置/隐私面板显示。
- **单篇 embed 失败** → 跳过该向量、不中断索引（文档仍 FTS 可搜）。
- **query embed 失败 / 超时** → 该次查询退回 FTS-only 结果（镜像 BETA-23 fallback 的"永不失败"超时哲学）。
- **维度 / 模型不匹配**（换过模型）→ `embed_model` 字段不符 → 旧向量视为陈旧/缺失，按需重嵌；查询期遇维度不符的候选直接跳过。
- **外键 / cascade**：确认 `PRAGMA foreign_keys` 状态，否则 prune 路径显式删向量，避免悬挂行。

## 6. 测试

- **单元**：cosine / 加权 RRF 纯函数（从 spike 移植 + 单测）、BLOB serde round-trip、`document_vectors` upsert / delete / cascade / source_hash 跳过逻辑。
- **集成**：小 fixture 语料端到端——索引带 embed → 语义 query 命中 FTS 漏掉的模糊 / 跨语言 case（合成或 gitignored 真实子集，守 CONVENTIONS §7 隐私）。
- **回归（硬门）**：
  - feature **关** 时现有 evals **byte-equal**（v0.5=473 / v0.9=726 parser-only 不动；语义臂不碰 parser，应天然守住）。
  - feature 开 + 模型在场时 hybrid **不拖垮 exact-name**（镜像探针守护桶 0 回退）。
- BETA-26 的 68 条 5 桶评测集作相关性锚点（eval-only；held-out 扩量是 15B-3）。
- macOS 真机手测场景登记到 [docs/manual-test-scenarios.md](../../manual-test-scenarios.md)：放 embedding 模型 → reindex → 跨语言 query（中文 query 命中英文文档）出 "按意思找到" 标注 + 不放模型时 FTS-only 无差异。

## 7. 平台

macOS 优先（Metal embed 已 BETA-26 验证）。Windows embedding（CPU/Vulkan）= 15B-4。

## 8. 验收标准

1. feature 关 / 模型缺：搜索行为与今天逐字节一致（evals byte-equal + 现有 backend 测试零回归）。
2. feature 开 + 模型在场：
   - 文档增量索引顺带产出向量，删除/陈旧正确清理。
   - 跨语言 query（中文→英文文档）在桌面 app 返回 FTS5 结构性给不出的命中，并标注 "按意思找到"。
   - exact-name 不回退（守护桶 0 回退）。
3. 全 workspace `fmt --check` / `clippy -D warnings` 0 / `cargo test --workspace`（含新 feature 形态）零失败。
4. 真机手测场景通过（登记，留用户执行）。
5. 实测并记录：reindex 增量耗时、向量库体积、常驻内存（供 15B-2/门槛③ 参考，本切片只记录不优化）。

## 9. 未尽 / 后续子项交接

- **15B-2**：向量索引后台 / 批处理 / 防抖调度，解决"reindex 变慢"。
- **15B-3**：held-out 评测扩量 + 加权 RRF 权重调优 + FTS 置信度阈值路由。
- **15B-4**：Windows embedding、音乐/媒体/OCR 臂、sqlite-vec + int8、探更大模型天花板。
- 分发产品化（打包 / 自动下载）使旗舰默认可用——独立决策。
