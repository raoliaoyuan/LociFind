# BETA-15B-2 向量索引后台预热 + 解耦调度 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 embedding 模型加载（暖机）从用户查询路径移走，并把语义嵌入 pass 从 FTS reindex 解耦成独立后台阶段、带可见进度；feature/模型双门控关闭时与今天逐字节一致。

**Architecture:** indexer 拆出 `embed_pending(roots, embedder, progress_cb)` 只跑补嵌循环；embedding 句柄加 `prewarm()` 后台阻塞 load；desktop `perform_reindex` 去掉内联嵌入只做 FTS（跑完即释放 `indexing` 守卫），新增 `spawn_semantic_index` 后台 worker（独立 `semantic_indexing` 守卫 → prewarm → embed_pending 更新 `IndexStatus` 进度）；启动后台任务 + `reindex` 命令两处接入。

**Tech Stack:** Rust（rusqlite / tauri async_runtime spawn_blocking）、React/TypeScript（Tauri invoke）。

参考 spec：[docs/superpowers/specs/2026-06-18-beta-15b-2-semantic-index-scheduling-design.md](../specs/2026-06-18-beta-15b-2-semantic-index-scheduling-design.md)

---

## File Structure

| 文件 | 职责 | 改动 |
|---|---|---|
| `packages/indexer/src/doc_db.rs` | 文档库 + 向量补嵌 | 抽出 `embed_pending`（带 progress 回调），`index_dirs_with_embedder` 改为薄封装 |
| `packages/indexer/src/scan.rs`（test 模块） | 现有 `StubEmbedder` 测试 | 新增 `embed_pending` 单测 |
| `apps/desktop/src-tauri/src/search/embedding_model.rs` | embedding 懒加载句柄 | 新增 `prewarm()` |
| `apps/desktop/src-tauri/src/search/index_status.rs` | 索引状态 + reindex 执行 | `IndexStatus` 加 3 字段；`perform_reindex` 去内联嵌入；新增 `semantic_*` 状态助手 + `semantic_index_pass` + `spawn_semantic_index` |
| `apps/desktop/src-tauri/src/main.rs` | tauri setup + 命令 | 启动任务 + `reindex` 命令两处接 `spawn_semantic_index` |
| `apps/desktop/src/pages/SettingsPage.tsx` | 索引设置 UI | `IndexStatus` TS 接口加 3 字段 + 渲染语义索引行 |

---

## Task A: indexer 抽出 `embed_pending`（带进度回调）

**Files:**
- Modify: `packages/indexer/src/doc_db.rs:428-459`（`index_dirs_with_embedder`）
- Test: `packages/indexer/src/scan.rs`（现有 `#[cfg(test)]` 模块，`StubEmbedder` 已在 `scan.rs:802` 附近）

- [ ] **Step 1: 写失败测试**

在 `packages/indexer/src/scan.rs` 的 test 模块末尾（现有 `index_dirs_with_embedder_*` 测试附近）追加。`StubEmbedder` 已存在（确定性 3 维向量）；若其只在某 test fn 内定义，提升为模块级 `struct StubEmbedder;` 复用。

```rust
#[test]
fn embed_pending_reports_progress_and_skips_current() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("idx.db");
    let docs = dir.path().join("docs");
    std::fs::create_dir_all(&docs).unwrap();
    std::fs::write(docs.join("a.txt"), "alpha body text").unwrap();
    std::fs::write(docs.join("b.txt"), "beta body text").unwrap();
    let roots = vec![docs.clone()];

    let idx = DocumentIndex::open(&db).unwrap();
    idx.index_dirs(&roots).unwrap(); // 先建 FTS（embed_pending 只补向量，不建 FTS）

    // 首轮：两篇待嵌，进度回调单调到 (2,2)。
    let mut seen: Vec<(usize, usize)> = Vec::new();
    let (embedded, failed) = idx
        .embed_pending(&roots, &StubEmbedder, &mut |done, total| seen.push((done, total)))
        .unwrap();
    assert_eq!((embedded, failed), (2, 0), "两篇都新嵌、零失败");
    assert_eq!(seen.last().copied(), Some((2, 2)), "进度终值 (total,total)");
    assert!(seen.windows(2).all(|w| w[0].0 <= w[1].0), "done 单调不减");

    // 二轮：全 vector_is_current 命中 → 待嵌 0、回调不触发。
    let mut seen2: Vec<(usize, usize)> = Vec::new();
    let (e2, f2) = idx
        .embed_pending(&roots, &StubEmbedder, &mut |d, t| seen2.push((d, t)))
        .unwrap();
    assert_eq!((e2, f2), (0, 0), "二轮无新嵌");
    assert!(seen2.is_empty(), "无待嵌时不回调");
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-indexer embed_pending_reports_progress -- --nocapture`
Expected: FAIL（`no method named embed_pending`）

- [ ] **Step 3: 实现 `embed_pending` + 重构 `index_dirs_with_embedder`**

在 `packages/indexer/src/doc_db.rs` 把现有 `index_dirs_with_embedder`（428-459）改为：

```rust
    /// 补嵌 `roots` 子树下缺向量 / 陈旧向量的文档。**不建 FTS**（调用方先跑 `index_dirs`）。
    /// 先数出待嵌总数，再逐篇嵌入，每篇回调 `progress(done, total)`（done 含本篇）。
    /// **单篇 embed 失败**计入 failed、跳过、不中断（文档仍 FTS 可搜，镜像 catch_extract 哲学）；
    /// **DB 写失败**（upsert_vector）向上传播中断整轮。返回 `(成功嵌入篇数, 失败篇数)`。
    pub fn embed_pending(
        &self,
        roots: &[std::path::PathBuf],
        embedder: &dyn crate::embed::TextEmbedder,
        progress: &mut dyn FnMut(usize, usize),
    ) -> Result<(usize, usize), IndexError> {
        let root_strs: Vec<String> = roots
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();

        // 先收集待嵌（vector_is_current 未命中）的 (path, truncated, hash)，得 total 供进度。
        let mut pending: Vec<(String, String, String)> = Vec::new();
        for path in self.paths_under(&root_strs)? {
            let Some(body) = self.body_of(&path)? else {
                continue;
            };
            let truncated =
                crate::embed::truncate_chars(&body, crate::embed::EMBED_TRUNCATE_CHARS).to_owned();
            let hash = crate::embed::content_hash(&truncated);
            if self.vector_is_current(&path, embedder.model_id(), &hash)? {
                continue;
            }
            pending.push((path, truncated, hash));
        }
        let total = pending.len();

        let mut embedded = 0usize;
        let mut failed = 0usize;
        for (i, (path, truncated, hash)) in pending.iter().enumerate() {
            match embedder.embed(truncated) {
                Ok(vector) => {
                    self.upsert_vector(path, &vector, embedder.model_id(), hash)?;
                    embedded += 1;
                }
                Err(_) => failed += 1,
            }
            progress(i + 1, total);
        }
        Ok((embedded, failed))
    }

    /// FTS 增量 + 内联补嵌（薄封装：`index_dirs` 后 `embed_pending` 无进度）。
    /// 保留供现有调用方 / 测试；桌面后台调度改走 `index_dirs` + `embed_pending`（见 desktop）。
    pub fn index_dirs_with_embedder(
        &self,
        roots: &[std::path::PathBuf],
        embedder: &dyn crate::embed::TextEmbedder,
    ) -> Result<(IndexStats, usize, usize), IndexError> {
        let stats = self.index_dirs(roots)?;
        let (embedded, failed) = self.embed_pending(roots, embedder, &mut |_, _| {})?;
        Ok((stats, embedded, failed))
    }
```

确认 `packages/indexer/Cargo.toml` 的 `[dev-dependencies]` 已含 `tempfile`（现有 scan.rs 测试已用）。

- [ ] **Step 4: 跑测试确认通过 + 现有测试不退**

Run: `cargo test -p locifind-indexer` 
Expected: PASS（新测试 + 现有 `index_dirs_with_embedder_*` 全绿）

- [ ] **Step 5: fmt + clippy + commit**

```bash
cargo fmt -p locifind-indexer
cargo clippy -p locifind-indexer --all-targets -- -D warnings
git add packages/indexer/src/doc_db.rs packages/indexer/src/scan.rs
git commit -m "BETA-15B-2 A：indexer 抽出 embed_pending 带进度回调"
```

---

## Task B: embedding 句柄 `prewarm()`

**Files:**
- Modify: `apps/desktop/src-tauri/src/search/embedding_model.rs`（在 `is_active` 后加方法 + test 模块加测试）

- [ ] **Step 1: 写失败测试**

在 `embedding_model.rs` 的 `#[cfg(test)] mod tests` 末尾追加：

```rust
    /// feature 关时 prewarm 返 false（不可用）、不 panic、幂等。
    #[test]
    fn prewarm_feature_off_is_false_and_idempotent() {
        if cfg!(feature = "semantic-recall") {
            return; // feature 开形态下跳过（需真模型，留真机手测）
        }
        let h = EmbeddingModelHandle::new(None, PathBuf::from("/tmp/x"));
        assert!(!h.prewarm());
        assert!(!h.prewarm()); // 二次调用仍 false，不 panic
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-desktop prewarm_feature_off -- --nocapture`
Expected: FAIL（`no method named prewarm`）

> 注：desktop crate 名以 `apps/desktop/src-tauri/Cargo.toml` 的 `[package] name` 为准；若非 `locifind-desktop`，用实际名（可 `grep '^name' apps/desktop/src-tauri/Cargo.toml`）。

- [ ] **Step 3: 实现 `prewarm`**

在 `embedding_model.rs` 的 `is_active`（149 行附近）之后插入：

```rust
    /// 后台暖机：在当前线程阻塞 load 模型（付掉冷启动成本），使后续查询直接走 warm 路径。
    /// 幂等——已 `Ready` 直接返 true；`Loading`/`Failed`/`Unavailable`/`NotFound` 返 false（不重试）。
    /// 应在后台 `spawn_blocking` 线程调，**绝不**在 UI / 查询线程调（会阻塞 16.8s 量级）。
    #[must_use]
    pub fn prewarm(&self) -> bool {
        self.ready().is_some()
    }
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p locifind-desktop prewarm_feature_off`
Expected: PASS

- [ ] **Step 5: fmt + commit**

```bash
cargo fmt -p locifind-desktop
git add apps/desktop/src-tauri/src/search/embedding_model.rs
git commit -m "BETA-15B-2 B：embedding 句柄加 prewarm 后台暖机"
```

---

## Task C: `IndexStatus` 语义字段 + 状态助手

**Files:**
- Modify: `apps/desktop/src-tauri/src/search/index_status.rs`（`IndexStatus` 结构 + 新增助手 + test）

- [ ] **Step 1: 写失败测试**

在 `index_status.rs` 末尾新增（或追加到现有）`#[cfg(test)]` 模块：

```rust
#[cfg(test)]
mod semantic_status_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn semantic_lifecycle_updates_status() {
        let status = Arc::new(Mutex::new(IndexStatus::default()));

        // begin：空闲 → true，置守卫 + 暖机摘要。
        assert!(semantic_begin(&status), "空闲时 begin 返 true");
        {
            let s = status.lock().unwrap();
            assert!(s.semantic_indexing);
            assert_eq!(s.semantic_summary.as_deref(), Some("暖机中…"));
        }
        // 已在跑 → begin 返 false（守卫）。
        assert!(!semantic_begin(&status), "已在跑时 begin 返 false");

        // progress：写 (done,total)。
        semantic_set_progress(&status, 3, 10);
        assert_eq!(status.lock().unwrap().semantic_progress, Some((3, 10)));

        // done：清守卫 + 进度，写就绪摘要。
        semantic_done(&status, 10);
        let s = status.lock().unwrap();
        assert!(!s.semantic_indexing);
        assert_eq!(s.semantic_progress, None);
        assert_eq!(s.semantic_summary.as_deref(), Some("语义索引就绪 10 篇"));
    }

    #[test]
    fn semantic_abort_clears_guard_with_reason() {
        let status = Arc::new(Mutex::new(IndexStatus::default()));
        assert!(semantic_begin(&status));
        semantic_abort(&status, "未找到 embedding 模型");
        let s = status.lock().unwrap();
        assert!(!s.semantic_indexing);
        assert_eq!(s.semantic_progress, None);
        assert_eq!(s.semantic_summary.as_deref(), Some("未找到 embedding 模型"));
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-desktop semantic_lifecycle -- --nocapture`
Expected: FAIL（`semantic_indexing` 字段不存在 / `semantic_begin` 未定义）

- [ ] **Step 3: 加字段 + 助手**

在 `index_status.rs` 的 `IndexStatus`（11-19 行）加三字段：

```rust
#[derive(Debug, Clone, Default, Serialize)]
pub struct IndexStatus {
    /// 是否正在索引（并发守卫）。
    pub indexing: bool,
    /// 上次完成索引的时间（rfc3339）。
    pub last_indexed: Option<String>,
    /// 上次结果摘要，如 `"音乐 947 / 文档 320 / 图片 58"`。
    pub last_summary: Option<String>,
    /// BETA-15B-2：语义嵌入 pass 进行中（独立于 `indexing` 的并发守卫）。
    pub semantic_indexing: bool,
    /// BETA-15B-2：语义嵌入进度 `(已嵌, 待嵌总数)`；非进行中为 `None`。
    pub semantic_progress: Option<(usize, usize)>,
    /// BETA-15B-2：语义索引摘要，如 `"语义索引就绪 320 篇"` / `"暖机中…"`。
    pub semantic_summary: Option<String>,
}
```

在结构体下方加助手（与 `perform_reindex` 同模块）：

```rust
/// 语义 pass 并发守卫：空闲 → 置 `semantic_indexing=true` + 暖机摘要、返 true；已在跑 → 返 false。
pub(crate) fn semantic_begin(status: &Arc<Mutex<IndexStatus>>) -> bool {
    let mut s = status.lock().unwrap_or_else(|e| e.into_inner());
    if s.semantic_indexing {
        return false;
    }
    s.semantic_indexing = true;
    s.semantic_progress = None;
    s.semantic_summary = Some("暖机中…".to_owned());
    true
}

/// 写语义嵌入进度 `(done, total)`。
pub(crate) fn semantic_set_progress(status: &Arc<Mutex<IndexStatus>>, done: usize, total: usize) {
    let mut s = status.lock().unwrap_or_else(|e| e.into_inner());
    s.semantic_progress = Some((done, total));
}

/// 语义 pass 完成：清守卫 + 进度，写就绪摘要。
pub(crate) fn semantic_done(status: &Arc<Mutex<IndexStatus>>, count: usize) {
    let mut s = status.lock().unwrap_or_else(|e| e.into_inner());
    s.semantic_indexing = false;
    s.semantic_progress = None;
    s.semantic_summary = Some(format!("语义索引就绪 {count} 篇"));
}

/// 语义 pass 中止（无模型 / 暖机失败）：清守卫 + 进度，摘要写原因。
pub(crate) fn semantic_abort(status: &Arc<Mutex<IndexStatus>>, reason: &str) {
    let mut s = status.lock().unwrap_or_else(|e| e.into_inner());
    s.semantic_indexing = false;
    s.semantic_progress = None;
    s.semantic_summary = Some(reason.to_owned());
}
```

需确认 `use std::sync::{Arc, Mutex};` 已在文件顶部（现有，3 行）。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p locifind-desktop semantic_lifecycle && cargo test -p locifind-desktop semantic_abort`
Expected: PASS

- [ ] **Step 5: commit**

```bash
cargo fmt -p locifind-desktop
git add apps/desktop/src-tauri/src/search/index_status.rs
git commit -m "BETA-15B-2 C：IndexStatus 加语义字段 + 状态助手"
```

---

## Task D: `perform_reindex` 去内联嵌入 + `semantic_index_pass` + `spawn_semantic_index`

**Files:**
- Modify: `apps/desktop/src-tauri/src/search/index_status.rs`（删 59-84 内联块；加 `semantic_index_pass` + `spawn_semantic_index` + 集成测试）

### D-1：`perform_reindex` 只做 FTS

- [ ] **Step 1: 写失败测试**

在 D-2 同一 test 模块内（先写本测试，验证解耦——`perform_reindex` 不再写向量）。这里用一个**总是失败的 embedder 不参与**：直接断言 `perform_reindex` 不接受 embedding 后向量表为空。

```rust
#[cfg(test)]
mod decouple_tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use std::sync::{Arc, Mutex};

    #[test]
    fn perform_reindex_does_fts_only_no_vectors() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("idx.db");
        let status = Arc::new(Mutex::new(IndexStatus::default()));

        // 只跑 FTS reindex（新签名不再收 embedding）。
        let out = perform_reindex(&status, db.clone());
        assert!(out.is_ok(), "FTS reindex 应成功");

        // 返回时 indexing 守卫已释放。
        assert!(!status.lock().unwrap().indexing, "perform_reindex 返回即释放 indexing");

        // 本次未填 document_vectors（向量由 worker 负责）。
        let idx = locifind_indexer::DocumentIndex::open(&db).unwrap();
        assert_eq!(idx.vector_count().unwrap(), 0, "perform_reindex 不应写向量");
    }
}
```

> 若 `DocumentIndex` 无 `vector_count()`，本任务在 indexer 加一个最小只读计数（`SELECT count(*) FROM document_vectors`），与 `count()` 同形态；并补一行单测。下方 Step 3 含该方法。

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-desktop perform_reindex_does_fts_only -- --nocapture`
Expected: FAIL（`perform_reindex` 仍是旧三参签名 / `vector_count` 不存在）

- [ ] **Step 3: 改 `perform_reindex` 签名 + 删内联块；indexer 加 `vector_count`**

`index_status.rs`：把 `perform_reindex` 改为不再收 `embedding` 参数、删 59-84 的内联嵌入块：

```rust
/// 执行一次 FTS reindex 并更新 [`IndexStatus`]（BETA-07）。**并发守卫**：已在索引中 → 返 `Ok(None)` 跳过。
/// BETA-15B-2：只做 FTS，语义向量嵌入解耦到 `spawn_semantic_index` 后台 worker。
/// 手动命令 + 后台启动共用。阻塞函数，应在 `spawn_blocking` 内调用。
pub(crate) fn perform_reindex(
    status: &Arc<Mutex<IndexStatus>>,
    db_path: std::path::PathBuf,
) -> Result<Option<ReindexStats>, String> {
    {
        let mut s = status.lock().unwrap_or_else(|e| e.into_inner());
        if s.indexing {
            return Ok(None);
        }
        s.indexing = true;
    }

    let result = {
        let backend = locifind_local_index_backend::LocalIndexBackend::new(&db_path);
        backend.reindex(
            &locifind_indexer::default_music_roots(),
            &locifind_indexer::default_document_roots(),
            &locifind_indexer::default_image_roots(),
        )
    };
    let totals = result
        .as_ref()
        .ok()
        .and_then(|_| compute_index_totals(&db_path));
    apply_reindex_result(status, result, totals)
}
```

`packages/indexer/src/doc_db.rs`：在 `DocumentIndex` impl 加只读计数（紧邻现有 `count`）：

```rust
    /// `document_vectors` 行数（BETA-15B-2 解耦验证 + 进度统计用）。
    pub fn vector_count(&self) -> Result<u64, IndexError> {
        let n: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM document_vectors", [], |r| r.get(0))?;
        Ok(u64::try_from(n).unwrap_or(0))
    }
```

> 已核实：`DocumentIndex` 连接字段为 `self.conn`，现有 `count()`（doc_db.rs:79）即此形态——照搬一致。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p locifind-desktop perform_reindex_does_fts_only && cargo test -p locifind-indexer vector_count`
Expected: PASS

> 编译会因 `perform_reindex` / `index_dirs_with_embedder` 旧调用点报错（main.rs、旧测试）——Task D-3 接入新调用点后消除；本步先确保新测试逻辑通过（可暂时 `cargo test -p locifind-indexer` 验 indexer 侧，desktop 侧编译在 D-3 修齐）。

- [ ] **Step 5: commit**

```bash
cargo fmt -p locifind-indexer
git add apps/desktop/src-tauri/src/search/index_status.rs packages/indexer/src/doc_db.rs
git commit -m "BETA-15B-2 D-1：perform_reindex 只做 FTS + indexer vector_count"
```

### D-2：`semantic_index_pass` + `spawn_semantic_index`

- [ ] **Step 1: 写失败测试**

在 `index_status.rs` 的 `decouple_tests` 模块加（核心同步逻辑可测，避开 async + 真模型）：

```rust
    /// 本地 stub embedder：确定性 2 维向量，隔离真模型（镜像 indexer scan.rs 的 StubEmbedder）。
    struct StubEmbedder;
    impl locifind_indexer::embed::TextEmbedder for StubEmbedder {
        fn embed(&self, text: &str) -> Result<Vec<f32>, locifind_indexer::IndexError> {
            let n = text.len() as f32;
            Ok(vec![n, n + 1.0])
        }
        fn model_id(&self) -> &str {
            "stub"
        }
    }

    #[test]
    fn semantic_index_pass_embeds_and_finalizes() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("idx.db");
        let docs = dir.path().join("docs");
        std::fs::create_dir_all(&docs).unwrap();
        std::fs::write(docs.join("a.txt"), "alpha body").unwrap();
        std::fs::write(docs.join("b.txt"), "beta body").unwrap();

        // 先建 FTS（模拟 perform_reindex 已跑完）。
        let status = Arc::new(Mutex::new(IndexStatus::default()));
        assert!(perform_reindex(&status, db.clone()).is_ok());

        // 语义 pass：prewarm=true（stub 不需真模型）+ stub embedder。
        let roots = vec![docs.clone()];
        semantic_index_pass(&status, &db, true, &StubEmbedder, &roots);

        let s = status.lock().unwrap();
        assert!(!s.semantic_indexing, "完成后清守卫");
        assert_eq!(s.semantic_progress, None);
        assert_eq!(s.semantic_summary.as_deref(), Some("语义索引就绪 2 篇"));
        drop(s);

        let idx = locifind_indexer::DocumentIndex::open(&db).unwrap();
        assert_eq!(idx.vector_count().unwrap(), 2, "两篇向量已写");
    }

    #[test]
    fn semantic_index_pass_aborts_when_not_prewarmed() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("idx.db");
        let status = Arc::new(Mutex::new(IndexStatus::default()));
        assert!(perform_reindex(&status, db.clone()).is_ok());

        // prewarm=false（无模型）→ 不嵌入、摘要写原因、清守卫。
        semantic_index_pass(&status, &db, false, &StubEmbedder, &[]);
        let s = status.lock().unwrap();
        assert!(!s.semantic_indexing);
        assert_eq!(s.semantic_summary.as_deref(), Some("未找到 embedding 模型"));
    }

    #[test]
    fn semantic_index_pass_skips_when_guard_held() {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("idx.db");
        let status = Arc::new(Mutex::new(IndexStatus::default()));
        // 预先占守卫 → pass 应直接跳过、不动状态。
        assert!(semantic_begin(&status));
        semantic_index_pass(&status, &db, true, &StubEmbedder, &[]);
        // 守卫仍 true（被本测试持有，pass 没动它）、摘要仍是 begin 写的暖机中。
        let s = status.lock().unwrap();
        assert!(s.semantic_indexing);
        assert_eq!(s.semantic_summary.as_deref(), Some("暖机中…"));
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-desktop semantic_index_pass -- --nocapture`
Expected: FAIL（`semantic_index_pass` 未定义）

- [ ] **Step 3: 实现 `semantic_index_pass` + `spawn_semantic_index`**

在 `index_status.rs` 加（`semantic_index_pass` 为可测同步核心，`spawn_semantic_index` 为生产 async 包装）：

```rust
use super::embedding_model::EmbeddingModelHandle;

/// 语义嵌入 pass 同步核心（可单测）。`prewarmed`=embedding 句柄 `prewarm()` 结果，
/// `embedder`=实现 `TextEmbedder` 的句柄，`roots`=文档根。
/// 守卫被占 → 跳过；`prewarmed=false` → 中止写原因；否则 `embed_pending` + 进度 + 就绪摘要。
pub(crate) fn semantic_index_pass(
    status: &Arc<Mutex<IndexStatus>>,
    db_path: &std::path::Path,
    prewarmed: bool,
    embedder: &dyn locifind_indexer::embed::TextEmbedder,
    roots: &[std::path::PathBuf],
) {
    if !semantic_begin(status) {
        return; // 已有语义 pass 在跑，跳过（不排队）。
    }
    if !prewarmed {
        semantic_abort(status, "未找到 embedding 模型");
        return;
    }
    let Ok(idx) = locifind_indexer::DocumentIndex::open(db_path) else {
        semantic_abort(status, "打开文档库失败，语义索引跳过");
        return;
    };
    let mut on_progress = |done, total| semantic_set_progress(status, done, total);
    match idx.embed_pending(roots, embedder, &mut on_progress) {
        Ok((embedded, failed)) => {
            if failed > 0 {
                eprintln!("语义索引：嵌入 {embedded} 篇，失败 {failed} 篇");
            }
            // 就绪摘要显示库内向量总数（增量轮 embedded 多为 0，显总数才不误导）。
            let total = idx
                .vector_count()
                .ok()
                .and_then(|n| usize::try_from(n).ok())
                .unwrap_or(embedded);
            semantic_done(status, total);
        }
        Err(e) => {
            eprintln!("文档向量嵌入失败（语义召回降级 FTS-only）: {e}");
            semantic_abort(status, "语义索引失败，已降级 FTS-only");
        }
    }
}

/// 生产入口：后台 worker 跑语义嵌入 pass。仅 `is_active()`（feature 开 + 模型就位）时实际工作。
/// 在 `spawn_blocking` 内 prewarm（付掉冷启动）→ `semantic_index_pass`。
pub(crate) fn spawn_semantic_index(
    status: Arc<Mutex<IndexStatus>>,
    db_path: std::path::PathBuf,
    embedding: Arc<EmbeddingModelHandle>,
) {
    if !embedding.is_active() {
        return; // feature 关 / 无模型 → 不 spawn，逐字节一致。
    }
    tauri::async_runtime::spawn_blocking(move || {
        let prewarmed = embedding.prewarm();
        let roots = locifind_indexer::default_document_roots();
        semantic_index_pass(&status, &db_path, prewarmed, embedding.as_ref(), &roots);
    });
}
```

> `semantic_index_pass_skips_when_guard_held` 期望守卫被占时**不写原因**——但上面实现 `if !semantic_begin {return}` 在守卫被占时直接 return、不动状态，符合（测试断言摘要仍是外部 begin 写的「暖机中…」）。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p locifind-desktop semantic_index_pass`
Expected: PASS（3 个子测试全绿）

- [ ] **Step 5: commit**

```bash
cargo fmt -p locifind-desktop
git add apps/desktop/src-tauri/src/search/index_status.rs
git commit -m "BETA-15B-2 D-2：semantic_index_pass + spawn_semantic_index 后台 worker"
```

### D-3：接入 main.rs 两处调用点

- [ ] **Step 1: 改 `reindex` 命令 + 启动后台任务**

`apps/desktop/src-tauri/src/main.rs`。

① `reindex` 命令（233-249）改为 FTS 后接语义 worker：

```rust
#[tauri::command]
async fn reindex(
    deps: tauri::State<'_, search::SearchDeps>,
) -> Result<search::ReindexStats, String> {
    let status = deps.index_status_arc();
    let embedding = deps.embedding().clone();
    let db = local_index_db_path();
    let fts_status = status.clone();
    let fts_db = db.clone();
    let out = tauri::async_runtime::spawn_blocking(move || {
        search::perform_reindex(&fts_status, fts_db)
    })
    .await;
    match out {
        Ok(Ok(Some(stats))) => {
            // FTS 完成 → 后台启语义 worker（暖机 + 解耦嵌入），不阻塞命令返回。
            search::spawn_semantic_index(status, db, embedding);
            Ok(stats)
        }
        Ok(Ok(None)) => Err("正在索引中，请稍候".to_owned()),
        Ok(Err(msg)) => Err(msg),
        Err(e) => Err(format!("reindex 任务失败: {e}")),
    }
}
```

② 启动后台任务（320-331）改为 FTS 后接语义 worker：

```rust
            tauri::async_runtime::spawn(async move {
                let db = local_index_db_path();
                let fts_db = db.clone();
                let fts_status = bg_status.clone();
                match tauri::async_runtime::spawn_blocking(move || {
                    search::perform_reindex(&fts_status, fts_db)
                })
                .await
                {
                    Ok(Ok(_)) => {
                        // FTS 完成 → 后台语义 worker（暖机消除首查询冷启动 + 渐进嵌入）。
                        search::spawn_semantic_index(bg_status, db, bg_embedding);
                    }
                    Ok(Err(msg)) => eprintln!("后台索引失败: {msg}"),
                    Err(e) => eprintln!("后台索引任务失败: {e}"),
                }
            });
```

> `spawn_semantic_index` 需在 `search` 模块 re-export（`index_status.rs` 经 `pub(crate) use index_status::*;` 已导出，确认 `spawn_semantic_index` 是 `pub(crate)`——是）。`bg_embedding` 已在 setup 克隆（310 行）。

- [ ] **Step 2: 编译 + 现有测试**

Run: `cargo test -p locifind-desktop`
Expected: PASS（main.rs 编译通过，旧 `perform_reindex` 三参调用已全部消除）

> 若有其他 `perform_reindex(.., Some(embedding))` 旧调用点（grep 确认），一并改为新两参签名 + 后接 `spawn_semantic_index`。

- [ ] **Step 3: commit**

```bash
cargo fmt -p locifind-desktop
git add apps/desktop/src-tauri/src/main.rs
git commit -m "BETA-15B-2 D-3：main.rs 启动任务 + reindex 命令接入语义 worker"
```

---

## Task E: 前端语义索引行

**Files:**
- Modify: `apps/desktop/src/pages/SettingsPage.tsx:24-28`（接口）+ `:258-264`（渲染）

- [ ] **Step 1: 扩接口 + 渲染**

`IndexStatus` 接口（25-29）加三字段：

```tsx
// BETA-07：与 search.rs::IndexStatus 对应。
interface IndexStatus {
  indexing: boolean;
  last_indexed: string | null;
  last_summary: string | null;
  // BETA-15B-2：语义索引状态（无模型 / 老构建时恒 false/null，不渲染）。
  semantic_indexing: boolean;
  semantic_progress: [number, number] | null;
  semantic_summary: string | null;
}
```

在现有索引状态 `<div>`（258-264）之后插入语义行：

```tsx
        {(indexStatus?.semantic_indexing || indexStatus?.semantic_summary) && (
          <div style={{ fontSize: '13px', color: indexStatus?.semantic_indexing ? '#007aff' : '#999', marginBottom: '10px' }}>
            {indexStatus?.semantic_indexing
              ? `🧠 语义索引中${indexStatus.semantic_progress ? ` ${indexStatus.semantic_progress[0]}/${indexStatus.semantic_progress[1]}` : '…'}`
              : `🧠 ${indexStatus?.semantic_summary ?? ''}`}
          </div>
        )}
```

- [ ] **Step 2: 前端构建验证**

Run: `cd apps/desktop && npm run build`（或仓库既有 `tsc && vite build` 脚本）
Expected: PASS（tsc 严格模式无错）

- [ ] **Step 3: commit**

```bash
git add apps/desktop/src/pages/SettingsPage.tsx
git commit -m "BETA-15B-2 E：设置页渲染语义索引进度行"
```

---

## Task F: 回归门 + 手测登记 + 收尾

**Files:**
- Modify: `docs/manual-test-scenarios.md`（加 15B-2 节）

- [ ] **Step 1: 全 workspace 回归门（默认形态）**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```
Expected: 全绿、零失败。

- [ ] **Step 2: `semantic-recall` feature 形态门**

```bash
cargo clippy -p locifind-desktop --features semantic-recall --all-targets -- -D warnings
cargo test -p locifind-desktop --features semantic-recall
```
Expected: 编译通过、测试零失败（真模型相关 test 自带 `cfg!(feature)` 跳过守卫）。

- [ ] **Step 3: evals byte-equal 硬门**

Run 仓库既有 evals 命令（参照 STATUS：v0.5=473 / v0.9=726 parser-only）。
Expected: **v0.5=473 / v0.9=726 不动**（本切片不碰 parser/expand，应天然守住）。
> 具体命令查 `packages/evals` README 或 `ci.sh`；若不确定，先 `grep -rn "473\|726" ci.sh packages/evals` 找运行入口。

- [ ] **Step 4: 登记真机手测场景**

在 `docs/manual-test-scenarios.md` 加「BETA-15B-2」节：

```markdown
## BETA-15B-2 向量索引后台预热 + 解耦调度

前提：feature `semantic-recall`（+ metal）构建，约定目录放 embedding 模型（qwen3-embedding-0.6b-q4_k_m.gguf）。

1. **冷启动消除**：放好模型 → 启动 app → 设置页等「🧠 语义索引就绪 N 篇」出现 → 执行跨语言 query（中文「年假和休假规定」命中纯英文 leave policy 文档）→ **首个查询不再 16.8s**（对比 15B-1 实测的冷加载）。
2. **解耦可搜**：删 index.db（或大量改动）触发全量 → FTS 结果**秒级可搜**的同时，设置页状态条显示「🧠 语义索引中 X/Y」渐进推进 → 期间执行普通文件名查询应正常返回（FTS 不被嵌入阻塞）。
3. **手动 reindex 不被挡**：语义索引中点「立即索引」→ FTS 照常完成（不报「正在索引中」），语义 worker 见守卫跳过本轮（已知限制：本轮新增文档延到下次触发补嵌）。
4. **无模型降级**：移走模型文件 → 启动 → 无语义行、搜索行为与今天一致（FTS-only）。
5. **双平台**：macOS + Windows 各验 1、2（Windows 是 16.8s 冷启动证据来源，必验暖机后首查询提速）。

实测记录（填入会话日志）：暖机耗时、FTS 可搜时延、语义 worker 全量耗时、常驻内存。
```

- [ ] **Step 5: commit**

```bash
git add docs/manual-test-scenarios.md
git commit -m "BETA-15B-2 F：回归门通过 + 登记真机手测场景"
```

---

## Self-Review 覆盖核对

- **spec §3.2 embed_pending** → Task A ✅
- **spec §3.3 prewarm** → Task B ✅
- **spec §3.5 IndexStatus 字段 + 助手** → Task C ✅
- **spec §3.4 perform_reindex 去内联 + spawn_semantic_index + 两处接入** → Task D（D-1/D-2/D-3）✅
- **spec §3.6 前端语义行** → Task E ✅
- **spec §6 测试（单元/集成/回归门/手测登记）** → 各 Task 内 TDD + Task F ✅
- **spec §5 错误处理（无模型 abort / 守卫跳过 / 单篇失败计数）** → Task D-2 测试覆盖三分支 ✅
- **类型一致性**：`embed_pending(roots, embedder, &mut progress)`、`prewarm()->bool`、`semantic_begin/set_progress/done/abort`、`semantic_index_pass(status,db,prewarmed,embedder,roots)`、`spawn_semantic_index(status,db,embedding)`、`vector_count()->u64` 全计划内一致 ✅
- **已知限制**（worker 在跑时新增文档延到下次触发）→ 手测场景 3 + 收工 STATUS 登记 ✅
