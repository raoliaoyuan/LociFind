# BETA-04 Result Normalizer（多源融合）— 设计

> 状态：draft（待用户 review）
> 关联：ROADMAP §3.3 B2 BETA-04；承接 BETA-01/02 本地索引；PROJECT 架构「Result Normalizer + Ranker」
> ID：BETA-04

## 1. 背景与目标

BETA-01/02 建好了音乐 / 文档本地索引，但**没接进 Agent**——`MusicIndex`/`DocumentIndex` 完全独立，搜索链路不查它们。本任务把它们接成可搜后端，并把搜索流从「fallback 选一个后端」升级为「**fan-out 多源查询 + 归一化合并**」，让「找周华健的歌」「找含季度预算的文档」端到端走通（本地索引补上系统搜索按 artist/正文搜不到的命中）。

排序（BM25 / 启发式打分）留 [BETA-05 Ranker](./)；本任务合并后按简单规则排序（命中源数 + score + mtime），把「合并去重 + 归一化 + 来源溯源」这一层做扎实。

## 2. Brainstorming 决策（已与用户对齐）

| # | 决策 | 选择 |
|---|---|---|
| ① | 多源策略 | **fan-out + 归一合并**：内容/媒体查询同时查系统后端 + 本地索引，结果经 Result Normalizer 去重合并 |
| ② | 索引填数据 | **显式 reindex 命令**（tauri command 手动触发扫音乐+文档目录），后台自动调度留 BETA-07 |
| ③（spec 定） | 组件落位 | `LocalIndexBackend` → 新 crate `packages/search-backends/local-index`；归一化 → 新 crate `packages/result-normalizer`（match ROADMAP / PROJECT 架构） |

## 3. 关键约束 / 架构决策

- **`SearchBackend: Send + Sync`，但 rusqlite `Connection: !Sync`** → `LocalIndexBackend` **不能持久持有连接**，改为**持 db 路径、每次 `search()` 内部开连接**（`open()` 幂等 `CREATE TABLE IF NOT EXISTS`），查完构造 `Vec<SearchResult>` 后连接 drop，再 `backend_stream_from_results` 出流（与 Spotlight eager 模式一致）。
- **路径规范化由 backend 负责**（架构既定，见 fallback_chain 注释）：`LocalIndexBackend` 产 `SearchResult` 时对 path 做 `fs::canonicalize`（失败回退原值），与 Spotlight `result_from_path` 一致 → 保证跨源 dedup 的 path 字节一致。
- **Result Normalizer 是纯函数**（无 IO）：按 `path` dedup + 合并来源/match_type/metadata。
- **无新外部依赖**：两新 crate 仅依赖 `locifind-search-backend`(common) / `locifind-indexer` / `futures-*`（均在树）。
- **SQLite 并发**：reindex 写与 search 读可能短暂并发 → indexer `from_conn` 加 `busy_timeout(5s)`（小改，两索引共用）。

## 4. 架构

### 4.1 `packages/result-normalizer`（新 crate `locifind-result-normalizer`）

```rust
/// 合并后的一条结果：代表结果 + 多源溯源。
pub struct MergedResult {
    pub result: SearchResult,         // 代表结果（metadata 最丰富者）
    pub sources: Vec<BackendKind>,    // 命中此 path 的所有后端（去重，稳定序）
    pub match_types: Vec<MatchType>,  // 命中类型并集（Filename/Content/Metadata/Ocr）
}

/// 按 canonical path 去重合并多源结果。保持首现顺序（排序留 BETA-05）。
/// 合并规则：代表结果取 metadata 最丰富者（非空字段计数最高）；sources / match_types
/// 取并集（稳定去重）；score 取最大。
pub fn merge_results(results: Vec<SearchResult>) -> Vec<MergedResult>;
```

纯函数，完全可单测（喂构造的 `SearchResult`）。

### 4.2 `packages/search-backends/local-index`（新 crate `locifind-local-index-backend`）

```rust
pub struct LocalIndexBackend {
    db_path: PathBuf,   // 音乐 + 文档表共用一个 sqlite 文件
}

impl LocalIndexBackend {
    pub fn new(db_path: impl Into<PathBuf>) -> Self;
    /// 手动索引：扫音乐 + 文档目录（reindex 命令调用）。返回 (music_stats, doc_stats)。
    pub fn reindex(&self, music_roots: &[PathBuf], doc_roots: &[PathBuf])
        -> Result<(IndexStats, IndexStats), SearchError>;
}

impl SearchBackend for LocalIndexBackend {
    fn kind(&self) -> BackendKind { BackendKind::NativeIndex }
    fn is_available(&self) -> bool { true }   // 本地索引恒可用
    fn search(...) -> BackendSearchFuture;     // 见翻译
}
```

**intent → 索引查询翻译**（`search` 内开连接）：
- **MediaSearch{ media_type: Audio, .. }** → `MusicIndex::query`：
  - `MusicQuery.text` = artist / title / keywords 中最salient项拼接；`artist` / `album` 映射结构化字段。
  - `MusicEntry` → `SearchResult`：path（canonicalize）、name=file_name、source=NativeIndex、match_type=Metadata、metadata 填 artist/title/album/duration_seconds/modified_time。
- **FileSearch{ keywords, .. }** → `DocumentIndex::query`：
  - `DocumentQuery.text` = keywords 拼接；`doc_type` 由 extensions/file_type 推（可选，v1 仅 text）。
  - `DocumentHit` → `SearchResult`：path（canonicalize）、name、source=NativeIndex、match_type=Content、metadata 填 modified_time（snippet v1 暂丢，无 SearchResult 字段）。
- **MediaSearch{ media_type: Image/Video/Screenshot }**（本地无图像索引）/ **无 keyword 的纯扩展名 FileSearch** → 空流（不贡献，交系统后端；非 Err，便于 fan-out 合并）。
- **Refine / FileAction / Clarify** → `Err(UnsupportedIntent)`（与其他 backend 一致）。
- `search_expanded` → 默认 fallback 到 `search(&expanded.base)`（v1 不在本地索引层做同义词；系统后端已覆盖；后续可加）。

### 4.3 Harness：fan-out + merge

新增（与 `run_fallback_chain` 并列，不动后者）：

```rust
// intent_router.rs：返回「该一起查的后端集合」（fan-out），而非单个/有序回退。
pub fn route_search_fanout(&self, expanded: &ExpandedSearchIntent)
    -> Result<Vec<Arc<dyn SearchableTool>>, RouteError>;
//  内容/媒体 intent（base 需内容 或 keyword_groups 非空）→ 全部 content-capable 后端
//     （Spotlight/WindowsSearch/NativeIndex 中 available 者）；
//  纯文件名/扩展名 → [单个首选]（Everything/Spotlight），退化为 1 元 fan-out。

// fanout_merge.rs：并发查询集合内所有后端，收集 → merge_results → 回调发出。
pub async fn run_fanout_merge<R>(
    backends: &[Arc<dyn SearchableTool>],
    expanded: &ExpandedSearchIntent,
    cancel: CancellationToken,
    on_result: &mut R,                 // FnMut(MergedResult)
) -> FanoutOutcome
where R: FnMut(MergedResult) + Send;

pub struct FanoutOutcome {
    pub total: usize,                  // 合并去重后结果数
    pub sources_queried: Vec<BackendKind>,
    pub errors: Vec<(BackendKind, String)>,  // 各后端错误（部分失败不致命）
}
```

- 并发：对每个 backend `search_expanded` 取流、`collect` 成 `Vec<SearchResult>`（取消即停）；各 backend 错误记入 `errors`、不中断其他；全部收齐后 `merge_results` 合并。
- 失败语义：所有 backend 都失败或零结果 → `total=0`（调用方据此报错/空态）。

### 4.4 Desktop 接线

- `build_registry()`：**两平台都注册** `LocalIndexBackend`（cross-platform 纯 Rust，无 cfg-gate）；db 路径 = `dirs::data_dir()/LociFind/index.db`。
- `search` command：内容/媒体 intent 走 `route_search_fanout` + `run_fanout_merge`；结果经 `MergedResult` → `SearchResultJson`（加 `sources` 字段供 UI 显示「via spotlight + 本地索引」）。**纯文件名查询维持原 fallback 路径**（避免动稳定路径 + Everything 快通道）。
- 新增 `reindex` command：`LocalIndexBackend::reindex(default_music_roots, default_document_roots)`，返回统计（音乐/文档 added/updated/…）供 UI 提示。
- `SearchEvent`：`Result` 项 `SearchResultJson` 加 `sources: Vec<String>`；`Complete` 可加 `sources_queried`（可选）。

> Tauri UI 无法自动化测试 → 桌面接线最小化 + 手测 scenario 落 `docs/manual-test-scenarios.md`，真机手测留用户（沿用项目惯例）。harness/backend/normalizer 逻辑全部 mock/temp-index 单测覆盖。

## 5. 验收 / 验证门

1. **result-normalizer**：`merge_results` 单测——跨源同 path 合并（sources/match_types 并集、代表取最丰富 metadata、score 取大）；不同 path 不合并；空输入；顺序保持。
2. **local-index**：`LocalIndexBackend` 单测（temp db + 真 `MusicIndex`/`DocumentIndex` 写入样本）——MediaSearch(audio) 命中音乐、FileSearch 命中文档、path 经 canonicalize、metadata 正确、Image/空 keyword → 空流、Refine → UnsupportedIntent；`reindex` 写入后 search 命中。
3. **harness fanout**：`route_search_fanout`（内容→多后端 / 纯文件名→单后端）+ `run_fanout_merge`（mock 多后端：合并去重、部分失败仍合并、全失败 total=0、取消）单测。
4. **desktop**：编译 + `build_registry` 含 LocalIndexBackend；`reindex` command 存在；search 内容路径走 fanout。desktop 既有测试不回归。
5. **零回归**：`run_fallback_chain` 及其测试不动；evals 472/26/2、harness 既有测试全过；全 workspace test 除 platform-macos 预存 Windows 失败外全绿；fmt + clippy `-D warnings`。
6. **手测 scenario**：`docs/manual-test-scenarios.md` 加 BETA-04 节（reindex → 「找周华健的歌」命中本地音乐 + 「找含 X 的文档」命中本地文档 + sources 显示多源）。
7. **文档**：两新 crate README + workspace 接入 + ROADMAP BETA-04 done + STATUS。三方台账无新增外部依赖（仅注明两新内部 crate）。

## 6. 非目标（YAGNI）

- 不做 BM25 / 真排序打分（BETA-05 Ranker）。
- 不做后台自动索引调度（BETA-07）；仅显式 reindex。
- 不做「最近使用文件」快捷通道（ROADMAP BETA-04 可选项，留后续）。
- 不在本地索引层做同义词扩展（系统后端已覆盖）。
- 不改 `run_fallback_chain`（纯文件名查询维持）。
- 不做 OCR 源接入（BETA-03 未做）；normalizer 设计为源无关，OCR 后续增量接。
- snippet 不进 SearchResult（无字段）；后续 BETA-05 可加。

## 7. 风险与缓解

| 风险 | 缓解 |
|---|---|
| rusqlite `!Sync` 撞 `SearchBackend: Sync` | backend 持 db 路径、search 内开连接（§3） |
| 跨源 path 不一致致 dedup 失效 | backend 统一 canonicalize（与 Spotlight 同），normalizer 按规范化 path dedup |
| 动 desktop search.rs 破坏既有分层（refine/policy/synonym/fallback/tracing） | 只在「路由+执行」一步分流内容→fanout；上游与事件发射不动；纯文件名维持 fallback |
| reindex 写与 search 读并发 SQLITE_BUSY | indexer `from_conn` 加 busy_timeout(5s) |
| Tauri 不可自动测 | 逻辑层全 mock/temp 单测；UI 手测落 scenario 文档 |
| 空索引时 fan-out 体验 | LocalIndexBackend 空索引返回空流（非 Err），系统后端正常服务；UI 提示先 reindex |
