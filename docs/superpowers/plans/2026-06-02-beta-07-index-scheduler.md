# BETA-07 后台索引调度 Implementation Plan

> Steps use checkbox (`- [ ]`). 每 task 末尾过 fmt + clippy(-D warnings) + test。

**Goal:** 启动后台自动索引（非阻塞）+ stale 回收（删除已不存在文件记录）+ 索引状态可见。保留手动「立即索引」。

**Architecture:** 见 [spec](../specs/2026-06-02-beta-07-index-scheduler-design.md) §3。indexer `prune_deleted` + reindex 回收；desktop IndexStatus + 并发守卫 + 后台启动索引 + 状态命令/UI。

**Tech Stack:** Rust；无新外部依赖。

---

## Task 1: stale 回收（MusicIndex::prune_deleted + reindex 接入）

**Files:** `packages/indexer/src/db.rs`（prune_deleted）+ `packages/search-backends/local-index/src/lib.rs`（reindex_with 调 prune）+ 测试。

- [ ] **Step 1:** db.rs `MusicIndex::prune_deleted(&self) -> Result<u64, IndexError>`：`SELECT path FROM music` 收集 → 对 `!Path::new(&p).exists()` 的 `delete_by_path(&p)` → 计数返回。
- [ ] **Step 2:** db.rs 单测：插入 2 条（一条 path 指真 temp 文件、一条指不存在路径）→ prune_deleted 返 1、count 减 1、FTS 同步（查不存在那条的关键词无果）。
- [ ] **Step 3:** local-index `reindex_with`：音乐发现分支 `index_paths` 后 `music.prune_deleted().map_err(to_search_err)?`（回退 index_dirs 分支不加，已回收）。
- [ ] **Step 4:** local-index 单测：mock discovery 返回 [存在文件]，先 reindex 入库；再 mock 返回 [] + 该文件删除 → reindex 后 prune 掉（count 0）。或直接测 prune 经 reindex 生效。
- [ ] **Step 5:** fmt + clippy + test（indexer + local-index）。
- [ ] **Step 6:** Commit `feat(indexer): MusicIndex::prune_deleted 回收已删文件 + reindex 接入`。

## Task 2: IndexStatus + 并发守卫 + 后台启动索引（desktop）

**Files:** `apps/desktop/src-tauri/src/{search,main}.rs` + 测试。

- [ ] **Step 1:** search.rs `IndexStatus { indexing, last_indexed, last_summary }`（Serialize/Clone/Default）；`SearchDeps` 加 `index_status: Arc<Mutex<IndexStatus>>`（`new()` 默认 + `index_status()` getter；不动 37 调用点——字段默认 `Arc::new(Mutex::new(Default))`）。
- [ ] **Step 2:** `perform_reindex(status: &Arc<Mutex<IndexStatus>>, db: PathBuf) -> Option<ReindexStats>`（移到 search.rs 或 main.rs）：并发守卫（锁内查 indexing，true→返 None；否则置 true）；`LocalIndexBackend::new(db).reindex(default_music_roots(), default_document_roots())`；finally 置 indexing=false + last_indexed=now + last_summary。`ReindexStats` 复用/移到 search.rs。
- [ ] **Step 3:** main.rs `reindex` 命令改：`State<SearchDeps>` + `spawn_blocking(perform_reindex(status_clone, db))`；已在索引返回友好提示。新 `get_index_status(deps) -> IndexStatus` 命令 + 注册。
- [ ] **Step 4:** main.rs `setup()`：spawn 后台任务 `spawn_blocking(perform_reindex(status, db))` 启动即跑（不阻 UI）。
- [ ] **Step 5:** 测试：`perform_reindex` 并发守卫（status indexing=true → None）；成功后 status 更新（用 temp db + 真小文件或空 roots，断言 indexing=false + last_indexed Some）；get_index_status impl。既有 desktop 测试零回归（SearchDeps 加字段默认）。
- [ ] **Step 6:** fmt + clippy + test（desktop）。
- [ ] **Step 7:** Commit `feat(desktop): 启动后台自动索引 + IndexStatus + 并发守卫`。

## Task 3: 设置页状态 UI + 文档 + 全套 CI

**Files:** `apps/desktop/src/pages/SettingsPage.tsx` + `packages/indexer/README.md` + `docs/manual-test-scenarios.md` + ROADMAP/STATUS。

- [ ] **Step 1:** SettingsPage「本地索引」节：加载 `get_index_status`，显示「正在索引…」/「上次索引: <time>（音乐 N / 文档 M）」；轮询（indexing 时每 2s 刷新）或刷新按钮。tsc 通过。
- [ ] **Step 2:** indexer README prune_deleted 一句 + BETA-07 后台索引说明；local-index README reindex 回收一句。
- [ ] **Step 3:** manual-test BETA-07（启动不点按钮→稍等→本地搜有结果；设置页见上次索引；删文件再启动→消失）。
- [ ] **Step 4:** `bash scripts/ci.sh`（platform-macos 预存除外）+ 全 workspace test 零回归。无新外部依赖。
- [ ] **Step 5:** ROADMAP BETA-07 → done + §2；STATUS 当前阶段 + 会话日志。
- [ ] **Step 6:** 收工 commit + 向用户确认（真机手测留用户）。

---

## 验收对照（spec §4）

- prune_deleted + reindex 回收（T1）；IndexStatus + 守卫 + 后台启动（T2）；状态 UI + docs + ci（T3）。
