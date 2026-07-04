# BETA-04 Result Normalizer Implementation Plan

> Steps use checkbox (`- [ ]`). 每 task 末尾过 fmt + clippy(-D warnings) + test。

**Goal:** 把音乐/文档本地索引接成 `LocalIndexBackend`（NativeIndex），搜索流升级为 fan-out 多源 + Result Normalizer 归一合并，加 reindex 命令，让「找周华健的歌」「找含 X 的文档」端到端走通。

**Architecture:** 见 [spec](../specs/2026-06-02-beta-04-result-normalizer-design.md) §4。两新 crate（result-normalizer / local-index）+ harness fanout + desktop 接线。无新外部依赖。

**Tech Stack:** Rust；locifind-search-backend(common) / locifind-indexer / futures。

---

## Task 1: `packages/result-normalizer` crate — merge_results

**Files:** `packages/result-normalizer/{Cargo.toml,src/lib.rs}`（新）、根 `Cargo.toml` members。

- [ ] **Step 1:** 根 Cargo.toml members 加 `"packages/result-normalizer"`。
- [ ] **Step 2:** Cargo.toml：依赖 `locifind-search-backend`（path）。
- [ ] **Step 3:** lib.rs：`MergedResult { result, sources, match_types }` + `merge_results(Vec<SearchResult>) -> Vec<MergedResult>`（按 path dedup；代表取非空 metadata 字段计数最高者；sources/match_types 稳定去重并集；score 取 max；保持首现序）。helper `metadata_richness(&SearchResult) -> usize`。
- [ ] **Step 4:** 单测：跨源同 path 合并（sources 并集 / match_types 并集 / 代表取富 metadata / score max）；不同 path 各自保留；空输入；首现序保持；单源直通。
- [ ] **Step 5:** fmt + clippy + test。
- [ ] **Step 6:** Commit `feat(result-normalizer): BETA-04 多源结果归一化合并`。

## Task 2: indexer busy_timeout + `local-index` crate — LocalIndexBackend

**Files:** `packages/indexer/src/{db,doc_db}.rs`（busy_timeout 小改）、`packages/search-backends/local-index/{Cargo.toml,src/lib.rs}`（新）、根 Cargo.toml members。

- [ ] **Step 1:** indexer `db.rs` + `doc_db.rs` 的 `from_conn`：`conn.busy_timeout(Duration::from_secs(5))?` 后再建表。BETA-01/02 测试不回归。
- [ ] **Step 2:** 根 members 加 `"packages/search-backends/local-index"`。
- [ ] **Step 3:** Cargo.toml：依赖 common / indexer / futures-core / futures-util。
- [ ] **Step 4:** lib.rs：`LocalIndexBackend { db_path }` + `new` + `reindex(music_roots, doc_roots) -> Result<(IndexStats,IndexStats), SearchError>`（开 MusicIndex/DocumentIndex、index_dirs）。
- [ ] **Step 5:** impl `SearchBackend`：kind=NativeIndex / is_available=true / search 翻译（spec §4.2）。helper：`music_results(intent, &MusicIndex) -> Vec<SearchResult>` / `doc_results(intent, &DocumentIndex) -> Vec<SearchResult>` / `canonical(path)`。MediaSearch(audio)→music、FileSearch(有 keyword)→doc、其余空流、Refine/FileAction/Clarify→UnsupportedIntent。复用 `backend_stream_from_results`（在 common？确认；否则本地 unfold）。
- [ ] **Step 6:** 单测（temp db）：写音乐样本 → MediaSearch(audio, artist) 命中 + metadata(artist/duration) 正确 + path canonical；写文档样本 → FileSearch(keyword) 命中；Image media → 空；空 keyword FileSearch → 空；Refine → UnsupportedIntent；reindex 后 search 命中。
- [ ] **Step 7:** fmt + clippy + test（indexer + local-index）。
- [ ] **Step 8:** Commit `feat(local-index): LocalIndexBackend 把音乐/文档索引包成 SearchBackend`。

## Task 3: Harness fan-out + merge

**Files:** `packages/harness/src/intent_router.rs`（route_search_fanout）、`packages/harness/src/fanout_merge.rs`（新）、`packages/harness/src/lib.rs`（mod + re-export）、`packages/harness/Cargo.toml`（加 result-normalizer 依赖）。

- [ ] **Step 1:** Cargo.toml 加 `locifind-result-normalizer`（path）。
- [ ] **Step 2:** `route_search_fanout(&expanded) -> Result<Vec<Arc<dyn SearchableTool>>, RouteError>`：内容/媒体（复用 route_search_expanded 的判定）→ 全部 content-capable available 候选；否则 → [首个 available]。空候选 → RouteError::NoBackend。
- [ ] **Step 3:** `fanout_merge.rs`：`FanoutOutcome` + `run_fanout_merge(backends, expanded, cancel, on_result)`：对每 backend `search_expanded`→collect（取消即停、错误记 errors）→ `merge_results` → 逐条 on_result；返回 outcome（total/sources_queried/errors）。
- [ ] **Step 4:** lib.rs `mod fanout_merge; pub use ...`。
- [ ] **Step 5:** 单测（mock SearchableTool，复用 fallback_chain 测试的 mock 模式）：route 内容→2 后端 / 纯文件名→1；fanout 合并去重（两 mock 各返部分重叠 path）、部分失败仍合并其余、全失败 total=0、取消停。
- [ ] **Step 6:** fmt + clippy + test（harness）。
- [ ] **Step 7:** Commit `feat(harness): fan-out 多源查询 + route_search_fanout`。

## Task 4: Desktop 接线（register + reindex + fanout 路由）

**Files:** `apps/desktop/src-tauri/src/{main,search}.rs`（+ `Cargo.toml` 加 local-index/result-normalizer）。

- [ ] **Step 1:** Cargo.toml 加 locifind-local-index-backend / locifind-result-normalizer / locifind-indexer（reindex 命令需 default roots）。
- [ ] **Step 2:** `main.rs build_registry`：两平台注册 `LocalIndexBackend::new(data_dir/LociFind/index.db)`（id `search.local`，supported FileSearch+MediaSearch）；`SearchDeps` 加 `local_index_db: PathBuf`（或 Arc<LocalIndexBackend> 供 reindex 复用）。
- [ ] **Step 3:** `reindex` command：`LocalIndexBackend::reindex(default_music_roots(), default_document_roots())` → 返回统计 JSON；注册进 invoke_handler。
- [ ] **Step 4:** `search` command：内容/媒体 intent 走 `route_search_fanout` + `run_fanout_merge`；`SearchResultJson` 加 `sources: Vec<String>`；on_result 发 `MergedResult`。纯文件名维持原 fallback。
- [ ] **Step 5:** desktop 既有测试适配（SearchResultJson 加字段）+ 至少 1 新测（如有可单测的纯函数：merged→json 映射）。
- [ ] **Step 6:** fmt + clippy + test（desktop）。
- [ ] **Step 7:** Commit `feat(desktop): 注册 LocalIndexBackend + reindex 命令 + 内容查询走 fan-out`。

## Task 5: 文档 + 全套 CI + 收尾

**Files:** 两 crate README、`docs/manual-test-scenarios.md`、`docs/third-party-licenses.md`（注明新内部 crate，无新外部依赖）、`ROADMAP.md`、`STATUS.md`。

- [ ] **Step 1:** `packages/result-normalizer/README.md` + `packages/search-backends/local-index/README.md`。
- [ ] **Step 2:** manual-test-scenarios.md 加 BETA-04 节（reindex → 找周华健的歌 / 找含 X 文档 / sources 多源显示）。
- [ ] **Step 3:** 台账注明两新内部 crate（无新 crates.io 依赖）。
- [ ] **Step 4:** `bash scripts/ci.sh`（platform-macos Windows 预存失败除外）+ 全 workspace test 零回归（evals 472/26/2、harness fallback 测试不动）。
- [ ] **Step 5:** ROADMAP BETA-04 → done + 实证；§2 / B2 注。
- [ ] **Step 6:** STATUS 当前阶段 + 会话日志。
- [ ] **Step 7:** 收工 commit + 向用户确认。

---

## 验收对照（spec §5）

- merge_results 纯函数（T1）；LocalIndexBackend temp-index（T2）；route_search_fanout + run_fanout_merge mock（T3）；desktop register+reindex+fanout（T4）；docs + ci 零回归（T5）。
