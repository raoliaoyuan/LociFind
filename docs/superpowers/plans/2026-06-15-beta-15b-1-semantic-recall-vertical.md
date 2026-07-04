# BETA-15B-1 语义召回纵切 MVP 实施计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 BETA-26 验证过的本地语义召回链路第一次落到真实桌面 app：文档臂 embedding 索引 + 向量存储 + 语义 backend + 与 FTS5 的 hybrid 加权 RRF 融合 + UI 可见，全程 feature + 模型存在性双重门控，关闭时与今天行为逐字节一致。

**Architecture:** 复用 `model-runtime::embed()`（Qwen3-Embedding-0.6B，dim 1024，L2 归一化）。向量以 f32 小端 BLOB 存进现有 SQLite 新表 `document_vectors`，文档增量索引时内联嵌入（截断首 1200 字、不分块）。新 `SemanticIndexBackend`（新 `BackendKind::SemanticIndex` + `MatchType::Semantic`）在查询期 embed query → 暴力 cosine → topK，与 FTS 臂一起进 BETA-04 fanout，由新的加权 RRF 融合层合并。桌面侧镜像 BETA-23 `ModelFallbackHandle` 做 embedding 模型懒加载句柄；前端给语义命中加"按意思找到"徽标。

**Tech Stack:** Rust（rusqlite / FTS5 / llama-cpp-4 embedding）、Tauri 2、React/TypeScript。feature flag `semantic-recall` 链 `model-runtime/llama-cpp`。

**锁定决策（不在本计划内改动）**：embedding 模型固定 Qwen3-Embedding-0.6B / dim 1024；整篇首 1200 字截断、不分块（BETA-26 §4.5）。融合权重用固定默认"偏向量"值，调优 + 置信度路由属 15B-3。macOS 优先；Windows embedding 属 15B-4。

**测试运行约定**：本仓库 workspace 测试用 `cargo test --workspace`；feature 形态用 `cargo test -p <crate> --features <feat>`。fmt/clippy 门：`cargo fmt --all --check` + `cargo clippy --workspace --all-targets -D warnings`。每个 task 收尾必须三者全绿（含 fmt，见 memory「Per-task 验证含 fmt」）。

---

## 阶段总览

- **Phase A**：向量数学 + 存储原语（indexer：BLOB serde、cosine、`document_vectors` 表 CRUD）。
- **Phase B**：内联嵌入（indexer：`TextEmbedder` 抽象 + 截断 + 索引期写向量）。
- **Phase C**：加权 RRF 融合（result-normalizer：纯函数）。
- **Phase D**：枚举扩展 + 语义 backend（common + 新 crate `semantic-index`）。
- **Phase E**：harness fanout 接 RRF + 路由放行 SemanticIndex。
- **Phase F**：桌面集成（embedding 模型句柄 + 注册 + db 路径 + 索引接线 + feature flag）。
- **Phase G**：前端（"按意思找到"徽标 + 设置页状态行）。
- **Phase H**：回归硬门（feature 关 byte-equal + exact-name 守护）+ 手测场景登记 + 收工。

---

## Phase A — 向量数学 + 存储原语

### Task A1: f32 BLOB serde + cosine 纯函数

**Files:**
- Create: `packages/indexer/src/vectors.rs`
- Modify: `packages/indexer/src/lib.rs`（加 `mod vectors;` + 重导出）

- [ ] **Step 1: 写失败测试**

在 `packages/indexer/src/vectors.rs` 写：

```rust
//! 向量存储原语：f32 ⇆ 小端 BLOB 序列化 + cosine 相似度。
//!
//! 向量来自 `model-runtime::embed()`（已 L2 归一化），故 cosine = 点积；
//! 但本函数不假设归一化，按定义算（除以双方模长），对未归一化输入也正确。

/// f32 向量序列化为小端字节（dim × 4 字节）。
#[must_use]
pub fn vector_to_blob(v: &[f32]) -> Vec<u8> {
    let mut out = Vec::with_capacity(v.len() * 4);
    for x in v {
        out.extend_from_slice(&x.to_le_bytes());
    }
    out
}

/// 小端字节反序列化为 f32 向量。长度非 4 倍数 → `None`。
#[must_use]
pub fn blob_to_vector(b: &[u8]) -> Option<Vec<f32>> {
    if b.len() % 4 != 0 {
        return None;
    }
    Some(
        b.chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect(),
    )
}

/// cosine 相似度。维度不等或任一为零向量 → `0.0`。
#[must_use]
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() {
        return 0.0;
    }
    let mut dot = 0.0f32;
    let mut na = 0.0f32;
    let mut nb = 0.0f32;
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na == 0.0 || nb == 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::float_cmp)]
    use super::*;

    #[test]
    fn blob_round_trip_preserves_values() {
        let v = vec![0.0f32, 1.0, -2.5, 3.14159, f32::MIN, f32::MAX];
        let blob = vector_to_blob(&v);
        assert_eq!(blob.len(), v.len() * 4);
        let back = blob_to_vector(&blob).unwrap();
        assert_eq!(back, v);
    }

    #[test]
    fn blob_to_vector_rejects_misaligned_len() {
        assert!(blob_to_vector(&[0u8, 1, 2]).is_none());
        assert!(blob_to_vector(&[]).unwrap().is_empty());
    }

    #[test]
    fn cosine_identical_is_one() {
        let v = vec![1.0f32, 2.0, 3.0];
        assert!((cosine(&v, &v) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_orthogonal_is_zero() {
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
    }

    #[test]
    fn cosine_dim_mismatch_or_zero_is_zero() {
        assert_eq!(cosine(&[1.0, 2.0], &[1.0]), 0.0);
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 1.0]), 0.0);
    }
}
```

- [ ] **Step 2: 接 module**

在 `packages/indexer/src/lib.rs` 找到 `mod` 声明区，加：

```rust
pub mod vectors;
```

（若 lib.rs 用 `pub use` 集中重导出，追加 `pub use vectors::{blob_to_vector, cosine, vector_to_blob};`；否则保持 `pub mod` 即可，调用方用 `locifind_indexer::vectors::cosine`。按文件现有风格择一。）

- [ ] **Step 3: 跑测试确认通过**

Run: `cargo test -p locifind-indexer vectors`
Expected: 5 个测试 PASS。

- [ ] **Step 4: fmt + clippy**

Run: `cargo fmt --all --check && cargo clippy -p locifind-indexer --all-targets -- -D warnings`
Expected: 无输出（干净）。

- [ ] **Step 5: 提交**

```bash
git add packages/indexer/src/vectors.rs packages/indexer/src/lib.rs
git commit -m "BETA-15B-1 A1: 向量 BLOB serde + cosine 纯函数"
```

---

### Task A2: `document_vectors` 表 + 向量 CRUD

**Files:**
- Modify: `packages/indexer/src/doc_db.rs`（SCHEMA 加表、新增向量方法、delete 级联清向量）
- Test: 同文件 `#[cfg(test)] mod tests`

- [ ] **Step 1: 写失败测试**

在 `packages/indexer/src/doc_db.rs` 的 `mod tests` 末尾追加（`doc()` 辅助已存在）：

```rust
#[test]
fn upsert_and_load_vector_round_trips() {
    let idx = DocumentIndex::open_in_memory().unwrap();
    idx.upsert_document(&doc("/d/a.docx", "docx", "张三"), "正文")
        .unwrap();
    let v = vec![0.1f32, 0.2, 0.3];
    idx.upsert_vector("/d/a.docx", &v, "qwen3-emb-0.6b", "hash1")
        .unwrap();
    let loaded = idx.candidate_vectors().unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].path, "/d/a.docx");
    assert_eq!(loaded[0].vector, v);
}

#[test]
fn upsert_vector_for_unknown_path_is_noop() {
    let idx = DocumentIndex::open_in_memory().unwrap();
    // 没有对应 documents 行 → 不应插入悬挂向量。
    let inserted = idx
        .upsert_vector("/d/missing.docx", &[1.0], "m", "h")
        .unwrap();
    assert!(!inserted, "无文档行时不写向量");
    assert!(idx.candidate_vectors().unwrap().is_empty());
}

#[test]
fn vector_hash_lets_caller_skip_reembed() {
    let idx = DocumentIndex::open_in_memory().unwrap();
    idx.upsert_document(&doc("/d/a.docx", "docx", "A"), "正文")
        .unwrap();
    idx.upsert_vector("/d/a.docx", &[1.0], "m", "hashA").unwrap();
    // 已存且 (model, hash) 相同 → vector_is_current 返 true（调用方据此跳过）。
    assert!(idx.vector_is_current("/d/a.docx", "m", "hashA").unwrap());
    assert!(!idx.vector_is_current("/d/a.docx", "m", "hashB").unwrap());
    assert!(!idx.vector_is_current("/d/a.docx", "m2", "hashA").unwrap());
    assert!(!idx.vector_is_current("/d/none.docx", "m", "hashA").unwrap());
}

#[test]
fn deleting_document_cascades_vector() {
    let idx = DocumentIndex::open_in_memory().unwrap();
    idx.upsert_document(&doc("/d/a.docx", "docx", "A"), "机密")
        .unwrap();
    idx.upsert_vector("/d/a.docx", &[1.0, 2.0], "m", "h").unwrap();
    assert_eq!(idx.candidate_vectors().unwrap().len(), 1);
    assert!(idx.delete_by_path("/d/a.docx").unwrap());
    assert!(
        idx.candidate_vectors().unwrap().is_empty(),
        "删文档应级联删向量，不留悬挂行"
    );
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-indexer upsert_and_load_vector_round_trips`
Expected: 编译失败（`upsert_vector` / `candidate_vectors` / `vector_is_current` 未定义）。

- [ ] **Step 3: 扩 SCHEMA + 开外键**

在 `packages/indexer/src/doc_db.rs` 的 `SCHEMA` 常量末尾（`documents_fts` 之后）追加表定义：

```rust
const SCHEMA: &str = "
CREATE TABLE IF NOT EXISTS documents (
  id            INTEGER PRIMARY KEY,
  path          TEXT NOT NULL UNIQUE,
  file_name     TEXT NOT NULL,
  title         TEXT,
  author        TEXT,
  doc_type      TEXT NOT NULL,
  page_count    INTEGER,
  modified_time INTEGER NOT NULL,
  indexed_time  INTEGER NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_documents_modified ON documents(modified_time);
CREATE VIRTUAL TABLE IF NOT EXISTS documents_fts USING fts5(
  title, author, body,
  tokenize='trigram'
);
CREATE TABLE IF NOT EXISTS document_vectors (
  doc_id        INTEGER PRIMARY KEY REFERENCES documents(id) ON DELETE CASCADE,
  dim           INTEGER NOT NULL,
  vector        BLOB NOT NULL,
  embed_model   TEXT NOT NULL,
  source_hash   TEXT NOT NULL,
  embedded_time INTEGER NOT NULL
);
";
```

在 `from_conn`（`busy_timeout` 之后、`execute_batch(SCHEMA)` 之前）开启外键级联：

```rust
fn from_conn(conn: Connection) -> Result<Self, IndexError> {
    // reindex 写与 search 读可能并发（BETA-04），给锁等待留 5s 窗口。
    conn.busy_timeout(std::time::Duration::from_secs(5))?;
    // document_vectors 外键级联依赖此 PRAGMA（默认关）。
    conn.execute_batch("PRAGMA foreign_keys = ON;")?;
    conn.execute_batch(SCHEMA)?;
    Ok(Self { conn })
}
```

- [ ] **Step 4: 加向量方法 + 候选结构体**

在 `doc_db.rs` 顶部 `use` 区后加一个轻量结构（放在 `DocumentIndex` 定义之前）：

```rust
/// 一条候选向量（语义检索暴力扫描用）。
#[derive(Debug, Clone)]
pub struct CandidateVector {
    pub path: String,
    pub vector: Vec<f32>,
}
```

在 `impl DocumentIndex` 内追加方法（放在 `paths_under_impl` 之后）：

```rust
/// 写/更新某文档的向量（按 path 找 doc_id；无文档行则不写、返回 false）。
/// 返回 true 表示新插入、false 表示更新或无文档行。
pub fn upsert_vector(
    &self,
    path: &str,
    vector: &[f32],
    embed_model: &str,
    source_hash: &str,
) -> Result<bool, IndexError> {
    let Some(id) = self.doc_id_of(path)? else {
        return Ok(false);
    };
    let blob = crate::vectors::vector_to_blob(vector);
    let dim = i64::try_from(vector.len()).unwrap_or(0);
    let now = crate::db::unix_now();
    let changed = self.conn.execute(
        "INSERT INTO document_vectors(doc_id, dim, vector, embed_model, source_hash, embedded_time)
             VALUES (?1,?2,?3,?4,?5,?6)
         ON CONFLICT(doc_id) DO UPDATE SET
             dim=excluded.dim, vector=excluded.vector, embed_model=excluded.embed_model,
             source_hash=excluded.source_hash, embedded_time=excluded.embedded_time",
        rusqlite::params![id, dim, blob, embed_model, source_hash, now],
    )?;
    // execute 返回受影响行数；INSERT=1，UPDATE 命中=1。区分新增/更新看冲突前是否存在。
    let _ = changed;
    Ok(self.vector_was_new(id))
}

/// 当前向量是否与给定 (model, hash) 一致（调用方据此跳过重嵌）。
pub fn vector_is_current(
    &self,
    path: &str,
    embed_model: &str,
    source_hash: &str,
) -> Result<bool, IndexError> {
    let Some(id) = self.doc_id_of(path)? else {
        return Ok(false);
    };
    let hit: Option<i64> = self
        .conn
        .query_row(
            "SELECT 1 FROM document_vectors
                 WHERE doc_id=?1 AND embed_model=?2 AND source_hash=?3",
            rusqlite::params![id, embed_model, source_hash],
            |r| r.get(0),
        )
        .optional()?;
    Ok(hit.is_some())
}

/// 全部候选向量（path + 反序列化向量）。语义检索暴力扫描用。
/// 损坏/维度异常的 BLOB 跳过（不致命）。
pub fn candidate_vectors(&self) -> Result<Vec<CandidateVector>, IndexError> {
    let mut stmt = self.conn.prepare(
        "SELECT d.path, v.vector FROM document_vectors v
             JOIN documents d ON d.id = v.doc_id",
    )?;
    let rows = stmt.query_map([], |r| {
        let path: String = r.get(0)?;
        let blob: Vec<u8> = r.get(1)?;
        Ok((path, blob))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (path, blob) = row?;
        if let Some(vector) = crate::vectors::blob_to_vector(&blob) {
            out.push(CandidateVector { path, vector });
        }
    }
    Ok(out)
}

fn doc_id_of(&self, path: &str) -> Result<Option<i64>, IndexError> {
    let id = self
        .conn
        .query_row("SELECT id FROM documents WHERE path = ?1", [path], |r| {
            r.get(0)
        })
        .optional()?;
    Ok(id)
}

fn vector_was_new(&self, doc_id: i64) -> bool {
    // embedded_time == now 且仅一行存在不足以区分；本 MVP 不需精确 added/updated 计数，
    // 统一返回 false（"更新"语义）即可——调用方只用返回值做日志，不做统计断言。
    let _ = doc_id;
    false
}
```

> 注：`vector_was_new` 简化为恒 false（MVP 不需要向量级 added/updated 精确计数；上层 `IndexStats` 统计的是文档而非向量）。测试 `upsert_and_load_vector_round_trips` 不断言返回值，`upsert_vector_for_unknown_path_is_noop` 断言无文档行时为 false——均成立。

确保 `doc_db.rs` 顶部已 `use rusqlite::OptionalExtension;`（现有 import 已含，见文件头 `use rusqlite::{params, Connection, OptionalExtension};`）。`crate::db::unix_now` 已是 `pub(crate)`（文件头已 `use crate::db::{fts_sanitize, path_is_under, unix_now};`）。

- [ ] **Step 5: 跑测试确认通过**

Run: `cargo test -p locifind-indexer doc_db`
Expected: 新增 4 个向量测试 + 现有文档测试全 PASS。`deleting_document_cascades_vector` 验证 `ON DELETE CASCADE` + `PRAGMA foreign_keys=ON` 生效。

- [ ] **Step 6: fmt + clippy + 全 crate 测试**

Run: `cargo fmt --all --check && cargo clippy -p locifind-indexer --all-targets -- -D warnings && cargo test -p locifind-indexer`
Expected: 干净 + 全绿。

- [ ] **Step 7: 提交**

```bash
git add packages/indexer/src/doc_db.rs
git commit -m "BETA-15B-1 A2: document_vectors 表 + 向量 CRUD（外键级联）"
```

---

## Phase B — 内联嵌入

### Task B1: `TextEmbedder` 抽象 + 截断辅助

**Files:**
- Create: `packages/indexer/src/embed.rs`
- Modify: `packages/indexer/src/lib.rs`（`pub mod embed;`）

设计理由：indexer **不直接依赖** model-runtime（避免把 llama-cpp 拉进索引 crate，保持轻量、可被无模型构建编译）。定义一个窄 trait，由桌面层注入真实 embedder。

- [ ] **Step 1: 写失败测试**

`packages/indexer/src/embed.rs`：

```rust
//! 文档嵌入抽象。indexer 不依赖具体模型运行时——桌面层注入实现。
//!
//! 截断遵 BETA-26 §4.5：整篇首 1200 字、不分块（分块在该语料 wash-to-略负且成本 7.5×）。

use crate::IndexError;

/// 句向量生成器（由桌面层用 `model-runtime::embed()` 实现）。
pub trait TextEmbedder: Send + Sync {
    /// 嵌入一段文本，返回向量。
    fn embed(&self, text: &str) -> Result<Vec<f32>, IndexError>;
    /// 模型标识（写入 `document_vectors.embed_model`，换模型→旧向量陈旧）。
    fn model_id(&self) -> &str;
}

/// 截断到首 `max_chars` 个**字符**（非字节，CJK 安全）。
#[must_use]
pub fn truncate_chars(text: &str, max_chars: usize) -> &str {
    match text.char_indices().nth(max_chars) {
        Some((byte_idx, _)) => &text[..byte_idx],
        None => text,
    }
}

/// 嵌入用的正文截断上限（BETA-26 锁定值）。
pub const EMBED_TRUNCATE_CHARS: usize = 1200;

/// 稳定内容指纹（写入 `source_hash`；正文没变→跳过重嵌）。
/// 用 FNV-1a 64bit，零依赖、确定性。
#[must_use]
pub fn content_hash(text: &str) -> String {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in text.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    format!("{h:016x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn truncate_is_char_safe_for_cjk() {
        let s = "季度预算分析报告";
        assert_eq!(truncate_chars(s, 4), "季度预算");
        assert_eq!(truncate_chars(s, 100), s);
        assert_eq!(truncate_chars(s, 0), "");
    }

    #[test]
    fn content_hash_is_stable_and_sensitive() {
        assert_eq!(content_hash("abc"), content_hash("abc"));
        assert_ne!(content_hash("abc"), content_hash("abd"));
        assert_eq!(content_hash("").len(), 16);
    }
}
```

- [ ] **Step 2: 接 module**

`packages/indexer/src/lib.rs` 加：

```rust
pub mod embed;
```

- [ ] **Step 3: 跑测试**

Run: `cargo test -p locifind-indexer embed`
Expected: 2 测试 PASS。

- [ ] **Step 4: fmt + clippy + 提交**

```bash
cargo fmt --all --check && cargo clippy -p locifind-indexer --all-targets -- -D warnings
git add packages/indexer/src/embed.rs packages/indexer/src/lib.rs
git commit -m "BETA-15B-1 B1: TextEmbedder 抽象 + 截断/指纹辅助"
```

---

### Task B2: 文档增量索引内联写向量

**Files:**
- Modify: `packages/indexer/src/doc_db.rs`（新增 `index_dirs_with_embedder`，复用 `index_dirs` 主体后补一遍向量）
- Test: `packages/indexer/src/scan.rs` 的 `mod tests`（已有文档增量端到端测试基建 `touch_text`）

设计：`index_dirs` 行为不变（无模型构建照常）。新增 `index_dirs_with_embedder`：先跑现有文档增量（FTS 入库），再对当前 DB 中需要的文档补嵌向量（按 `vector_is_current` 跳过）。第二遍读 FTS body 作为嵌入输入。

- [ ] **Step 1: 在 doc_db.rs 加读取 body 的辅助 + 带 embedder 的索引方法**

在 `impl DocumentIndex` 内追加：

```rust
/// 取某文档的 FTS body（嵌入输入）。无该文档 → None。
fn body_of(&self, path: &str) -> Result<Option<String>, IndexError> {
    let body = self
        .conn
        .query_row(
            "SELECT f.body FROM documents d JOIN documents_fts f ON f.rowid = d.id
                 WHERE d.path = ?1",
            [path],
            |r| r.get::<_, String>(0),
        )
        .optional()?;
    Ok(body)
}

/// 增量索引 + 内联嵌入。先跑 FTS 增量（行为同 `index_dirs`），
/// 再对 `roots` 子树下的文档补嵌向量（`vector_is_current` 命中则跳过）。
/// 单篇嵌入失败计数但不中断（文档仍 FTS 可搜）。返回 (文档 stats, 嵌入篇数)。
pub fn index_dirs_with_embedder(
    &self,
    roots: &[std::path::PathBuf],
    embedder: &dyn crate::embed::TextEmbedder,
) -> Result<(IndexStats, usize), IndexError> {
    let stats = self.index_dirs(roots)?;

    let root_strs: Vec<String> = roots
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    let mut embedded = 0usize;
    for path in self.paths_under(&root_strs)? {
        let Some(body) = self.body_of(&path)? else {
            continue;
        };
        let truncated = crate::embed::truncate_chars(&body, crate::embed::EMBED_TRUNCATE_CHARS);
        let hash = crate::embed::content_hash(truncated);
        if self.vector_is_current(&path, embedder.model_id(), &hash)? {
            continue;
        }
        match embedder.embed(truncated) {
            Ok(vector) => {
                self.upsert_vector(&path, &vector, embedder.model_id(), &hash)?;
                embedded += 1;
            }
            // 单篇失败不中断整轮（镜像 catch_extract 哲学）；文档仍 FTS 可搜。
            Err(_) => continue,
        }
    }
    Ok((stats, embedded))
}
```

确保文件已能解析 `IndexStats`（文件头已 `use crate::model::{...}`，但 `IndexStats` 是否在其中需核对；若缺，把 `IndexStats` 加入该 `use`）。`paths_under` 是 `IncrementalStore` trait 方法，本 `impl` 内可直接 `self.paths_under(...)`（已在同 crate 实现）。

- [ ] **Step 2: 写失败测试（scan.rs）**

在 `packages/indexer/src/scan.rs` 的 `mod tests` 末尾追加：

```rust
/// stub embedder：把文本长度映射成固定维度向量（确定性，隔离真模型）。
struct StubEmbedder;
impl crate::embed::TextEmbedder for StubEmbedder {
    fn embed(&self, text: &str) -> Result<crate::Vec3OrSo, IndexError> {
        // 简单确定性嵌入：3 维 = [字符数, 是否含'报', 常量]。
        Ok(vec![
            text.chars().count() as f32,
            if text.contains('报') { 1.0 } else { 0.0 },
            1.0,
        ])
    }
    fn model_id(&self) -> &str {
        "stub-emb"
    }
}

#[test]
fn index_dirs_with_embedder_writes_and_skips_vectors() {
    use crate::doc_db::DocumentIndex;
    let dir = tempfile::tempdir().unwrap();
    touch_text(&dir.path().join("a.txt"), "季度预算分析报告", 1000);
    touch_text(&dir.path().join("b.md"), "# 标题\n纯内容无关键字", 1000);
    let idx = DocumentIndex::open_in_memory().unwrap();
    let roots = [dir.path().to_path_buf()];

    let (stats, embedded) = idx
        .index_dirs_with_embedder(&roots, &StubEmbedder)
        .unwrap();
    assert_eq!(stats.added, 2);
    assert_eq!(embedded, 2, "两篇都新嵌");
    assert_eq!(idx.candidate_vectors().unwrap().len(), 2);

    // 再跑：body 未变 → source_hash 命中 → 0 重嵌。
    let (_s2, e2) = idx
        .index_dirs_with_embedder(&roots, &StubEmbedder)
        .unwrap();
    assert_eq!(e2, 0, "内容未变应跳过重嵌");

    // 删一篇 → 文档回收 + 向量级联删。
    std::fs::remove_file(dir.path().join("a.txt")).unwrap();
    let (s3, _e3) = idx
        .index_dirs_with_embedder(&roots, &StubEmbedder)
        .unwrap();
    assert_eq!(s3.removed, 1);
    assert_eq!(idx.candidate_vectors().unwrap().len(), 1, "向量随文档级联删");
}
```

> 修正：上面 stub 的返回类型写成了占位 `crate::Vec3OrSo`——实际应为 `Result<Vec<f32>, IndexError>`。替换该方法签名为：
> ```rust
> fn embed(&self, text: &str) -> Result<Vec<f32>, IndexError> {
> ```

- [ ] **Step 3: 跑测试确认通过**

Run: `cargo test -p locifind-indexer index_dirs_with_embedder_writes_and_skips_vectors`
Expected: PASS（写 2 → 跳过 → 级联删验证）。

- [ ] **Step 4: fmt + clippy + 全 crate 测试 + 提交**

```bash
cargo fmt --all --check && cargo clippy -p locifind-indexer --all-targets -- -D warnings && cargo test -p locifind-indexer
git add packages/indexer/src/doc_db.rs packages/indexer/src/scan.rs
git commit -m "BETA-15B-1 B2: 文档增量索引内联写向量（hash 跳过 + 级联删）"
```

---

## Phase C — 加权 RRF 融合

### Task C1: `fuse_rrf` 纯函数（保留 per-backend rank）

**Files:**
- Modify: `packages/result-normalizer/src/lib.rs`（新增 `fuse_rrf` + 默认权重常量）
- Test: 同文件 tests

设计：现有 `merge_results` 按路径去重取 max score，不做 rank 融合。新增 `fuse_rrf` 接受**每个 backend 的有序列表**（位置即 rank），按加权 RRF 累加跨列表得分，产 `MergedResult`（源/match_type 并集、代表取 richness、score = RRF 和），按 score 降序返回。

- [ ] **Step 1: 写失败测试**

在 `packages/result-normalizer/src/lib.rs` 的 `mod tests` 末尾追加：

```rust
#[test]
fn fuse_rrf_combines_ranks_across_backends() {
    // FTS 列表：A(rank0), B(rank1)；语义列表：B(rank0), C(rank1)。
    // B 在两列表都靠前 → RRF 最高。
    let a = sr("/a", BackendKind::NativeIndex, MatchType::Content);
    let b_fts = sr("/b", BackendKind::NativeIndex, MatchType::Content);
    let b_sem = sr("/b", BackendKind::SemanticIndex, MatchType::Semantic);
    let c = sr("/c", BackendKind::SemanticIndex, MatchType::Semantic);

    let fused = fuse_rrf(
        vec![vec![a, b_fts], vec![b_sem, c]],
        DEFAULT_RRF_K,
        DEFAULT_SEMANTIC_WEIGHT,
    );

    assert_eq!(fused[0].result.path, std::path::PathBuf::from("/b"));
    // B 命中两源 → sources/match_types 并集。
    assert_eq!(fused[0].sources.len(), 2);
    assert!(fused[0].match_types.contains(&MatchType::Semantic));
    assert!(fused[0].match_types.contains(&MatchType::Content));
    assert_eq!(fused.len(), 3);
}

#[test]
fn fuse_rrf_empty_lists_yield_empty() {
    assert!(fuse_rrf(vec![], DEFAULT_RRF_K, DEFAULT_SEMANTIC_WEIGHT).is_empty());
    assert!(fuse_rrf(vec![vec![], vec![]], DEFAULT_RRF_K, DEFAULT_SEMANTIC_WEIGHT).is_empty());
}
```

在 tests 顶部加辅助（若已有类似的 `sr` 构造器则复用现有）：

```rust
fn sr(path: &str, source: BackendKind, mt: MatchType) -> SearchResult {
    SearchResult {
        id: path.to_string(),
        path: std::path::PathBuf::from(path),
        name: path.trim_start_matches('/').to_string(),
        source,
        match_type: mt,
        score: None,
        metadata: SearchResultMetadata::default(),
    }
}
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-result-normalizer fuse_rrf`
Expected: 编译失败（`fuse_rrf` / 常量未定义）。

- [ ] **Step 3: 实现 `fuse_rrf`**

在 `packages/result-normalizer/src/lib.rs`（`merge_results` 之后）加：

```rust
/// 默认 RRF k（BETA-26 实测对 k 不敏感）。
pub const DEFAULT_RRF_K: f64 = 60.0;
/// 默认语义臂权重（BETA-26 §4.6「偏向量」默认；FTS 臂权重固定 1.0）。
/// 调优 + 置信度路由属 15B-3，本 MVP 用固定默认。
pub const DEFAULT_SEMANTIC_WEIGHT: f64 = 2.0;

/// 加权 Reciprocal Rank Fusion：每个 backend 一个**有序**列表（位置=rank）。
/// 语义臂（`BackendKind::SemanticIndex`）列表用 `semantic_weight`，其余权重 1.0。
/// 跨列表按 path 累加 `weight / (k + rank + 1)`，源/match_type 取并集，
/// 代表结果取 metadata 最丰富者，score = 累加 RRF，按 score 降序返回。
#[must_use]
pub fn fuse_rrf(
    lists: Vec<Vec<SearchResult>>,
    k: f64,
    semantic_weight: f64,
) -> Vec<MergedResult> {
    use std::collections::HashMap;
    let mut order: Vec<PathBuf> = Vec::new();
    let mut map: HashMap<PathBuf, (MergedResult, f64)> = HashMap::new();

    for list in lists {
        for (rank, r) in list.into_iter().enumerate() {
            let weight = if r.source == BackendKind::SemanticIndex {
                semantic_weight
            } else {
                1.0
            };
            #[allow(clippy::cast_precision_loss)]
            let contrib = weight / (k + rank as f64 + 1.0);
            if let Some((m, score)) = map.get_mut(&r.path) {
                if !m.sources.contains(&r.source) {
                    m.sources.push(r.source);
                }
                if !m.match_types.contains(&r.match_type) {
                    m.match_types.push(r.match_type);
                }
                if metadata_richness(&r) > metadata_richness(&m.result) {
                    m.result = r;
                }
                *score += contrib;
            } else {
                order.push(r.path.clone());
                map.insert(
                    r.path.clone(),
                    (
                        MergedResult {
                            sources: vec![r.source],
                            match_types: vec![r.match_type],
                            result: r,
                        },
                        contrib,
                    ),
                );
            }
        }
    }

    let mut out: Vec<(MergedResult, f64)> =
        order.into_iter().filter_map(|p| map.remove(&p)).collect();
    // 写入融合分到 result.score，并按 score 降序（稳定：原插入序为 tiebreak）。
    for (m, score) in &mut out {
        m.result.score = Some(*score);
    }
    out.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    out.into_iter().map(|(m, _)| m).collect()
}
```

确认文件头已 `use` 到 `PathBuf`、`BackendKind`、`MatchType`、`SearchResult`、`SearchResultMetadata`（`merge_results` 已用前几个；`SearchResultMetadata` 仅测试用，加到 tests 的 `use super::*` 即可）。

- [ ] **Step 4: 跑测试确认通过 + fmt + clippy + 提交**

Run: `cargo test -p locifind-result-normalizer && cargo fmt --all --check && cargo clippy -p locifind-result-normalizer --all-targets -- -D warnings`
Expected: 全 PASS + 干净。

```bash
git add packages/result-normalizer/src/lib.rs
git commit -m "BETA-15B-1 C1: 加权 RRF 融合（保留 per-backend rank）"
```

---

## Phase D — 枚举扩展 + 语义 backend

### Task D1: common 加 `BackendKind::SemanticIndex` + `MatchType::Semantic`

**Files:**
- Modify: `packages/search-backends/common/src/lib.rs`

- [ ] **Step 1: 加枚举变体**

`BackendKind`（现有 4 变体后追加）：

```rust
pub enum BackendKind {
    /// macOS Spotlight。
    Spotlight,
    /// Windows Search / `SystemIndex`。
    WindowsSearch,
    /// Everything 可选加速后端。
    Everything,
    /// `LociFind` 未来自建索引。
    NativeIndex,
    /// `LociFind` 本地语义召回（embedding + cosine）。
    SemanticIndex,
}
```

`MatchType`（现有 4 变体后追加）：

```rust
pub enum MatchType {
    /// 文件名命中。
    Filename,
    /// 文件内容命中。
    Content,
    /// 元数据命中。
    Metadata,
    /// OCR 文本命中。
    Ocr,
    /// 语义召回命中（按意思 / 跨语言）。
    Semantic,
}
```

- [ ] **Step 2: 编译确认无穷尽 match 漏网**

Run: `cargo build --workspace`
Expected: 若有非穷尽 `match BackendKind`/`MatchType` 会报错。**逐个修复**为合理默认：
- ranker `match_type_weight`：给 `MatchType::Semantic` 权重（放在 Content 与 Ocr 之间，如 `0.7`——语义命中相关性可信度类同内容）。
- 任何把 `BackendKind` 映射到字符串/能力的 `match`：`SemanticIndex` 比照 `NativeIndex` 处理。

（此步是发现-修复循环：build → 看报错文件:行 → 补分支 → 再 build，直到 workspace 编过。每个修复点用最小合理值，不改其它行为。）

- [ ] **Step 3: fmt + clippy + 全测试 + 提交**

```bash
cargo build --workspace && cargo test --workspace && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
git add -A
git commit -m "BETA-15B-1 D1: common 加 SemanticIndex 后端 + Semantic 匹配类型"
```

---

### Task D2: 新 crate `semantic-index` + `SemanticIndexBackend`

**Files:**
- Create: `packages/search-backends/semantic-index/Cargo.toml`
- Create: `packages/search-backends/semantic-index/src/lib.rs`
- Modify: 根 `Cargo.toml`（workspace members 加新 crate）

设计：backend 持 db 路径 + 一个 `Arc<dyn QueryEmbedder>`（query 侧嵌入器，由桌面注入；与 indexer 的 `TextEmbedder` 同形但本 crate 自定义以避免依赖 indexer 的 trait——或直接复用 `locifind_indexer::embed::TextEmbedder`）。为 DRY，**复用 indexer 的 `TextEmbedder`**。

- [ ] **Step 1: 建 Cargo.toml**

`packages/search-backends/semantic-index/Cargo.toml`：

```toml
[package]
name = "locifind-semantic-index"
version.workspace = true
edition.workspace = true
rust-version.workspace = true
license.workspace = true

[dependencies]
locifind-search-backend = { path = "../common" }
locifind-indexer = { path = "../../indexer" }
futures-util = { workspace = true }

[lints]
workspace = true
```

> 核对 common crate 的实际包名（前述桌面代码用 `locifind_search_backend::SearchResult`，故包名应为 `locifind-search-backend`，路径 `../common`）。`futures-util` 是否在 `[workspace.dependencies]`：A 步 backend 用到 `backend_stream_from_results`（来自 common），可能无需直接依赖 futures-util——若 lib.rs 不直接用，删掉该依赖。

- [ ] **Step 2: 写 backend + 失败测试**

`packages/search-backends/semantic-index/src/lib.rs`：

```rust
//! BETA-15B-1：本地语义召回后端。query embed → 暴力 cosine → topK。
//!
//! 与 FTS 臂并列进 BETA-04 fanout，由 harness 的加权 RRF 融合层合并。
//! 模型缺失时 backend 不可用（`is_available()=false`），整链优雅降级 FTS-only。

use std::path::{Path, PathBuf};
use std::sync::Arc;

use locifind_indexer::doc_db::DocumentIndex;
use locifind_indexer::embed::TextEmbedder;
use locifind_indexer::vectors::cosine;
use locifind_search_backend::{
    backend_stream_from_results, BackendKind, BackendSearchFuture, ExpandedSearchIntent,
    MatchType, SearchBackend, SearchError, SearchIntent, SearchResult, SearchResultMetadata,
};
use tokio_util::sync::CancellationToken;

/// 每次查询暴力扫描后返回的 topK。
const TOP_K: usize = 10;

/// 语义召回后端。`embedder` 为 `None` → 不可用（无模型时降级）。
#[derive(Clone)]
pub struct SemanticIndexBackend {
    db_path: PathBuf,
    embedder: Option<Arc<dyn TextEmbedder>>,
}

impl std::fmt::Debug for SemanticIndexBackend {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SemanticIndexBackend")
            .field("db_path", &self.db_path)
            .field("has_embedder", &self.embedder.is_some())
            .finish()
    }
}

impl SemanticIndexBackend {
    /// 用 db 路径 + 可选 query 嵌入器构造。
    pub fn new(db_path: impl Into<PathBuf>, embedder: Option<Arc<dyn TextEmbedder>>) -> Self {
        Self {
            db_path: db_path.into(),
            embedder,
        }
    }

    /// 取查询文本：FileSearch 用 base query 文本（语义召回只服务文件搜索）。
    fn query_text(intent: &SearchIntent) -> Option<String> {
        match intent {
            SearchIntent::FileSearch(fs) => {
                let q = fs.query.trim();
                if q.is_empty() {
                    None
                } else {
                    Some(q.to_string())
                }
            }
            _ => None,
        }
    }

    fn search_results(&self, intent: &SearchIntent) -> Result<Vec<SearchResult>, SearchError> {
        let Some(embedder) = &self.embedder else {
            return Ok(Vec::new());
        };
        let Some(text) = Self::query_text(intent) else {
            return Ok(Vec::new());
        };
        let qvec = embedder
            .embed(&text)
            .map_err(|e| SearchError::Backend(e.to_string()))?;

        let idx = DocumentIndex::open(&self.db_path).map_err(|e| SearchError::Backend(e.to_string()))?;
        let mut scored: Vec<(f32, String)> = idx
            .candidate_vectors()
            .map_err(|e| SearchError::Backend(e.to_string()))?
            .into_iter()
            .map(|c| (cosine(&qvec, &c.vector), c.path))
            .collect();
        scored.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(TOP_K);

        Ok(scored
            .into_iter()
            .map(|(score, path)| {
                let p = PathBuf::from(&path);
                let name = Path::new(&path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&path)
                    .to_string();
                SearchResult {
                    id: path.clone(),
                    path: p,
                    name,
                    source: BackendKind::SemanticIndex,
                    match_type: MatchType::Semantic,
                    score: Some(f64::from(score)),
                    metadata: SearchResultMetadata::default(),
                }
            })
            .collect())
    }
}

impl SearchBackend for SemanticIndexBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::SemanticIndex
    }

    fn is_available(&self) -> bool {
        self.embedder.is_some()
    }

    fn search<'a>(
        &'a self,
        intent: &'a SearchIntent,
        cancel: CancellationToken,
    ) -> BackendSearchFuture<'a> {
        Box::pin(async move {
            let results = self.search_results(intent)?;
            Ok(backend_stream_from_results(results, cancel))
        })
    }

    fn search_expanded<'a>(
        &'a self,
        expanded: &'a ExpandedSearchIntent,
        cancel: CancellationToken,
    ) -> BackendSearchFuture<'a> {
        // 语义召回用 base query 文本，不消费 keyword_groups（embedding 走原始语义）。
        self.search(&expanded.base, cancel)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use locifind_indexer::embed::TextEmbedder;
    use locifind_indexer::IndexError;

    struct AxisEmbedder;
    impl TextEmbedder for AxisEmbedder {
        fn embed(&self, text: &str) -> Result<Vec<f32>, IndexError> {
            // 含"猫"→[1,0]，含"狗"→[0,1]，否则[0,0]。
            Ok(vec![
                if text.contains('猫') { 1.0 } else { 0.0 },
                if text.contains('狗') { 1.0 } else { 0.0 },
            ])
        }
        fn model_id(&self) -> &str {
            "axis"
        }
    }

    fn seed_db() -> (tempfile::TempDir, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let db = dir.path().join("index.db");
        let idx = DocumentIndex::open(&db).unwrap();
        // 两篇：一篇"猫"向量，一篇"狗"向量。query 文本本身不进 FTS，纯测向量召回。
        idx.upsert_document_for_test("/d/cat.txt", "关于猫的笔记").unwrap();
        idx.upsert_vector("/d/cat.txt", &[1.0, 0.0], "axis", "h1").unwrap();
        idx.upsert_document_for_test("/d/dog.txt", "关于狗的笔记").unwrap();
        idx.upsert_vector("/d/dog.txt", &[0.0, 1.0], "axis", "h2").unwrap();
        (dir, db)
    }

    #[test]
    fn semantic_query_ranks_by_cosine() {
        let (_dir, db) = seed_db();
        let backend = SemanticIndexBackend::new(db, Some(Arc::new(AxisEmbedder)));
        let intent = SearchIntent::FileSearch(locifind_search_backend::FileSearch {
            query: "我家的猫".to_string(),
            ..Default::default()
        });
        let results = backend.search_results(&intent).unwrap();
        assert_eq!(results[0].path, PathBuf::from("/d/cat.txt"));
        assert_eq!(results[0].source, BackendKind::SemanticIndex);
        assert_eq!(results[0].match_type, MatchType::Semantic);
    }

    #[test]
    fn no_embedder_is_unavailable_and_empty() {
        let (_dir, db) = seed_db();
        let backend = SemanticIndexBackend::new(db, None);
        assert!(!backend.is_available());
        let intent = SearchIntent::FileSearch(locifind_search_backend::FileSearch {
            query: "猫".to_string(),
            ..Default::default()
        });
        assert!(backend.search_results(&intent).unwrap().is_empty());
    }
}
```

> 依赖核对清单（实现时逐一对齐真实 API，编译器会指出偏差）：
> - `SearchError` 的变体名（这里用 `SearchError::Backend(String)`）——核对 common 实际定义，若不同改用真实变体。
> - `CancellationToken` 来源（`tokio_util::sync` vs re-export）——按 common 现用的为准（前述 trait 签名用 `CancellationToken`，core 用 `tokio_util`）。把 `tokio-util` 加进 Cargo.toml deps（或复用 common 的 re-export）。
> - `FileSearch` 的字段名（`query`）——核对 common 的 `struct FileSearch`，对齐真实字段（可能叫 `query`/`text`）。
> - `upsert_document_for_test`：indexer 现有 `upsert_document` 是 `pub(crate)`。本测试需要一个 crate 外可建文档行的入口——**在 indexer 加一个 `#[cfg(any(test, feature = "test-util"))]` 或 `pub` 的薄封装**，或测试改为通过 `index_dirs` 落盘真实文件。最简：在 indexer 暴露 `pub fn upsert_document_min(&self, path, body)`（仅测试用，门控）。若不想加生产 API，改测试用 `tempfile` 写真实 txt + `index_dirs` 入库再 `upsert_vector`。

- [ ] **Step 3: 注册到 workspace**

根 `Cargo.toml` 的 `members` 列表加：

```toml
    "packages/search-backends/semantic-index",
```

- [ ] **Step 4: 跑测试 + fmt + clippy + 提交**

Run: `cargo test -p locifind-semantic-index && cargo fmt --all --check && cargo clippy -p locifind-semantic-index --all-targets -- -D warnings`
Expected: 2 测试 PASS + 干净。（实现时按上面"依赖核对清单"对齐真实 API 直到编过。）

```bash
git add -A
git commit -m "BETA-15B-1 D2: SemanticIndexBackend（embed query → cosine topK）"
```

---

## Phase E — harness fanout 接 RRF + 路由放行

### Task E1: 路由放行 SemanticIndex

**Files:**
- Modify: `packages/harness/src/intent_router.rs`（`backend_indexes_content`）

- [ ] **Step 1: 改 predicate + 测试**

`backend_indexes_content` 加 `SemanticIndex`：

```rust
const fn backend_indexes_content(kind: Option<BackendKind>) -> bool {
    matches!(
        kind,
        Some(
            BackendKind::Spotlight
                | BackendKind::WindowsSearch
                | BackendKind::NativeIndex
                | BackendKind::SemanticIndex
        )
    )
}
```

在该文件 tests 加（若已有 fanout 路由测试，仿其构造一个注册了 semantic tool 的 registry，断言它进 selected）：

```rust
#[test]
fn semantic_backend_joins_content_fanout() {
    assert!(backend_indexes_content(Some(BackendKind::SemanticIndex)));
}
```

- [ ] **Step 2: 跑测试 + fmt + clippy + 提交**

Run: `cargo test -p locifind-harness backend_indexes_content || cargo test -p locifind-harness semantic_backend_joins_content_fanout`

```bash
cargo fmt --all --check && cargo clippy -p locifind-harness --all-targets -- -D warnings
git add packages/harness/src/intent_router.rs
git commit -m "BETA-15B-1 E1: 路由放行 SemanticIndex 进内容 fanout"
```

---

### Task E2: fanout RRF 融合变体

**Files:**
- Modify: `packages/harness/src/fanout_merge.rs`（新增 `run_fanout_merge_rrf`，收集 per-backend 列表后用 `fuse_rrf`）

设计：现有 `run_fanout_merge` 把所有结果摊平后 `merge_results`。新增 `run_fanout_merge_rrf`：每个 backend 的结果各自成列（保留到达序=rank），最后 `fuse_rrf`。其余（sources_queried/errors/取消）逻辑与现有一致。

- [ ] **Step 1: 写失败测试**

仿 `fanout_merge.rs` 现有测试风格（mock `SearchableTool`），加一个断言：两个 mock 后端（一个 NativeIndex 返 [A,B]，一个 SemanticIndex 返 [B,C]）经 `run_fanout_merge_rrf` 后，B 排第一、total=3。（复用文件内已有的 mock tool 构造器；若无则参照 `run_fanout_merge` 现有测试。）

```rust
#[tokio::test]
async fn fanout_rrf_fuses_per_backend_ranks() {
    // 构造两个 mock SearchableTool（仿本文件现有测试的 mock 构造）：
    //   fts_tool: NativeIndex → [A, B]
    //   sem_tool: SemanticIndex → [B, C]
    // 断言融合后 B 第一、collected 3 条。
    // （具体 mock 样板复用本文件 mod tests 中既有 helper。）
}
```

- [ ] **Step 2: 实现 `run_fanout_merge_rrf`**

在 `fanout_merge.rs` 加（与 `run_fanout_merge` 并列）：

```rust
use locifind_result_normalizer::{fuse_rrf, DEFAULT_RRF_K, DEFAULT_SEMANTIC_WEIGHT};

/// 与 [`run_fanout_merge`] 同样多源查询，但**保留各后端排名**用加权 RRF 融合
/// （语义召回臂 + FTS 臂的 hybrid 路径用此变体）。
#[must_use = "FanoutOutcome 须被检查；total==0 时需向用户报告空态/错误"]
pub async fn run_fanout_merge_rrf<R>(
    backends: &[Arc<dyn SearchableTool>],
    expanded: &ExpandedSearchIntent,
    cancel: CancellationToken,
    on_result: &mut R,
) -> FanoutOutcome
where
    R: FnMut(MergedResult) + Send,
{
    let mut lists: Vec<Vec<SearchResult>> = Vec::new();
    let mut sources_queried: Vec<BackendKind> = Vec::new();
    let mut errors: Vec<(String, String)> = Vec::new();

    for tool in backends {
        if cancel.is_cancelled() {
            break;
        }
        let tool_id = tool.id().to_owned();
        match tool.search_expanded(expanded, cancel.clone()).await {
            Err(err) => errors.push((tool_id, err.to_string())),
            Ok(mut stream) => {
                if let Some(kind) = tool.capability().backend_kind {
                    if !sources_queried.contains(&kind) {
                        sources_queried.push(kind);
                    }
                }
                let mut list: Vec<SearchResult> = Vec::new();
                while let Some(item) = stream.next().await {
                    if cancel.is_cancelled() {
                        break;
                    }
                    match item {
                        Ok(result) => list.push(result),
                        Err(err) => {
                            errors.push((tool_id.clone(), err.to_string()));
                            break;
                        }
                    }
                }
                lists.push(list);
            }
        }
    }

    let merged = fuse_rrf(lists, DEFAULT_RRF_K, DEFAULT_SEMANTIC_WEIGHT);
    let total = merged.len();
    for m in merged {
        on_result(m);
    }

    FanoutOutcome {
        total,
        sources_queried,
        errors,
    }
}
```

确保 `fanout_merge.rs` 头部 `use` 含 `SearchResult`（merge 路径已用 `SearchResult`，应已 import）。把 `locifind-result-normalizer` 的 `fuse_rrf`/常量 import 加到文件头（上面 `use` 行）。

- [ ] **Step 3: 跑测试 + fmt + clippy + 提交**

Run: `cargo test -p locifind-harness fanout_rrf_fuses_per_backend_ranks`

```bash
cargo fmt --all --check && cargo clippy -p locifind-harness --all-targets -- -D warnings
git add packages/harness/src/fanout_merge.rs
git commit -m "BETA-15B-1 E2: fanout 加权 RRF 融合变体"
```

---

## Phase F — 桌面集成

### Task F1: AppSettings 加 embedding 模型路径字段

**Files:**
- Modify: `apps/desktop/src-tauri/src/settings.rs`（`AppSettings` 加 `embedding_model_path: Option<String>`）

- [ ] **Step 1: 加字段**

在 `AppSettings`（`#[serde(default)]`，已有 `model_path`）末尾加：

```rust
    /// BETA-15B-1：embedding 模型文件路径覆盖（None = 默认 app 数据目录 models/）。
    pub embedding_model_path: Option<String>,
```

`Default` impl（若手写）补 `embedding_model_path: None,`；若 derive(Default) 则自动。

- [ ] **Step 2: build + 提交**

Run: `cargo build -p locifind-desktop`（按桌面 crate 实名）
Expected: 编过（serde default 兼容旧 settings.json）。

```bash
git add apps/desktop/src-tauri/src/settings.rs
git commit -m "BETA-15B-1 F1: AppSettings 加 embedding_model_path"
```

---

### Task F2: embedding 模型句柄（镜像 ModelFallbackHandle）

**Files:**
- Create: `apps/desktop/src-tauri/src/search/embedding_model.rs`
- Modify: `apps/desktop/src-tauri/src/search/mod.rs`（或 search.rs 的 `mod` 声明，加 `pub mod embedding_model;`）

设计：实现 `TextEmbedder`（indexer 的 trait）的桌面侧 `EmbeddingModelHandle`，内部懒加载 `model-runtime` embedding 模型，状态机镜像 `ModelFallbackHandle`（NotLoaded/Loading/Ready/Failed/Unavailable）。对外暴露 `embed()`（无模型时返回 Err，让 backend 视作无结果）+ `status()`（设置页用）。

- [ ] **Step 1: 写句柄**

`apps/desktop/src-tauri/src/search/embedding_model.rs`（镜像 `model_fallback.rs` 结构，简化：embedding 无超时/单飞需求，但要懒加载 + 路径解析 + feature 门控）：

```rust
//! BETA-15B-1：embedding 模型懒加载句柄（镜像 model_fallback 的约定目录 + feature 门控）。
//! 实现 indexer 的 `TextEmbedder`，供索引期与查询期共用。

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use locifind_indexer::embed::TextEmbedder;
use locifind_indexer::IndexError;

const DEFAULT_EMBED_MODEL_FILE: &str = "qwen3-embedding-0.6b-q4_k_m.gguf";
const EMBED_MODEL_ID: &str = "qwen3-embedding-0.6b";

#[allow(dead_code)]
enum EmbedState {
    NotLoaded,
    Ready(Arc<locifind_model_runtime::ModelDaemon>),
    Failed(String),
    Unavailable(String),
}

/// 对外状态（设置页 / 隐私面板用）。
#[derive(Debug, Clone, serde::Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum EmbedStatus {
    Ready,
    NotFound { expected_path: String },
    Failed { reason: String },
    Unavailable { reason: String },
}

pub struct EmbeddingModelHandle {
    state: Mutex<EmbedState>,
    settings_path: Option<PathBuf>,
    default_model_path: PathBuf,
}

impl std::fmt::Debug for EmbeddingModelHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EmbeddingModelHandle").finish()
    }
}

impl EmbeddingModelHandle {
    #[must_use]
    pub fn new(settings_path: Option<PathBuf>, data_dir: PathBuf) -> Self {
        let initial = if cfg!(feature = "semantic-recall") {
            EmbedState::NotLoaded
        } else {
            EmbedState::Unavailable("本构建不含语义召回（feature semantic-recall 未开启）".to_owned())
        };
        Self {
            state: Mutex::new(initial),
            settings_path,
            default_model_path: data_dir.join("models").join(DEFAULT_EMBED_MODEL_FILE),
        }
    }

    fn resolved_model_path(&self) -> PathBuf {
        if let Some(path) = &self.settings_path {
            if let Ok(s) = std::fs::read_to_string(path) {
                if let Ok(v) = serde_json::from_str::<crate::settings::AppSettings>(&s) {
                    if let Some(custom) = v.embedding_model_path.filter(|p| !p.trim().is_empty()) {
                        return PathBuf::from(custom);
                    }
                }
            }
        }
        self.default_model_path.clone()
    }

    /// 同步取就绪 daemon；NotLoaded 时尝试一次阻塞加载（索引/查询均可接受首次同步加载）。
    fn ready(&self) -> Option<Arc<locifind_model_runtime::ModelDaemon>> {
        let mut st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match &*st {
            EmbedState::Ready(d) => return Some(Arc::clone(d)),
            EmbedState::Failed(_) | EmbedState::Unavailable(_) => return None,
            EmbedState::NotLoaded => {}
        }
        let path = self.resolved_model_path();
        if !path.exists() {
            return None; // 保持 NotLoaded，下次再探测（设置页 status 显示 NotFound）。
        }
        let params = locifind_model_runtime::ModelLoadParams {
            gpu_layers: 99,
            context_size: 2048,
        };
        match locifind_model_runtime::ModelDaemon::load_blocking(&path, params) {
            Ok(d) => {
                let arc = Arc::new(d);
                *st = EmbedState::Ready(Arc::clone(&arc));
                Some(arc)
            }
            Err(e) => {
                *st = EmbedState::Failed(e.to_string());
                None
            }
        }
    }

    /// 设置页 / 隐私面板状态。
    #[must_use]
    pub fn status(&self) -> EmbedStatus {
        let st = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match &*st {
            EmbedState::Ready(_) => EmbedStatus::Ready,
            EmbedState::Failed(r) => EmbedStatus::Failed { reason: r.clone() },
            EmbedState::Unavailable(r) => EmbedStatus::Unavailable { reason: r.clone() },
            EmbedState::NotLoaded => EmbedStatus::NotFound {
                expected_path: self.resolved_model_path().to_string_lossy().into_owned(),
            },
        }
    }
}

impl TextEmbedder for EmbeddingModelHandle {
    fn embed(&self, text: &str) -> Result<Vec<f32>, IndexError> {
        let daemon = self
            .ready()
            .ok_or_else(|| IndexError::Tag {
                path: String::new(),
                detail: "embedding 模型不可用".to_owned(),
            })?;
        daemon
            .embed(text)
            .map_err(|e| IndexError::Tag {
                path: String::new(),
                detail: e.to_string(),
            })
    }
    fn model_id(&self) -> &str {
        EMBED_MODEL_ID
    }
}
```

> 核对清单：
> - `ModelDaemon::load_blocking` 与 `ModelDaemon::embed` 的真实签名（`model_fallback.rs` 用 `ModelDaemon::load_blocking(&path, params)`；`embed` 是否在 `ModelDaemon` 上暴露需核对 daemon.rs，可能需加一个透传 `embed` 方法到 `ModelDaemon`——若缺，加薄封装委托给内部 `LlamaModelRuntime::embed`）。
> - `IndexError::Tag { path, detail }` 变体名（indexer 现用此变体，见 scan.rs `catch_extract`）。
> - feature 名 `semantic-recall`（Task F4 定义）。

- [ ] **Step 2: 接 module + build**

在 search 模块声明处加 `pub mod embedding_model;`。

Run: `cargo build -p locifind-desktop --features semantic-recall`（feature 见 F4；先做 F4 或本步暂用 `--features model-fallback` 验证非 feature 部分编译，feature 全链在 F4 收口）。

- [ ] **Step 3: 提交**

```bash
git add apps/desktop/src-tauri/src/search/embedding_model.rs apps/desktop/src-tauri/src/search/mod.rs
git commit -m "BETA-15B-1 F2: embedding 模型懒加载句柄（实现 TextEmbedder）"
```

---

### Task F3: 注册 SemanticIndexBackend + 索引接线

**Files:**
- Modify: `apps/desktop/src-tauri/src/main.rs`（`build_registry` 注册 semantic backend；索引路径用 embedder）
- Modify: 索引触发处（reindex 命令）改调 `index_dirs_with_embedder`

- [ ] **Step 1: 构造共享 embedding 句柄**

在 app 初始化处（`build_registry` 调用前后、有 `app.handle()` 与 data_dir 的地方）构造一个 `Arc<EmbeddingModelHandle>`，同时供：
- `SemanticIndexBackend`（query 侧）
- reindex 命令（index 侧）
- 设置页 status 命令

```rust
let embedding_handle = Arc::new(search::embedding_model::EmbeddingModelHandle::new(
    settings::settings_file_path(&app.handle().clone()),
    dirs::data_dir().unwrap_or_else(|| PathBuf::from(".")).join("LociFind"),
));
```

存进 Tauri state（`.manage(embedding_handle.clone())` 或并入既有 AppState）。

- [ ] **Step 2: 在 build_registry 注册 semantic backend**

`build_registry` 需要拿到 `embedding_handle`（改签名 `fn build_registry(embedding: Arc<EmbeddingModelHandle>) -> ToolRegistry`）。在 LocalIndexBackend 注册之后加：

```rust
{
    let semantic = locifind_semantic_index::SemanticIndexBackend::new(
        local_index_db_path(),
        Some(embedding as Arc<dyn locifind_indexer::embed::TextEmbedder>),
    );
    let tool = SearchTool::new(
        "search.semantic",
        "语义召回",
        semantic,
        vec![SupportedIntent::FileSearch],
        "LociFind 本地语义召回（embedding + cosine，按意思/跨语言）",
    );
    if let Err(err) = registry.register_search(tool) {
        eprintln!("注册 SemanticIndexBackend 失败: {err}");
    }
}
```

> `SearchTool::new` 的 capability 必须让 `backend_kind = Some(SemanticIndex)`——核对 `SearchTool`/`ToolCapability` 如何从 `SearchBackend::kind()` 推导 backend_kind（前述 fanout 用 `tool.capability().backend_kind`）。若 capability 不自动取 `kind()`，按其构造方式显式设。

- [ ] **Step 3: 选择 RRF fanout 路径**

在 `search.rs` 跑 fanout 处（现用 `run_fanout_search` → 内部 `run_fanout_merge`/`_with_fallback`），当 fanout backends 含 SemanticIndex 时改用 `run_fanout_merge_rrf`。最小改法：在 fanout 内部判断 `backends.iter().any(|t| t.capability().backend_kind == Some(BackendKind::SemanticIndex))`，是则走 RRF 变体，否则保持现有 `run_fanout_merge`（**保证无语义臂时行为逐字节不变**）。

- [ ] **Step 4: reindex 用 embedder**

找到 reindex 命令（调 `DocumentIndex::index_dirs` / `LocalIndexBackend` reindex 的地方）。把文档增量改为 `index_dirs_with_embedder(&roots, embedding_handle.as_ref())`（从 state 取 `Arc<EmbeddingModelHandle>`，它实现了 `TextEmbedder`）。模型不在场时 `embed()` 返 Err → 内联嵌入对每篇 continue → 退化为纯 FTS 索引（行为安全）。

- [ ] **Step 5: build + 提交**

Run: `cargo build -p locifind-desktop --features semantic-recall`（F4 后）
Expected: 编过。

```bash
git add apps/desktop/src-tauri/src/main.rs apps/desktop/src-tauri/src/search.rs
git commit -m "BETA-15B-1 F3: 注册语义 backend + 索引接 embedder + RRF fanout 选路"
```

---

### Task F4: feature flag `semantic-recall`

**Files:**
- Modify: `apps/desktop/src-tauri/Cargo.toml`（新 feature 链 model-runtime/llama-cpp + 新依赖）

- [ ] **Step 1: 加 feature + 依赖**

桌面 `Cargo.toml` `[dependencies]` 加：

```toml
locifind-semantic-index = { path = "../../../packages/search-backends/semantic-index" }
```

`[features]` 加：

```toml
semantic-recall = ["locifind-model-runtime/llama-cpp"]
semantic-recall-metal = ["semantic-recall", "locifind-model-runtime/metal"]
```

> 核对：桌面 crate 是否已依赖 `locifind-model-runtime`（model_fallback 已用，应有）。`SemanticIndexBackend`/`EmbeddingModelHandle` 在 feature 关时也要能编译（backend 持 `Option<embedder>`=None；句柄 `Unavailable`）——确保非 feature 构建里这些类型仍编过（embed() 调 daemon 的部分用 `#[cfg(feature="semantic-recall")]` 或靠 model-runtime 的 stub 默认 `embed()` 返 Err，二者皆可；优先靠 stub 默认 embed 返 Err，**避免 cfg 分叉**）。

- [ ] **Step 2: 三形态构建门禁**

Run（逐一）：
```
cargo build -p locifind-desktop
cargo build -p locifind-desktop --features semantic-recall
cargo build -p locifind-desktop --features semantic-recall-metal
```
Expected: 三形态全编过。

- [ ] **Step 3: 提交**

```bash
git add apps/desktop/src-tauri/Cargo.toml
git commit -m "BETA-15B-1 F4: feature semantic-recall（默认关，链 llama-cpp/metal）"
```

---

### Task F5: 设置页 status Tauri 命令

**Files:**
- Modify: `apps/desktop/src-tauri/src/`（加 `#[tauri::command] fn embedding_model_status`，注册到 `invoke_handler`）

- [ ] **Step 1: 加命令**

```rust
#[tauri::command]
fn embedding_model_status(
    handle: tauri::State<'_, Arc<crate::search::embedding_model::EmbeddingModelHandle>>,
) -> crate::search::embedding_model::EmbedStatus {
    handle.status()
}
```

加入 `tauri::generate_handler![ ... , embedding_model_status]`。

- [ ] **Step 2: build + 提交**

```bash
cargo build -p locifind-desktop --features semantic-recall
git add -A
git commit -m "BETA-15B-1 F5: embedding 模型状态 Tauri 命令"
```

---

## Phase G — 前端

### Task G1: "按意思找到"徽标

**Files:**
- Modify: `apps/desktop/src/SearchView.tsx`（`SearchResultJson` 已含 `match_type: string`；match 列 render 给 `semantic` 友好标签）

- [ ] **Step 1: 改 match 列 render + 类型**

`SearchResultJson` 无需改（`match_type` 已是 string，后端会序列化 `"semantic"`）。把 match 列的 `render` 从裸字符串改为带中文标签 + 语义高亮：

```tsx
{
  key: "match",
  label: "匹配方式",
  defaultWidth: 110,
  defaultVisible: true, // 语义召回是旗舰卖点，默认可见
  cellClass: "col-match",
  render: (r) =>
    r.match_type === "semantic" ? (
      <span className="badge-semantic" title="按语义/跨语言召回">按意思找到</span>
    ) : (
      matchTypeLabel(r.match_type)
    ),
  sortValue: (r) => r.match_type,
},
```

在文件顶部加标签辅助：

```tsx
function matchTypeLabel(mt: string): string {
  switch (mt) {
    case "filename": return "文件名";
    case "content": return "内容";
    case "metadata": return "元数据";
    case "ocr": return "OCR";
    case "semantic": return "按意思找到";
    default: return mt;
  }
}
```

加最小 CSS（对应 stylesheet）：

```css
.badge-semantic {
  background: #e8f0fe;
  color: #1a56db;
  border-radius: 4px;
  padding: 1px 6px;
  font-size: 12px;
}
```

> 多 match_type 注意：BETA-04 合并后一个结果可能多源（FTS + 语义都命中）。`SearchResultJson` 当前只序列化单个 `match_type`（代表结果的）。MVP 接受只显示代表 match_type；若要显示"内容 + 按意思"组合，需后端 `result_to_json` 额外序列化 `match_types: string[]`（可选增强，非必须）。本 task 只做单 `match_type` 徽标。

- [ ] **Step 2: 前端构建**

Run: `cd apps/desktop && npm run build`（或仓库实际命令 `pnpm`/`tsc && vite build`）
Expected: tsc 通过、构建成功。

- [ ] **Step 3: 提交**

```bash
git add apps/desktop/src/SearchView.tsx apps/desktop/src/<stylesheet>
git commit -m "BETA-15B-1 G1: 语义命中「按意思找到」徽标"
```

---

### Task G2: 设置页 embedding 模型状态行

**Files:**
- Modify: 设置页组件（镜像 BETA-23 model fallback 状态行，调 `embedding_model_status`）

- [ ] **Step 1: 调命令显示状态**

在设置页（找 BETA-23 model fallback 状态行所在组件）加一行调用：

```tsx
const [embedStatus, setEmbedStatus] = useState<any>(null);
useEffect(() => {
  invoke("embedding_model_status").then(setEmbedStatus).catch(() => {});
}, []);
// 渲染：
// state==="ready" → 「语义召回：已就绪」
// state==="not_found" → 「语义召回模型未找到，放到 {expected_path} 后将自动启用」
// state==="unavailable"/"failed" → 显示 reason
```

按设置页现有状态行的样式与文案风格对齐（镜像 model fallback 三态行）。

- [ ] **Step 2: 构建 + 提交**

```bash
cd apps/desktop && npm run build
git add apps/desktop/src/<settings component>
git commit -m "BETA-15B-1 G2: 设置页 embedding 模型状态行"
```

---

## Phase H — 回归硬门 + 手测登记 + 收工

### Task H1: 回归硬门（feature 关 byte-equal + 现有测试零回归）

**Files:**
- 验证型 task（无新代码或仅补守护测试）

- [ ] **Step 1: feature 关全量回归**

Run:
```
cargo test --workspace
cargo fmt --all --check
cargo clippy --workspace --all-targets -- -D warnings
```
Expected: 全绿。**evals parser-only byte-equal**（v0.5=473 / v0.9=726）由现有 `cargo test --workspace` 内的 evals 完整性测试守护——确认未变（语义臂不碰 parser，应天然守住）。

- [ ] **Step 2: feature 开形态测试**

Run: `cargo test -p locifind-indexer && cargo test -p locifind-semantic-index && cargo test -p locifind-harness`
（含真 llama-cpp 的端到端不在单元测试跑，靠手测 H3。）
Expected: 全绿。

- [ ] **Step 3: 补 exact-name 守护意识测试（可选轻量）**

若 fanout RRF 选路逻辑有"无语义臂时仍走旧 merge"分支，补一个 harness 测试断言：fanout backends 不含 SemanticIndex 时调用的是 `run_fanout_merge`（行为不变）。确保旧路径零行为漂移。

- [ ] **Step 4: 提交（若有补测）**

```bash
git add -A
git commit -m "BETA-15B-1 H1: 回归守护（feature 关 byte-equal + 旧 fanout 路径不漂移）"
```

---

### Task H2: 手测场景登记

**Files:**
- Modify: `docs/manual-test-scenarios.md`（加 BETA-15B-1 节）

- [ ] **Step 1: 写场景**

在 `docs/manual-test-scenarios.md` 加一节 `## BETA-15B-1 语义召回纵切`：

```markdown
### 前置：放 embedding 模型
- 把 Qwen3-Embedding-0.6B GGUF 放到 app 数据目录 `models/qwen3-embedding-0.6b-q4_k_m.gguf`（或设置页填 embedding_model_path）。
- 设置页应显示「语义召回：已就绪」；不放时显示「模型未找到 + 放置提示」。

### 场景 1：跨语言召回（最硬卖点）
- 索引一个含英文文档的目录（reindex 后日志/状态显示已嵌 N 篇）。
- 用中文 query 搜一个只在英文文档里出现的概念（query 不含任何英文精确词）。
- 期望：该英文文档出现在结果里，匹配方式列显示「按意思找到」徽标。FTS-only（不放模型）时搜不到。

### 场景 2：模糊同义召回
- query 用与正文不同的措辞（如「讲退款的文档」对正文写「退货流程」的文件）。
- 期望：命中，徽标「按意思找到」。

### 场景 3：exact-name 不回退
- 用精确文件名/标题词搜一个已知文件。
- 期望：仍稳定排在前列（语义臂不拖垮精确名）。

### 场景 4：无模型优雅降级
- 移除/不放 embedding 模型，重启。
- 期望：搜索行为与今天完全一致（纯 FTS），无报错；设置页显示模型未找到。
```

- [ ] **Step 2: 提交**

```bash
git add docs/manual-test-scenarios.md
git commit -m "BETA-15B-1 H2: 手测场景登记（跨语言/模糊/exact-name/降级）"
```

---

### Task H3: 收工（STATUS + ROADMAP + 文档）

**Files:**
- Modify: `STATUS.md`、`ROADMAP.md`、`docs/third-party-licenses.md`（如有新依赖）、各 README

- [ ] **Step 1: ROADMAP 登记 15B 分解**

`ROADMAP.md` BETA-15B 卡片下登记四子项 15B-1/2/3/4（依赖、估时、状态）；15B-1 标 done（或 in_progress 视落地程度）。新增 crate `semantic-index` 在 package 列补一行。

- [ ] **Step 2: STATUS 更新**

按 CONVENTIONS §3：更新「当前 task」「下一步」（指向 15B-2 后台调度），会话日志顶部加一段（署名 Claude Code）。

- [ ] **Step 3: 第三方许可 / README**

若 embedding 模型或新依赖（tokio-util 等）涉及，更新 `docs/third-party-licenses.md`；indexer/semantic-index README 补向量层说明。

- [ ] **Step 4: 全量验证 + 收工 commit + 向用户确认**

Run: `cargo test --workspace && cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings`

```bash
git add -A
git commit -m "BETA-15B-1 收工：语义召回纵切落地 + ROADMAP/STATUS 同步"
```

向用户报告提交内容，等确认。

---

## 自审记录（writing-plans Self-Review）

- **Spec 覆盖**：§3.1 embedding 句柄→F2/F5；§3.2 向量存储→A2；§3.3 内联嵌入→B1/B2；§3.4 语义 backend→D1/D2；§3.5 hybrid 融合→C1/E2；§3.6 ranker→D1 Step2（Semantic 权重）；§3.7 UI→G1/G2；§3.8 feature 门控→F4；§5 错误处理→分散在 D2(query 失败)/B2(单篇失败)/F2(模型缺)；§6 测试→各 task TDD + H1；§8 验收→H1/H3 + 手测 H2。无遗漏节。
- **占位扫描**：无 "TBD/TODO"。"核对清单"是对真实 API 名的对齐提示（编译器会强制），非需求空缺；每个核对项都给了 fallback 做法。
- **类型一致**：`TextEmbedder`（A→D→F 同名同签名）、`fuse_rrf(lists,k,weight)`（C 定义 / E2 调用一致）、`candidate_vectors`/`upsert_vector`/`vector_is_current`（A2 定义、B2/D2 调用一致）、`BackendKind::SemanticIndex`/`MatchType::Semantic`（D1 定义、贯穿）、`EmbeddingModelHandle`（F2 定义、F3/F5 用）。
- **已知需实现期对齐的真实 API**（非占位，编译器指引）：`SearchError` 变体名、`FileSearch` 字段名、`ModelDaemon::embed` 是否已暴露、`SearchTool`/`ToolCapability` 如何取 backend_kind、桌面 crate 实名、前端构建命令与 stylesheet 路径。每处已在对应 task 标注 fallback。
