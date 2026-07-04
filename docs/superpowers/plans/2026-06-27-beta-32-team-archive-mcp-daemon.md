# BETA-32 Implementation Plan：团队归档 MCP daemon

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 LociFind 已有的 hybrid（FTS + embedding）检索能力包装成 headless MCP daemon，使在职同事通过本机 Claude Code / Codex 用自然语言查询离职同事归档目录内的文件，仅返路径 + 元数据。

**Architecture:** 新增 `packages/locifind-server`（lib，含 Tool trait + Tool 实现 + axum HTTP layer + auth + admin endpoints）+ `apps/daemon`（binary `locifindd`，CLI + lifecycle + fail-fast）。复用现有 `packages/harness::fallback_chain` 作为检索 orchestrator、复用 `packages/indexer` 作为索引器、复用 `packages/search-backends/{local-index,semantic-index}` 作为 backend 实现。仅 `packages/indexer` 加一个 `IndexProgress` callback trait 用于 daemon tracing log（桌面 app 不动行为）。

**Tech Stack:** Rust 1.80+ / tokio / axum 0.7 / rmcp (Anthropic 官方 Rust MCP SDK) / clap 4.x / secrecy + subtle / serde + serde_json / tracing + tracing-subscriber / httptest（dev-dep、集成测试桩）

**Spec:** [docs/superpowers/specs/2026-06-27-beta-32-team-archive-mcp-daemon-design.md](../specs/2026-06-27-beta-32-team-archive-mcp-daemon-design.md)

**Naming note:** spec §4.3 用 `packages/search-engine` 是 conceptual 指代，本 plan 用真实 crate 名：`packages/intent-parser` + `packages/harness` + `packages/result-normalizer` + `packages/ranker` + `packages/search-backends/{local-index,semantic-index}`。

---

## Task 0：开 cycle 预检 + feature branch（不 commit、几秒）

**Goal:** 起点状态确认 — 仓库干净 + 分支切出。

- [ ] **Step 0.1: 看仓库状态干净**

```bash
cd /Users/alice/Work/LocalFind
git status
git log --oneline -5
```

Expected: working tree clean；HEAD 应位于 BETA-31 / v0.8.0 之后（commit `855242b` doc-sync 或更新）。

- [ ] **Step 0.2: 看现有 crate 结构对齐 spec**

```bash
ls packages/ | sort
grep -A5 '^members' Cargo.toml
```

Expected: 看到 `intent-parser` / `harness` / `result-normalizer` / `ranker` / `indexer` / `model-runtime` / `evals` / `search-backends/`；workspace members 含 `apps/desktop/src-tauri` 和 `apps/locifind-cli`，**无** `apps/daemon` 也**无** `packages/locifind-server`（本 cycle 新加）。

- [ ] **Step 0.3: 开 feature branch**

```bash
git checkout -b feat-beta-32-team-archive-mcp-daemon
git status
```

Expected: switched to new branch、working tree clean。

---

## Task 1：C1 a — `packages/indexer` 加 `IndexProgress` callback trait（无破坏改动）

**Goal:** 为 daemon 提供"边索引边 tracing log 进度"通道，桌面 app 现行行为完全等价（默认 no-op impl）。

**Files:**
- Create: `packages/indexer/src/progress.rs`
- Modify: `packages/indexer/src/lib.rs:1-30`（pub mod 加 progress）
- Modify: `packages/indexer/src/scan.rs:?-?`（在 `index_dirs` 等函数加可选 progress 参数 / 或新加 `_with_progress` 变体）
- Test: `packages/indexer/src/progress.rs` 自带 `#[cfg(test)]`

- [ ] **Step 1.1: 写 progress trait + no-op impl + 单元测试（先看 scan.rs 当前签名）**

```bash
grep -n 'pub fn index_dirs' packages/indexer/src/scan.rs | head -5
```

记下当前签名（应类似 `pub fn index_dirs(&self, roots: &[PathBuf]) -> Result<IndexStats, IndexError>`），下一步要新加 `_with_progress` 变体。

新建 `packages/indexer/src/progress.rs`：

```rust
//! 索引进度回调 — daemon 走 tracing log、桌面 app 默认走 no-op、
//! 测试代码可注入 spy 收集进度事件。

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};

/// 索引进度回调 trait。线程安全，可被 indexer 内部跨线程调用。
pub trait IndexProgress: Send + Sync {
    /// 单文件索引完成（成功或跳过）。
    fn on_file(&self, path: &Path, mime: &str, indexed: bool);
    /// 整批索引完成。
    fn on_batch_done(&self, scanned: u64, indexed: u64);
}

/// 默认 no-op 实现。桌面 app 走这个、行为与改造前 100% 等价。
#[derive(Default, Clone, Copy)]
pub struct NoopProgress;

impl IndexProgress for NoopProgress {
    fn on_file(&self, _: &Path, _: &str, _: bool) {}
    fn on_batch_done(&self, _: u64, _: u64) {}
}

/// 测试 spy 收集进度计数。
#[derive(Default)]
pub struct SpyProgress {
    pub files: AtomicU64,
    pub batches: AtomicU64,
}

impl IndexProgress for SpyProgress {
    fn on_file(&self, _: &Path, _: &str, _: bool) {
        self.files.fetch_add(1, Ordering::Relaxed);
    }
    fn on_batch_done(&self, _: u64, _: u64) {
        self.batches.fetch_add(1, Ordering::Relaxed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn noop_progress_compiles_and_no_panics() {
        let p = NoopProgress;
        p.on_file(&PathBuf::from("/x"), "text/plain", true);
        p.on_batch_done(10, 10);
    }

    #[test]
    fn spy_progress_counts() {
        let s = SpyProgress::default();
        s.on_file(&PathBuf::from("/a"), "text/plain", true);
        s.on_file(&PathBuf::from("/b"), "text/plain", false);
        s.on_batch_done(2, 1);
        assert_eq!(s.files.load(Ordering::Relaxed), 2);
        assert_eq!(s.batches.load(Ordering::Relaxed), 1);
    }
}
```

`packages/indexer/src/lib.rs` 第 1-30 行附近加 `pub mod progress;`。

- [ ] **Step 1.2: 跑测试 fail（progress mod 还没被 lib.rs 加载、或新单测 import 失败）**

```bash
cargo test -p locifind-indexer progress::tests
```

Expected: FAIL（mod 未注册）或 NotFound。

- [ ] **Step 1.3: lib.rs 注册 mod 再跑**

```bash
grep -n '^pub mod' packages/indexer/src/lib.rs | head
```

在已有 `pub mod ...;` 列表之后加：

```rust
pub mod progress;
```

- [ ] **Step 1.4: 跑测试 pass**

```bash
cargo test -p locifind-indexer progress::tests
```

Expected: PASS 2 个测试。

- [ ] **Step 1.5: 不动 scan.rs 现行签名、用 _with_progress 变体（避免触动桌面 app 调用）**

`packages/indexer/src/scan.rs` 在 `index_dirs` 之后加（保留旧函数不动）：

```rust
impl MusicIndex {
    /// 与 `index_dirs` 等价，但接受 IndexProgress 回调汇报每文件 / 每批进度。
    /// 桌面 app 维持调用旧 `index_dirs`（行为不变）；daemon 调用本函数。
    pub fn index_dirs_with_progress<P: IndexProgress>(
        &self,
        roots: &[PathBuf],
        progress: &P,
    ) -> Result<IndexStats, IndexError> {
        // 实现 = 复用 index_dirs 内部循环、在每文件处理后 progress.on_file，
        // 批结束时 progress.on_batch_done。建议提取私有 helper 让两 API 共享。
        // （具体实现按当前 index_dirs 真实结构写——subagent 在执行时按当前代码补全）
        unimplemented!("subagent 执行时按 index_dirs 真实循环结构填充")
    }
}
```

> **subagent 注意**：先 `cat packages/indexer/src/scan.rs` 看 `index_dirs` 真实结构、再决定是把循环提到私有 `index_one_root_with` helper 让两公共 API 共享、还是 _with_progress 完整复制一份循环。**必须保留 `index_dirs` 现行签名与行为**——桌面 app 通过这个 API 调。`DocumentIndex` 同理（如果 spec 期望文档索引也走 daemon、本 task 也要给文档索引版本）。

- [ ] **Step 1.6: 加单元测试 `index_dirs_with_progress_calls_callback`**

```rust
#[test]
fn index_dirs_with_progress_calls_callback() {
    use crate::progress::SpyProgress;
    use std::sync::atomic::Ordering;
    use tempfile::tempdir;
    use std::fs;

    let tmp = tempdir().unwrap();
    fs::write(tmp.path().join("a.txt"), "hello").unwrap();
    fs::write(tmp.path().join("b.txt"), "world").unwrap();

    let db = tempdir().unwrap();
    let idx = MusicIndex::open(db.path().join("idx.db")).unwrap();
    let spy = SpyProgress::default();
    let stats = idx.index_dirs_with_progress(&[tmp.path().to_path_buf()], &spy).unwrap();

    assert!(stats.scanned >= 2);
    assert!(spy.files.load(Ordering::Relaxed) >= 2);
    assert!(spy.batches.load(Ordering::Relaxed) >= 1);
}
```

- [ ] **Step 1.7: cargo test + clippy + fmt 全过**

```bash
cargo test -p locifind-indexer
cargo clippy -p locifind-indexer --all-targets -- -D warnings
cargo fmt -p locifind-indexer -- --check
```

Expected: 全 PASS、0 warning、fmt 净。

- [ ] **Step 1.8: workspace 验证桌面 app 不破**

```bash
cargo check --workspace
```

Expected: 全过（桌面 app 仍调旧 `index_dirs`、行为等价）。

- [ ] **Step 1.9: commit**

```bash
git add packages/indexer/
git commit -m "BETA-32 C1a：indexer 加 IndexProgress callback trait + _with_progress 变体"
```

---

## Task 2：C1 b — `packages/indexer` 加 schema version metadata 表（forward-compatible）

**Goal:** 让 daemon 启动时能 fail-fast 检查 schema 版本；桌面 app 老 db 自动 migrate 到带 version 表。

**Files:**
- Modify: `packages/indexer/src/db.rs:?-?`（SCHEMA 常量加 `CREATE TABLE IF NOT EXISTS schema_meta(key TEXT PRIMARY KEY, value TEXT)`、open 时若无 version 行就 INSERT 当前 `INDEXER_SCHEMA_VERSION`）
- Create: `packages/indexer/src/version.rs`（pub const + reader）
- Test: 同 `version.rs` `#[cfg(test)]`

- [ ] **Step 2.1: 写 version mod + 常量 + open 后自动 ensure 行**

```rust
// packages/indexer/src/version.rs
use rusqlite::Connection;
use crate::IndexError;

/// 索引 SQLite schema 版本。增 schema 字段 / 表时必须 bump。
/// daemon 启动时检查；不匹配则要求 --allow-rebuild-schema 显式重建。
pub const INDEXER_SCHEMA_VERSION: &str = "1";

pub fn ensure_schema_version(conn: &Connection) -> Result<(), IndexError> {
    conn.execute(
        "INSERT OR IGNORE INTO schema_meta(key, value) VALUES('version', ?1)",
        [INDEXER_SCHEMA_VERSION],
    )?;
    Ok(())
}

pub fn read_schema_version(conn: &Connection) -> Result<Option<String>, IndexError> {
    let mut stmt = conn.prepare("SELECT value FROM schema_meta WHERE key='version'")?;
    let mut rows = stmt.query([])?;
    Ok(rows.next()?.map(|r| r.get::<_, String>(0)).transpose()?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    fn fresh() -> Connection {
        let c = Connection::open_in_memory().unwrap();
        c.execute(
            "CREATE TABLE schema_meta(key TEXT PRIMARY KEY, value TEXT)",
            [],
        ).unwrap();
        c
    }

    #[test]
    fn ensure_then_read_returns_current_version() {
        let c = fresh();
        ensure_schema_version(&c).unwrap();
        assert_eq!(read_schema_version(&c).unwrap().as_deref(), Some(INDEXER_SCHEMA_VERSION));
    }

    #[test]
    fn read_returns_none_when_empty() {
        let c = fresh();
        assert!(read_schema_version(&c).unwrap().is_none());
    }
}
```

`packages/indexer/src/lib.rs` 加 `pub mod version;` + `pub use version::{INDEXER_SCHEMA_VERSION, ensure_schema_version, read_schema_version};`。

- [ ] **Step 2.2: 在 `db.rs` SCHEMA 常量末尾加 schema_meta 表 DDL（idempotent、桌面 app 老 db 升级安全）**

```bash
grep -n 'CREATE TABLE' packages/indexer/src/db.rs | head
```

在 SCHEMA 字符串中追加：

```sql
CREATE TABLE IF NOT EXISTS schema_meta (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL
);
```

`db.rs` 的 `open` / `init` 路径在 SCHEMA 执行完后调一次 `ensure_schema_version(&conn)?;`。

- [ ] **Step 2.3: 跑测试**

```bash
cargo test -p locifind-indexer version::tests
cargo test -p locifind-indexer  # workspace 已有 indexer 测试也要全过
```

Expected: 新单测 2 个 PASS、已有测试全过。

- [ ] **Step 2.4: clippy + fmt + workspace check**

```bash
cargo clippy -p locifind-indexer --all-targets -- -D warnings
cargo fmt -p locifind-indexer -- --check
cargo check --workspace
```

Expected: 全过。

- [ ] **Step 2.5: commit**

```bash
git add packages/indexer/
git commit -m "BETA-32 C1b：indexer 加 schema_meta 表 + version 常量"
```

---

## Task 3：C2 a — 新 `packages/locifind-server` 骨架 + workspace 注册

**Goal:** crate 创建、依赖 wire 通、空 lib build pass。

**Files:**
- Create: `packages/locifind-server/Cargo.toml`
- Create: `packages/locifind-server/src/lib.rs`
- Modify: `Cargo.toml`（workspace members 加 `packages/locifind-server`）
- Modify: `Cargo.toml`（workspace [workspace.dependencies] 加 rmcp / axum / secrecy / subtle、若未有）

- [ ] **Step 3.1: 写 Cargo.toml**

```toml
# packages/locifind-server/Cargo.toml
[package]
name = "locifind-server"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[dependencies]
locifind-indexer = { path = "../indexer" }
locifind-model-runtime = { path = "../model-runtime" }
locifind-intent-parser = { path = "../intent-parser" }
locifind-harness = { path = "../harness" }
locifind-result-normalizer = { path = "../result-normalizer" }
locifind-ranker = { path = "../ranker" }
locifind-search-backends-common = { path = "../search-backends/common" }
locifind-local-index-backend = { path = "../search-backends/local-index" }
locifind-semantic-index-backend = { path = "../search-backends/semantic-index" }

tokio = { workspace = true, features = ["rt-multi-thread", "macros", "sync", "time", "fs", "signal"] }
axum = { version = "0.7", features = ["macros", "json"] }
tower = "0.5"
tower-http = { version = "0.6", features = ["trace", "limit"] }
rmcp = { version = "0.2", features = ["server", "transport-streamable-http-server"] }

serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
tracing = "0.1"
secrecy = { version = "0.10", features = ["serde"] }
subtle = "2.6"
parking_lot = "0.12"
async-trait = "0.1"
chrono = { version = "0.4", features = ["serde"] }

[dev-dependencies]
tempfile = "3"
httptest = "0.16"
tokio = { workspace = true, features = ["test-util", "macros", "rt-multi-thread"] }
```

> **subagent 注意**：workspace `[workspace.dependencies]` 中部分 crate（如 rmcp、axum、secrecy、subtle、httptest）若尚未加，需要顺手加到根 Cargo.toml。先 `grep -E '^(rmcp|axum|secrecy|subtle)' Cargo.toml` 确认。如果 rmcp 0.2 接口与本 plan 描述偏差较大，allowed to bump 到 cargo 当前发布最新版、API 调用按真实版本适配，但保持 spec 描述的 streamable-HTTP transport。

- [ ] **Step 3.2: 写空 lib.rs**

```rust
// packages/locifind-server/src/lib.rs

//! LociFind 服务端核心 — 把现有 hybrid 检索能力包装成 MCP server。
//! daemon binary 和未来桌面 app 嵌入模式都通过本 crate 复用同一份 server 逻辑。

pub mod config;
pub mod auth;
pub mod tools;
pub mod admin;
pub mod reindex;
pub mod mcp;
pub mod app;       // axum::Router 工厂

pub use config::{ServerConfig, ServerCtx};
```

> 各 mod 后续 task 填充；先放空 mod 让 cargo check 过。

- [ ] **Step 3.3: 占位写空 mod**

为每个 pub mod 各建一个 `mod.rs` 或同名文件、内放 `// placeholder` 注释 + 必要的 pub re-export 占位（具体 task 各自填）。subagent 此时仅需让 `cargo check -p locifind-server` 过。

- [ ] **Step 3.4: 加 workspace members**

`Cargo.toml`（根）的 `[workspace] members = [...]` 追加：

```toml
"packages/locifind-server",
```

- [ ] **Step 3.5: cargo check pass**

```bash
cargo check -p locifind-server
cargo check --workspace
```

Expected: 全过、locifind-server build 成空 lib。

- [ ] **Step 3.6: commit**

```bash
git add packages/locifind-server/ Cargo.toml Cargo.lock
git commit -m "BETA-32 C2a：locifind-server crate 骨架 + workspace 注册"
```

---

## Task 4：C2 b — ServerConfig + ServerCtx + bearer auth middleware

**Goal:** 配置类型、依赖容器、bearer token 中间件（常量时间比较）。

**Files:**
- Create: `packages/locifind-server/src/config.rs`
- Create: `packages/locifind-server/src/auth.rs`

- [ ] **Step 4.1: 写 config.rs**

```rust
// packages/locifind-server/src/config.rs

use secrecy::SecretString;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::level_filters::LevelFilter;

use locifind_indexer::{MusicIndex, DocumentIndex};
use locifind_indexer::embed::TextEmbedder;

/// 启动参数 — 由 CLI / TOML / env 合并填充。
#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub bind_addr: SocketAddr,
    pub bearer_token: SecretString,
    pub data_dir: PathBuf,
    pub root: PathBuf,
    pub model_path: PathBuf,
    pub log_level: LevelFilter,
}

/// 运行时依赖容器 — 注入到所有 tools / handlers。
/// Arc-clonable，axum State 通过 Arc<ServerCtx> 传递。
pub struct ServerCtx {
    pub config: ServerConfig,
    pub music_index: Arc<MusicIndex>,
    pub document_index: Arc<DocumentIndex>,
    pub embedder: Arc<dyn TextEmbedder>,
    // 进度状态、reindex IN_FLIGHT、indexed_at 等可加 RwLock<RuntimeState> 字段。
    pub state: Arc<parking_lot::RwLock<RuntimeState>>,
}

#[derive(Default)]
pub struct RuntimeState {
    pub indexed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub doc_count: u64,
    pub reindex_in_flight: bool,
}
```

> 真实 ServerCtx 构造放 `app.rs` 的 builder 里、本 task 只定义 type 形状。

- [ ] **Step 4.2: 写 auth.rs**

```rust
// packages/locifind-server/src/auth.rs

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    middleware::Next,
    response::Response,
    body::Body,
    http::Request,
};
use secrecy::ExposeSecret;
use std::sync::Arc;
use subtle::ConstantTimeEq;

use crate::config::ServerCtx;

/// 校验 `Authorization: Bearer <token>` header，token 用 subtle 常量时间比较。
/// 401 = 缺失 / 非 Bearer / 不匹配。
pub async fn require_bearer(
    State(ctx): State<Arc<ServerCtx>>,
    headers: HeaderMap,
    req: Request<Body>,
    next: Next,
) -> Result<Response, StatusCode> {
    let provided = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.strip_prefix("Bearer "))
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let expected = ctx.config.bearer_token.expose_secret().as_bytes();
    let provided_bytes = provided.as_bytes();

    if provided_bytes.len() != expected.len() {
        // 长度差异先返 401 — 但要等长度比较后才能用 ConstantTimeEq；
        // 此处长度泄露不算真正 timing attack 维度（token 由 server 指定）。
        return Err(StatusCode::UNAUTHORIZED);
    }
    let ok: bool = provided_bytes.ct_eq(expected).into();
    if !ok {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{Router, routing::get, http::Request, body::Body};
    use secrecy::SecretString;
    use std::sync::Arc;
    use tower::ServiceExt;

    fn ctx(token: &str) -> Arc<ServerCtx> {
        Arc::new(ServerCtx {
            config: ServerConfig {
                bind_addr: "127.0.0.1:0".parse().unwrap(),
                bearer_token: SecretString::new(token.into()),
                data_dir: "/tmp".into(),
                root: "/tmp".into(),
                model_path: "/tmp/model".into(),
                log_level: tracing::level_filters::LevelFilter::OFF,
            },
            // music_index / document_index / embedder 真实测试中用 stub 注入
            // 此处 ConstFn auth 中间件单测中可暂时 panic—但 axum middleware
            // 不实际 deref ctx 中这些字段，所以 unsafe Arc::from_raw 占位 OK。
            // 推荐做法：在 tests 模块加一个 `pub fn test_ctx_with_only_token(...)` builder。
            music_index: panic!("test ctx — auth tests should not deref music_index"),
            document_index: panic!("..."),
            embedder: panic!("..."),
            state: Default::default(),
        })
    }

    // 真实做法：把 ServerCtx 拆出 `AuthCtx { config }` sub-context 让 auth 单测纯净。
    // —— subagent 在实现时改造 ServerCtx / 增 AuthCtx；本 plan 给出 intent，
    // 让 subagent 选最干净的拆分。

    #[tokio::test]
    async fn missing_header_returns_401() {
        // ... 用 axum::http::Request<Body> + Router::oneshot 测；
        // 见 axum 0.7 middleware 测试样板
    }

    #[tokio::test]
    async fn wrong_token_returns_401() { /* ... */ }

    #[tokio::test]
    async fn correct_token_passes() { /* ... */ }

    #[tokio::test]
    async fn wrong_length_returns_401() { /* ... */ }
}
```

> **subagent 注意**：上方 `panic!` 是占位，正确做法是把 auth 中间件参数从 `State<Arc<ServerCtx>>` 改为更窄的 `State<Arc<AuthCtx>>`（仅含 token），让单测可以 trivially 构造。或者 ServerCtx 用 lazy Arc 给非 auth-related 字段。subagent 选最简洁的实现。

- [ ] **Step 4.3: 跑 auth 单测**

```bash
cargo test -p locifind-server auth::tests
```

Expected: 4 测试全 PASS（missing / wrong / correct / wrong-length）。

- [ ] **Step 4.4: clippy + fmt**

```bash
cargo clippy -p locifind-server --all-targets -- -D warnings
cargo fmt -p locifind-server -- --check
```

Expected: 0 warning、fmt 净。

- [ ] **Step 4.5: commit**

```bash
git add packages/locifind-server/
git commit -m "BETA-32 C2b：ServerConfig + ServerCtx + bearer auth middleware"
```

---

## Task 5：C2 c — Tool trait + SearchTool + ListRootsTool（TDD）

**Goal:** 两个 MCP tool 的纯 Rust 入口，签名稳定、有单测。

**Files:**
- Create: `packages/locifind-server/src/tools/mod.rs`
- Create: `packages/locifind-server/src/tools/search.rs`
- Create: `packages/locifind-server/src/tools/list_roots.rs`

- [ ] **Step 5.1: 先写 tools/mod.rs trait + ToolRegistry + 输出 schema**

```rust
// packages/locifind-server/src/tools/mod.rs

pub mod search;
pub mod list_roots;

use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use crate::config::ServerCtx;

#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn input_schema(&self) -> Value;
    async fn invoke(&self, args: Value, ctx: Arc<ServerCtx>) -> Result<Value, ToolError>;
}

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("invalid params: {0}")]
    InvalidParams(String),
    #[error("internal error: {0}")]
    Internal(String),
}

/// 默认注册器 — 当前两个 tool。
pub fn default_tools() -> Vec<Arc<dyn Tool>> {
    vec![
        Arc::new(search::SearchTool),
        Arc::new(list_roots::ListRootsTool),
    ]
}
```

- [ ] **Step 5.2: 写 search.rs（TDD：先红、再绿）**

```rust
// packages/locifind-server/src/tools/search.rs

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

use super::{Tool, ToolError};
use crate::config::ServerCtx;

const HARD_LIMIT_CAP: usize = 50;
const DEFAULT_LIMIT: usize = 20;

#[derive(Deserialize)]
struct SearchInput {
    query: String,
    #[serde(default)]
    limit: Option<usize>,
}

#[derive(Serialize)]
struct SearchHit {
    path: String,
    name: String,
    size: u64,
    mtime: i64,
    score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    why: Option<String>,
}

#[derive(Serialize)]
struct SearchOutput {
    results: Vec<SearchHit>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    degraded: bool,
}

pub struct SearchTool;

#[async_trait]
impl Tool for SearchTool {
    fn name(&self) -> &'static str { "search" }
    fn description(&self) -> &'static str {
        "Search the indexed archive by natural language query. Returns hit file paths + metadata."
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {"type": "string", "description": "自然语言查询"},
                "limit": {"type": "integer", "minimum": 1, "maximum": HARD_LIMIT_CAP,
                          "default": DEFAULT_LIMIT}
            },
            "required": ["query"]
        })
    }

    async fn invoke(&self, args: Value, ctx: Arc<ServerCtx>) -> Result<Value, ToolError> {
        let input: SearchInput = serde_json::from_value(args)
            .map_err(|e| ToolError::InvalidParams(e.to_string()))?;

        if input.query.trim().is_empty() {
            return Err(ToolError::InvalidParams("query 不能为空".into()));
        }
        let limit = input.limit.unwrap_or(DEFAULT_LIMIT).min(HARD_LIMIT_CAP).max(1);

        // 复用 packages/harness::fallback_chain 调度 backend
        // —— subagent 实现时按 packages/evals 当前 invoke pattern 抄一份适应 daemon。
        // 这里给出 stub 输出让单测先通。
        let _ = ctx; // 实际用 ctx.embedder / ctx.music_index / ctx.document_index
        let _ = limit;

        let output = SearchOutput { results: vec![], degraded: false };
        Ok(serde_json::to_value(output).unwrap())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn input_schema_has_query_and_limit() {
        let s = SearchTool.input_schema();
        assert_eq!(s["properties"]["query"]["type"], "string");
        assert_eq!(s["properties"]["limit"]["maximum"], HARD_LIMIT_CAP);
        assert_eq!(s["required"], json!(["query"]));
    }

    #[tokio::test]
    async fn empty_query_returns_invalid_params() {
        // ctx 需要 test builder — 当 ServerCtx 重构出 stub builder 时补
        // 此测试占位、Task 6 SearchTool 真实 wire-up 时补齐
    }

    #[tokio::test]
    async fn limit_caps_at_hard_limit() {
        // 同上 — 真实测试在 Task 6 接 stub backend 后写
    }
}
```

- [ ] **Step 5.3: 写 list_roots.rs**

```rust
// packages/locifind-server/src/tools/list_roots.rs

use async_trait::async_trait;
use serde::Serialize;
use serde_json::{json, Value};
use std::sync::Arc;

use super::{Tool, ToolError};
use crate::config::ServerCtx;

#[derive(Serialize)]
struct ListRootsOutput {
    root: String,
    doc_count: u64,
    indexed_at: Option<String>,
}

pub struct ListRootsTool;

#[async_trait]
impl Tool for ListRootsTool {
    fn name(&self) -> &'static str { "list_roots" }
    fn description(&self) -> &'static str {
        "Return the daemon's archive root directory and index stats."
    }
    fn input_schema(&self) -> Value {
        json!({"type": "object", "properties": {}, "additionalProperties": false})
    }

    async fn invoke(&self, _args: Value, ctx: Arc<ServerCtx>) -> Result<Value, ToolError> {
        let st = ctx.state.read();
        let output = ListRootsOutput {
            root: ctx.config.root.display().to_string(),
            doc_count: st.doc_count,
            indexed_at: st.indexed_at.map(|t| t.to_rfc3339()),
        };
        Ok(serde_json::to_value(output).unwrap())
    }
}

#[cfg(test)]
mod tests {
    // 真实测试需 stub ctx — 与 SearchTool 同款、在 Task 6 ctx test builder 落定后补齐
}
```

- [ ] **Step 5.4: 跑测试 + lint**

```bash
cargo test -p locifind-server tools::
cargo clippy -p locifind-server --all-targets -- -D warnings
cargo fmt -p locifind-server -- --check
```

Expected: schema 单测 PASS、其余测试占位 PASS（暂无 assert）、0 warning。

- [ ] **Step 5.5: commit**

```bash
git add packages/locifind-server/
git commit -m "BETA-32 C2c：Tool trait + SearchTool + ListRootsTool 骨架"
```

---

## Task 6：C2 d — SearchTool 接现实 harness（fallback_chain）+ stub-friendly test ctx

**Goal:** SearchTool::invoke 真正调 `packages/harness::fallback_chain` 走完 intent-parser → backend → ranker；补齐 SearchTool 单测（stub embedder + 内存 SQLite）。

**Files:**
- Modify: `packages/locifind-server/src/tools/search.rs`（接现实 harness）
- Modify: `packages/locifind-server/src/config.rs`（加 test builder）
- Create: `packages/locifind-server/src/test_support.rs`（stub embedder + ctx builder、`#[cfg(feature = "test-support")]` gate 让 daemon 集成测试也复用）

- [ ] **Step 6.1: 看 packages/evals 当前是怎么 wire 起 backend + harness 的**

```bash
ls packages/evals/src/bin/
grep -n 'fallback_chain' packages/evals/src/**/*.rs | head
grep -n 'IntentRouter' packages/harness/src/**/*.rs | head
```

记下 evals 的调用模式（应类似 `IntentRouter::new(...).resolve(query) → backend.search_expanded() → ranker::rank()`）。

- [ ] **Step 6.2: SearchTool::invoke 接现实**

按 packages/evals 同款 wire SearchTool：构造 `IntentRouter`、调 `fallback_chain::run_fallback_chain`、把结果按 spec §4.1 表里的字段（path/name/size/mtime/score/why）jsonify。`why` 字段聚合命中原因（同义词 / 跨语言 / FTS-only）—— subagent 按 BETA-15B-5 段落高亮聚合逻辑提取，**不返原文 snippet**。

- [ ] **Step 6.3: test_support.rs — stub embedder + ctx builder**

```rust
// packages/locifind-server/src/test_support.rs
//! 测试支持：stub embedder（返固定 dim=768 向量）+ ctx builder（内存 SQLite）。

use crate::config::{ServerConfig, ServerCtx, RuntimeState};
use locifind_indexer::embed::TextEmbedder;
use locifind_indexer::IndexError;
use secrecy::SecretString;
use std::sync::Arc;

pub struct StubEmbedder { pub dim: usize }
impl TextEmbedder for StubEmbedder {
    fn embed(&self, text: &str) -> Result<Vec<f32>, IndexError> {
        // 简单 hash → 固定向量；同 query 同 vec、不同 query 不同 vec
        let h = text.bytes().fold(0u32, |a, b| a.wrapping_mul(31).wrapping_add(b as u32));
        let mut v = vec![0.0; self.dim];
        for i in 0..self.dim {
            v[i] = ((h.wrapping_add(i as u32)) % 100) as f32 / 100.0;
        }
        // L2 normalize
        let norm: f32 = v.iter().map(|x| x*x).sum::<f32>().sqrt();
        if norm > 0.0 { v.iter_mut().for_each(|x| *x /= norm); }
        Ok(v)
    }
    fn model_id(&self) -> &str { "stub-embedder" }
}

pub fn build_test_ctx_inmem() -> Arc<ServerCtx> {
    // 内存 SQLite + stub embedder + 临时 root
    // 详见 subagent 实现 —— 复用 packages/indexer 的 in-memory test helper
    todo!("subagent fill: 复用 packages/indexer 现行测试 helper 构造")
}
```

- [ ] **Step 6.4: 补齐 SearchTool 单测**

```rust
// packages/locifind-server/src/tools/search.rs 末尾测试模块
#[tokio::test]
async fn empty_query_returns_invalid_params() {
    use crate::test_support::build_test_ctx_inmem;
    let ctx = build_test_ctx_inmem();
    let err = SearchTool.invoke(json!({"query": ""}), ctx).await.unwrap_err();
    assert!(matches!(err, ToolError::InvalidParams(_)));
}

#[tokio::test]
async fn limit_caps_at_hard_limit() {
    use crate::test_support::build_test_ctx_inmem;
    let ctx = build_test_ctx_inmem();
    let v = SearchTool.invoke(
        json!({"query": "anything", "limit": 1000}),
        ctx,
    ).await.unwrap();
    // results 数量不应超过 HARD_LIMIT_CAP（这里也允许 0、因为索引可能空）
    assert!(v["results"].as_array().unwrap().len() <= HARD_LIMIT_CAP);
}

#[tokio::test]
async fn list_roots_returns_root() {
    use crate::test_support::build_test_ctx_inmem;
    use crate::tools::list_roots::ListRootsTool;
    let ctx = build_test_ctx_inmem();
    let v = ListRootsTool.invoke(json!({}), ctx.clone()).await.unwrap();
    assert_eq!(v["root"], ctx.config.root.display().to_string());
}
```

- [ ] **Step 6.5: 测试 + lint + workspace 验证**

```bash
cargo test -p locifind-server
cargo clippy -p locifind-server --all-targets -- -D warnings
cargo fmt -p locifind-server -- --check
cargo check --workspace
```

Expected: 单测全 PASS、0 warning、workspace 净。

- [ ] **Step 6.6: commit**

```bash
git add packages/locifind-server/
git commit -m "BETA-32 C2d：SearchTool 接 harness + test_support + 单测全过"
```

---

## Task 7：C2 e — admin endpoints (/health, /admin/reindex with IN_FLIGHT, /metrics 可选) + reindex 原子 swap

**Goal:** 三个管理 endpoint 实现 + reindex 后台并发 + atomic swap + 重入 409。

**Files:**
- Create: `packages/locifind-server/src/admin.rs`
- Create: `packages/locifind-server/src/reindex.rs`

- [ ] **Step 7.1: admin.rs**

```rust
// packages/locifind-server/src/admin.rs

use axum::{extract::State, http::StatusCode, Json};
use serde::Serialize;
use std::sync::Arc;
use crate::config::ServerCtx;
use crate::reindex::trigger_reindex;

#[derive(Serialize)]
pub struct HealthResp { pub status: &'static str, pub version: &'static str }

pub async fn health() -> Json<HealthResp> {
    Json(HealthResp { status: "ok", version: env!("CARGO_PKG_VERSION") })
}

#[derive(Serialize)]
pub struct ReindexResp {
    pub status: &'static str,
    pub doc_count: u64,
    pub duration_ms: u128,
}

pub async fn admin_reindex(
    State(ctx): State<Arc<ServerCtx>>,
) -> Result<Json<ReindexResp>, StatusCode> {
    match trigger_reindex(ctx).await {
        Ok(r) => Ok(Json(r)),
        Err(crate::reindex::ReindexError::InFlight) => Err(StatusCode::CONFLICT),
        Err(_) => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
```

- [ ] **Step 7.2: reindex.rs（IN_FLIGHT guard + atomic swap）**

```rust
// packages/locifind-server/src/reindex.rs

use std::sync::Arc;
use std::time::Instant;
use thiserror::Error;
use crate::config::ServerCtx;
use crate::admin::ReindexResp;

#[derive(Debug, Error)]
pub enum ReindexError {
    #[error("已有 reindex 在进行中")] InFlight,
    #[error("索引失败：{0}")] Internal(String),
}

pub async fn trigger_reindex(ctx: Arc<ServerCtx>) -> Result<ReindexResp, ReindexError> {
    {
        let mut st = ctx.state.write();
        if st.reindex_in_flight { return Err(ReindexError::InFlight); }
        st.reindex_in_flight = true;
    }
    let _guard = ReindexGuard { ctx: ctx.clone() };

    let started = Instant::now();
    // 1) 写新 db 到 <data_dir>/index.db.rebuild
    // 2) fs::rename(index.db -> index.db.old)
    // 3) fs::rename(index.db.rebuild -> index.db)
    // 4) 重新打开连接池、旧池 drain（drop 即可、tokio task 引用计数归零关闭）
    // 5) fs::remove_file(index.db.old)
    //
    // 实现细节由 subagent 按 packages/indexer 当前 DB lifecycle 补全。
    //
    let doc_count = run_full_reindex(&ctx).await.map_err(|e| ReindexError::Internal(e.to_string()))?;
    {
        let mut st = ctx.state.write();
        st.doc_count = doc_count;
        st.indexed_at = Some(chrono::Utc::now());
    }
    Ok(ReindexResp {
        status: "completed",
        doc_count,
        duration_ms: started.elapsed().as_millis(),
    })
}

struct ReindexGuard { ctx: Arc<ServerCtx> }
impl Drop for ReindexGuard {
    fn drop(&mut self) {
        self.ctx.state.write().reindex_in_flight = false;
    }
}

async fn run_full_reindex(ctx: &ServerCtx) -> anyhow::Result<u64> {
    // 调 packages/indexer 的 MusicIndex::index_dirs_with_progress + DocumentIndex 同款
    // 用 ctx.config.root，写入 rebuild db；
    // tokio::task::spawn_blocking 包裹（indexer 是 sync）。
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::build_test_ctx_inmem;

    #[tokio::test]
    async fn concurrent_reindex_second_returns_in_flight() {
        let ctx = build_test_ctx_inmem();
        ctx.state.write().reindex_in_flight = true;
        let err = trigger_reindex(ctx).await.unwrap_err();
        assert!(matches!(err, ReindexError::InFlight));
    }

    #[tokio::test]
    async fn guard_clears_flag_on_drop() {
        let ctx = build_test_ctx_inmem();
        {
            let _g = ReindexGuard { ctx: ctx.clone() };
            ctx.state.write().reindex_in_flight = true;
        }
        // _g drop 后 flag 被清
        assert!(!ctx.state.read().reindex_in_flight);
    }
}
```

- [ ] **Step 7.3: 跑测试 + lint**

```bash
cargo test -p locifind-server admin reindex
cargo clippy -p locifind-server --all-targets -- -D warnings
```

Expected: 两个 reindex 单测 PASS、0 warning。

- [ ] **Step 7.4: commit**

```bash
git add packages/locifind-server/
git commit -m "BETA-32 C2e：admin endpoints + reindex IN_FLIGHT guard"
```

---

## Task 8：C2 f — MCP server adapter（rmcp，streamable-HTTP transport）+ app.rs Router 工厂

**Goal:** Tool trait → rmcp ToolHandler 适配；axum Router 把 /mcp + /health + /admin/* + auth middleware 串起来。

**Files:**
- Create: `packages/locifind-server/src/mcp.rs`
- Create: `packages/locifind-server/src/app.rs`

- [ ] **Step 8.1: mcp.rs — Tool trait → rmcp ToolHandler 适配**

```rust
// packages/locifind-server/src/mcp.rs

//! Bridge Tool trait → rmcp ServerHandler / Tool 适配。
//! rmcp 0.x API 在演进 —— subagent 按 cargo 拉到的真实 rmcp 版本调 doc.rs 看 API。

use rmcp::*;  // 占位、subagent 按真实 import 补全
use std::sync::Arc;
use crate::config::ServerCtx;
use crate::tools::{Tool, default_tools};

pub struct LocifindMcpHandler {
    pub ctx: Arc<ServerCtx>,
    pub tools: Vec<Arc<dyn Tool>>,
}

impl LocifindMcpHandler {
    pub fn new(ctx: Arc<ServerCtx>) -> Self {
        Self { ctx, tools: default_tools() }
    }
}

// Implement rmcp ServerHandler trait:
//   - list_tools() → 返回 self.tools 的 schemas
//   - call_tool(name, args) → 查 tools 调 Tool::invoke、把 ToolError 映到 MCP error code
// 详见 subagent 按 rmcp 当前 API 实现。
```

- [ ] **Step 8.2: app.rs — Router 工厂**

```rust
// packages/locifind-server/src/app.rs

use axum::{Router, routing::{get, post}, middleware};
use std::sync::Arc;
use crate::config::ServerCtx;
use crate::auth::require_bearer;
use crate::admin::{health, admin_reindex};
use crate::mcp::LocifindMcpHandler;

/// 组装 axum Router — daemon 和未来嵌入式都用同一份。
pub fn build_app(ctx: Arc<ServerCtx>) -> Router {
    let mcp_handler = LocifindMcpHandler::new(ctx.clone());

    let protected = Router::new()
        .route("/mcp", post(/* rmcp streamable-HTTP handler */ todo!()))
        .route("/admin/reindex", post(admin_reindex))
        .route_layer(middleware::from_fn_with_state(ctx.clone(), require_bearer));

    let public = Router::new()
        .route("/health", get(health));

    Router::new()
        .merge(public)
        .merge(protected)
        .with_state(ctx)
}
```

> **subagent 注意**：rmcp 0.x streamable-HTTP transport 提供 `axum::handler::Handler` impl 或类似 helper —— subagent 翻文档接上。

- [ ] **Step 8.3: 单测 — 启 router、curl /health**

```rust
#[tokio::test]
async fn health_endpoint_no_auth_required() {
    use crate::test_support::build_test_ctx_inmem;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    let ctx = build_test_ctx_inmem();
    let app = build_app(ctx);
    let resp = app.oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await.unwrap();
    assert_eq!(resp.status(), 200);
}
```

- [ ] **Step 8.4: 跑测试 + lint**

```bash
cargo test -p locifind-server
cargo clippy -p locifind-server --all-targets -- -D warnings
```

Expected: 全 PASS、0 warning。

- [ ] **Step 8.5: commit**

```bash
git add packages/locifind-server/
git commit -m "BETA-32 C2f：MCP server adapter + axum Router 工厂"
```

---

## Task 9：C3 a — 新 `apps/daemon` 骨架 + workspace 注册 + clap CLI

**Goal:** 新 binary crate `locifindd`、CLI 参数解析、空 main 跑通。

**Files:**
- Create: `apps/daemon/Cargo.toml`
- Create: `apps/daemon/src/main.rs`
- Create: `apps/daemon/src/cli.rs`
- Modify: `Cargo.toml`（workspace members 加 `apps/daemon`）

- [ ] **Step 9.1: Cargo.toml**

```toml
# apps/daemon/Cargo.toml
[package]
name = "locifindd"
version.workspace = true
edition.workspace = true
license.workspace = true
authors.workspace = true

[[bin]]
name = "locifindd"
path = "src/main.rs"

[dependencies]
locifind-server = { path = "../../packages/locifind-server" }
locifind-indexer = { path = "../../packages/indexer" }
locifind-model-runtime = { path = "../../packages/model-runtime" }

tokio = { workspace = true, features = ["rt-multi-thread", "macros", "signal", "time"] }
axum = "0.7"
clap = { version = "4", features = ["derive", "env"] }
secrecy = "0.10"
toml = "0.8"
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
anyhow = "1"

[dev-dependencies]
tempfile = "3"
reqwest = { version = "0.12", default-features = false, features = ["rustls-tls", "json"] }
serde_json = "1"
```

- [ ] **Step 9.2: cli.rs**

```rust
// apps/daemon/src/cli.rs

use clap::Parser;
use std::net::SocketAddr;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(name = "locifindd", version, about = "LociFind 团队归档 MCP daemon")]
pub struct Cli {
    /// 索引根目录
    #[arg(long)]
    pub root: PathBuf,

    /// 监听地址（默认 0.0.0.0:8765）
    #[arg(long, default_value = "0.0.0.0:8765")]
    pub bind: SocketAddr,

    /// Bearer token（或 LOCIFINDD_TOKEN 环境变量）
    #[arg(long, env = "LOCIFINDD_TOKEN")]
    pub token: String,

    /// 索引 DB 目录
    #[arg(long)]
    pub data_dir: PathBuf,

    /// embedder GGUF 文件路径
    #[arg(long, env = "LOCIFINDD_MODEL_PATH")]
    pub model_path: PathBuf,

    /// 可选 TOML 配置（CLI 参数覆盖）
    #[arg(long)]
    pub config: Option<PathBuf>,

    #[arg(long, default_value = "text")]
    pub log_format: String,

    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// 允许启动时检测到 schema_meta 不一致或残留 rebuild 文件时重建
    #[arg(long)]
    pub allow_rebuild_schema: bool,
}
```

- [ ] **Step 9.3: main.rs 占位**

```rust
// apps/daemon/src/main.rs

mod cli;

use clap::Parser;
use cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    // tracing init
    // fail-fast 检查（Task 10）
    // build ctx + indexer 启动全量
    // bind axum + 跑 server
    // 信号处理（SIGINT/SIGTERM 优雅退出）
    println!("locifindd {} — root: {:?} bind: {}", env!("CARGO_PKG_VERSION"), cli.root, cli.bind);
    Ok(())
}
```

- [ ] **Step 9.4: workspace members 加 `apps/daemon` + cargo check**

```bash
# 改根 Cargo.toml members 加 "apps/daemon"
cargo check -p locifindd
cargo build -p locifindd --bin locifindd
./target/debug/locifindd --help
```

Expected: build PASS、`--help` 列出所有 flag。

- [ ] **Step 9.5: commit**

```bash
git add apps/daemon/ Cargo.toml Cargo.lock
git commit -m "BETA-32 C3a：apps/daemon 骨架 + clap CLI"
```

---

## Task 10：C3 b — fail-fast 启动检查 + ctx 构造 + indexer 启动全量 + 信号处理

**Goal:** 把 cli.rs 的参数 → ServerConfig → ServerCtx → 索引全量 → 启 axum → 接 SIGINT。

**Files:**
- Modify: `apps/daemon/src/main.rs`
- Create: `apps/daemon/src/preflight.rs`（fail-fast 六条 check）
- Create: `apps/daemon/src/lifecycle.rs`（启 server + 信号处理）

- [ ] **Step 10.1: preflight.rs**

```rust
// apps/daemon/src/preflight.rs

use std::path::Path;
use anyhow::{anyhow, Context, Result};

pub fn check_root(root: &Path) -> Result<()> {
    if !root.exists() { return Err(anyhow!("root 目录不存在：{}", root.display())); }
    if !root.is_dir() { return Err(anyhow!("root 不是目录：{}", root.display())); }
    // 可读性 — 尝试 read_dir
    std::fs::read_dir(root).context(format!("root 不可读：{}", root.display()))?;
    Ok(())
}

pub fn check_data_dir(data_dir: &Path) -> Result<()> {
    let parent = data_dir.parent().ok_or_else(|| anyhow!("data_dir 无父目录"))?;
    if !parent.exists() { std::fs::create_dir_all(parent)?; }
    // 写探针
    let probe = parent.join(".locifindd-write-probe");
    std::fs::write(&probe, b"x")?;
    std::fs::remove_file(&probe)?;
    Ok(())
}

pub fn check_token(token: &str) -> Result<()> {
    if token.len() < 32 { return Err(anyhow!("token 长度必须 ≥ 32 字符（当前 {}）", token.len())); }
    Ok(())
}

pub fn check_model(model_path: &Path) -> Result<()> {
    if !model_path.exists() { return Err(anyhow!("embedder model 文件不存在：{}", model_path.display())); }
    if !model_path.is_file() { return Err(anyhow!("embedder model 不是文件：{}", model_path.display())); }
    Ok(())
}

pub fn check_rebuild_leftover(data_dir: &Path, allow: bool) -> Result<()> {
    let rebuild = data_dir.join("index.db.rebuild");
    let old = data_dir.join("index.db.old");
    if rebuild.exists() || old.exists() {
        if allow {
            let _ = std::fs::remove_file(&rebuild);
            let _ = std::fs::remove_file(&old);
            Ok(())
        } else {
            Err(anyhow!("检测到 reindex 中断残留（{:?} / {:?}），重启加 --allow-rebuild-schema 清理", rebuild, old))
        }
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs;

    #[test]
    fn check_root_ok() {
        let d = tempdir().unwrap();
        check_root(d.path()).unwrap();
    }
    #[test]
    fn check_root_missing_fails() {
        let p = std::path::Path::new("/nonexistent/zzzzz");
        assert!(check_root(p).is_err());
    }
    #[test]
    fn check_token_min_length() {
        assert!(check_token("short").is_err());
        let long = "a".repeat(32);
        assert!(check_token(&long).is_ok());
    }
    #[test]
    fn check_rebuild_leftover_blocks_without_flag() {
        let d = tempdir().unwrap();
        fs::write(d.path().join("index.db.rebuild"), b"x").unwrap();
        assert!(check_rebuild_leftover(d.path(), false).is_err());
        assert!(check_rebuild_leftover(d.path(), true).is_ok());
        assert!(!d.path().join("index.db.rebuild").exists());
    }
}
```

- [ ] **Step 10.2: lifecycle.rs**

```rust
// apps/daemon/src/lifecycle.rs

use std::sync::Arc;
use anyhow::Result;
use axum::Router;
use locifind_server::ServerCtx;
use tokio::net::TcpListener;
use tokio::signal;
use tracing::{info, warn};

pub async fn serve(ctx: Arc<ServerCtx>, router: Router) -> Result<()> {
    let listener = TcpListener::bind(ctx.config.bind_addr).await?;
    info!("locifindd 监听 {}", ctx.config.bind_addr);
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    info!("locifindd 已退出");
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async { signal::ctrl_c().await.expect("install ctrl_c"); };
    #[cfg(unix)]
    let term = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("install SIGTERM")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => warn!("收到 SIGINT，准备退出"),
        _ = term => warn!("收到 SIGTERM，准备退出"),
    }
}
```

- [ ] **Step 10.3: main.rs 接全套**

```rust
// apps/daemon/src/main.rs
mod cli;
mod preflight;
mod lifecycle;

use anyhow::Result;
use clap::Parser;
use cli::Cli;
use locifind_server::{ServerConfig, ServerCtx, app::build_app};
use secrecy::SecretString;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_new(&cli.log_level).unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    preflight::check_root(&cli.root)?;
    preflight::check_data_dir(&cli.data_dir)?;
    preflight::check_token(&cli.token)?;
    preflight::check_model(&cli.model_path)?;
    preflight::check_rebuild_leftover(&cli.data_dir, cli.allow_rebuild_schema)?;
    // schema version check 由 ctx 构造时 indexer open 路径完成（Task 2 已加 schema_meta 表）

    let config = ServerConfig {
        bind_addr: cli.bind,
        bearer_token: SecretString::new(cli.token.into()),
        data_dir: cli.data_dir.clone(),
        root: cli.root.clone(),
        model_path: cli.model_path.clone(),
        log_level: tracing::level_filters::LevelFilter::INFO,
    };

    // 构造 ServerCtx —— 打开 indexer DB、加载 embedder、跑首次全量索引
    let ctx = build_runtime_ctx(config).await?;
    let ctx = Arc::new(ctx);

    let app = build_app(ctx.clone());
    lifecycle::serve(ctx, app).await
}

async fn build_runtime_ctx(config: ServerConfig) -> Result<ServerCtx> {
    // 1) 打开 MusicIndex / DocumentIndex at data_dir/index.db
    //    （会自动 ensure_schema_version 因为 Task 2 已埋 hook）
    // 2) 加载 embedder（model-runtime 的 llama-cpp-4 + embeddinggemma）
    // 3) RuntimeState{indexed_at, doc_count, reindex_in_flight: false}
    // 4) 首次全量索引 → 更新 indexed_at / doc_count
    todo!("subagent 按 packages/indexer + packages/model-runtime 当前 API 填充")
}
```

- [ ] **Step 10.4: 跑单测**

```bash
cargo test -p locifindd preflight::tests
cargo build -p locifindd
```

Expected: 4 个 preflight 单测 PASS、binary build OK。

- [ ] **Step 10.5: commit**

```bash
git add apps/daemon/
git commit -m "BETA-32 C3b：preflight + lifecycle + 全量索引启动"
```

---

## Task 11：C4 — apps/daemon 集成测试（DaemonHandle + MCP client + e2e 五条）

**Goal:** 用 stub embedder + 真 indexer + 真 axum 串端到端测试。

**Files:**
- Create: `apps/daemon/tests/common/mod.rs`（DaemonHandle、stub embedder 复用 locifind-server::test_support）
- Create: `apps/daemon/tests/e2e.rs`

- [ ] **Step 11.1: common/mod.rs**

```rust
// apps/daemon/tests/common/mod.rs
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;
use tempfile::TempDir;
use locifind_server::{ServerConfig, ServerCtx, app::build_app};
use secrecy::SecretString;

pub struct DaemonHandle {
    pub addr: std::net::SocketAddr,
    pub token: String,
    pub _root: TempDir,
    pub _data: TempDir,
    pub _task: tokio::task::JoinHandle<()>,
}

impl DaemonHandle {
    pub async fn spawn_with_fixtures(corpus: &[(&str, &str)]) -> Self {
        let root = tempfile::tempdir().unwrap();
        for (name, body) in corpus {
            std::fs::write(root.path().join(name), body).unwrap();
        }
        let data = tempfile::tempdir().unwrap();
        let token = "test-token-32-chars-minimum-length".to_string();

        let config = ServerConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            bearer_token: SecretString::new(token.clone().into()),
            data_dir: data.path().to_path_buf(),
            root: root.path().to_path_buf(),
            model_path: "/dev/null".into(),  // 测试用 stub embedder、不读真模型
            log_level: tracing::level_filters::LevelFilter::OFF,
        };

        // 构造 ctx — 用 locifind-server::test_support 的 stub embedder
        let ctx = Arc::new(build_test_ctx_with(config, root.path(), corpus));
        let app = build_app(ctx.clone());

        let listener = TcpListener::bind(ctx.config.bind_addr).await.unwrap();
        let addr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });

        // 等 1ms 让 server 起来
        tokio::time::sleep(Duration::from_millis(10)).await;

        Self { addr, token, _root: root, _data: data, _task: task }
    }
}

fn build_test_ctx_with(_config: ServerConfig, _root: &std::path::Path, _corpus: &[(&str, &str)]) -> ServerCtx {
    todo!("用 locifind-server::test_support::StubEmbedder + 内存或临时 SQLite + 实际 walker 跑一遍 corpus → 索引")
}
```

- [ ] **Step 11.2: e2e.rs 五条用例**

```rust
// apps/daemon/tests/e2e.rs
mod common;
use common::DaemonHandle;
use reqwest::Client;
use serde_json::json;

#[tokio::test]
async fn e2e_health_no_auth_required() {
    let d = DaemonHandle::spawn_with_fixtures(&[("a.txt", "hello world")]).await;
    let r = Client::new().get(format!("http://{}/health", d.addr)).send().await.unwrap();
    assert_eq!(r.status(), 200);
    let j: serde_json::Value = r.json().await.unwrap();
    assert_eq!(j["status"], "ok");
}

#[tokio::test]
async fn e2e_auth_401_wrong_token() {
    let d = DaemonHandle::spawn_with_fixtures(&[("a.txt", "x")]).await;
    let r = Client::new()
        .post(format!("http://{}/admin/reindex", d.addr))
        .bearer_auth("wrong")
        .send().await.unwrap();
    assert_eq!(r.status(), 401);
}

#[tokio::test]
async fn e2e_list_roots_after_indexing() {
    let d = DaemonHandle::spawn_with_fixtures(&[("doc.txt", "competitive analysis")]).await;
    // 这里用 rmcp client 调 list_roots tool —— 或者临时调一个 testing-only 直接 MCP request POST
    // subagent 按 rmcp 0.x client API 写
}

#[tokio::test]
async fn e2e_search_returns_results() {
    let d = DaemonHandle::spawn_with_fixtures(&[
        ("notes-2024-Q1.txt", "competitive analysis vs CompetitorX"),
        ("readme.md", "unrelated content"),
    ]).await;
    // MCP call search query="competitive analysis"
    // 断言至少 1 个 result、path 在 root 内
}

#[tokio::test]
async fn e2e_reindex_409_when_in_flight() {
    let d = DaemonHandle::spawn_with_fixtures(&[("a.txt", "x")]).await;
    // 调用两次并发 reindex —— 第二个应得 409
    // 注意 stub embedder 索引很快、要让第一次 hang —— 用 stub_progress.block_at_file
    // 实现详见 subagent
}
```

- [ ] **Step 11.3: 跑集成测试**

```bash
cargo test -p locifindd --test e2e
```

Expected: 5 测试全 PASS（其中 list_roots / search 用 rmcp client 调用、subagent 按 rmcp 当前 client API 接）。

- [ ] **Step 11.4: clippy + fmt**

```bash
cargo clippy -p locifindd --all-targets -- -D warnings
cargo fmt -p locifindd -- --check
```

Expected: 0 warning、fmt 净。

- [ ] **Step 11.5: commit**

```bash
git add apps/daemon/
git commit -m "BETA-32 C4：集成测试 5 条 e2e（DaemonHandle + MCP client）"
```

---

## Task 12：C5 — `packages/evals` 加 `--mode daemon` + top-K 集合等价闸门

**Goal:** evals harness 增加 daemon 模式，自起 daemon 子进程跑评测、比 desktop mode 输出 top-K path 集合等价。

**Files:**
- Modify: `packages/evals/src/bin/evals.rs`（CLI + mode 分发）
- Create: `packages/evals/src/runner_daemon.rs`（拉起 locifindd 子进程、走 MCP client 查询）
- Modify: `packages/evals/src/lib.rs` 或 reporter（top-K 集合等价比对器）

- [ ] **Step 12.1: evals CLI 加 mode 参数**

```bash
grep -n 'struct .* {' packages/evals/src/bin/evals.rs | head
```

按现有 CLI 风格加 `--mode <desktop|daemon>` + `--daemon-binary <PATH>` + `--root <PATH>` + `--model-path <PATH>`，desktop 为默认（行为不变）。

- [ ] **Step 12.2: runner_daemon.rs — spawn 子进程 + MCP client + 跑每条 case**

实现：
- `tokio::process::Command::new("./target/release/locifindd").args([...])` spawn 子进程
- bind 0.0.0.0:0、等 /health 200 后开跑
- 每条 case 调 MCP search、拿 results
- 跑完 SIGTERM kill daemon

- [ ] **Step 12.3: top-K 集合等价比对器**

新加比对函数（或在 reporter 加 mode）：
- 对每条 case 拿 desktop 模式与 daemon 模式 top-20 path
- 转 HashSet 对比；不等价 → 记录 diff
- 全部 case 跑完 → 汇总 mismatch 数；超过阈值（≤2 条允许）→ 红线 fail

- [ ] **Step 12.4: 加 daemon-mode 评测集成测试**

```bash
cargo build -p locifindd --release
cargo run -p locifind-evals --release -- run \
    --mode daemon \
    --daemon-binary ./target/release/locifindd \
    --root tests/fixtures/eval-corpus \
    --model-path <model> \
    --fixtures-version v05
```

Expected: 跑完输出 desktop vs daemon top-K 集合等价闸门结果。本 task 不必跑全量 v0.9 真模型 evals（成本太高、留 C6 cycle 出场标准红线 4）；本 task 验证 mode 接通即可。

- [ ] **Step 12.5: clippy + fmt + workspace check**

```bash
cargo clippy -p locifind-evals --all-targets -- -D warnings
cargo fmt -p locifind-evals -- --check
cargo check --workspace
```

- [ ] **Step 12.6: commit**

```bash
git add packages/evals/
git commit -m "BETA-32 C5：evals 加 --mode daemon + top-K 集合等价闸门"
```

---

## Task 13：C6 — 三平台 binary CI workflow

**Goal:** GitHub Actions workflow 产 Mac arm64 / Mac x86_64 / Windows x86_64 / Linux x86_64 共 4 个 binary。

**Files:**
- Create: `.github/workflows/release-daemon.yml`

- [ ] **Step 13.1: 写 workflow**

```yaml
# .github/workflows/release-daemon.yml
name: Release Daemon

on:
  push:
    tags: ['daemon-v*']
  workflow_dispatch:

jobs:
  build:
    name: build-${{ matrix.target }}
    runs-on: ${{ matrix.os }}
    strategy:
      matrix:
        include:
          - target: aarch64-apple-darwin
            os: macos-14
          - target: x86_64-apple-darwin
            os: macos-13
          - target: x86_64-pc-windows-msvc
            os: windows-latest
          - target: x86_64-unknown-linux-gnu
            os: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      - name: Build
        run: cargo build --release --bin locifindd --target ${{ matrix.target }}
      - name: Package
        shell: bash
        run: |
          mkdir -p dist
          if [[ "${{ matrix.os }}" == "windows-latest" ]]; then
            cp target/${{ matrix.target }}/release/locifindd.exe dist/locifindd-${{ matrix.target }}.exe
          else
            cp target/${{ matrix.target }}/release/locifindd dist/locifindd-${{ matrix.target }}
          fi
      - uses: actions/upload-artifact@v4
        with:
          name: locifindd-${{ matrix.target }}
          path: dist/*

  release:
    needs: build
    runs-on: ubuntu-latest
    if: startsWith(github.ref, 'refs/tags/daemon-v')
    steps:
      - uses: actions/download-artifact@v4
      - name: Create Release
        uses: softprops/action-gh-release@v2
        with:
          files: locifindd-*/*
          prerelease: true
          generate_release_notes: true
```

- [ ] **Step 13.2: 本机 dry-run workflow YAML（用 `actionlint` 或 `gh workflow view`）**

```bash
which actionlint && actionlint .github/workflows/release-daemon.yml || echo 'actionlint not installed, skip syntax check'
```

- [ ] **Step 13.3: 加 ROADMAP BETA-32 卡片**

按 ROADMAP §3.3 B6 之后同款格式（参考 BETA-31 卡片），加 BETA-32 卡片含字段 ID/标题/状态(in_progress)/依赖/估时/范围/验收/风险/follow-up。

- [ ] **Step 13.4: commit**

```bash
git add .github/workflows/release-daemon.yml ROADMAP.md
git commit -m "BETA-32 C6：三平台 binary CI workflow + ROADMAP 卡片"
```

---

## Task 14：C7 — 出场红线全套 + doc-sync + PR

**Goal:** 红线 1-9 全过、doc-sync 完整、PR 提交。

**Files:**
- Create: `apps/daemon/README.md`（部署 / 配置 / 故障排查）
- Modify: `docs/third-party-licenses.md`（加 rmcp / axum / secrecy / subtle / clap / tower / tower-http 等新依赖）
- Modify: `ROADMAP.md`（BETA-32 状态 done）
- Modify: `STATUS.md`（顶部加会话日志 + 当前 task 收尾）

- [ ] **Step 14.1: 红线 1-3（lint / test）**

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

Expected: fmt 净、0 warning、workspace 测试全过。

- [ ] **Step 14.2: 红线 4-5（locifind-server 单测 + apps/daemon e2e）**

```bash
cargo test -p locifind-server
cargo test -p locifindd --test e2e
```

Expected: 全 PASS。

- [ ] **Step 14.3: 红线 6（评测层 top-K 等价、用 stub embedder + 合成集 v05）**

```bash
cargo build -p locifindd --release
cargo run -p locifind-evals --release -- run \
    --mode daemon \
    --daemon-binary ./target/release/locifindd \
    --root packages/evals/fixtures/v05/corpus \
    --model-path <model>
# 与 desktop mode 同款 v05 对比
```

Expected: top-20 path 集合等价、允许 ≤2 条 noise。

- [ ] **Step 14.4: 红线 7（desktop build 不破）**

```bash
cd apps/desktop && npm run build && cd ../..
cargo check -p locifind-desktop  # 桌面 app 整体 check 不破
```

Expected: 全过、indexer trait 抽离不破桌面 app。

- [ ] **Step 14.5: 红线 8（manual install 烟雾测）**

```bash
LOCIFINDD_TOKEN="$(openssl rand -hex 16)" \
LOCIFINDD_MODEL_PATH="/path/to/embeddinggemma-300m-q8_0.gguf" \
./target/release/locifindd \
    --root /tmp/test-archive \
    --bind 127.0.0.1:8765 \
    --data-dir /tmp/locifindd-data \
    &
sleep 3
curl http://127.0.0.1:8765/health
# 期望 {"status":"ok","version":"..."}
kill %1
```

Expected: /health 返 200。

- [ ] **Step 14.6: 写 apps/daemon/README.md**

按 spec §5.4 + §8 出场标准写部署样板（launchd plist / systemd unit / Windows service NSSM 三套 + Claude Code settings.json 接入示例 + 5 个故障排查）。

- [ ] **Step 14.7: doc-sync — third-party-licenses + ROADMAP + STATUS**

按 BETA-31 同款节奏：
- `docs/third-party-licenses.md` 末尾加 BETA-32 引入的新依赖（rmcp / axum / tower / tower-http / clap / secrecy / subtle / toml / tracing-subscriber / chrono）
- `ROADMAP.md` BETA-32 卡片状态 → done
- `STATUS.md` 顶部追加 BETA-32 会话日志

- [ ] **Step 14.8: commit doc-sync**

```bash
git add apps/daemon/README.md docs/third-party-licenses.md ROADMAP.md STATUS.md
git commit -m "BETA-32 C7：doc-sync 红线全过"
```

- [ ] **Step 14.9: push + PR**

```bash
git push -u origin feat-beta-32-team-archive-mcp-daemon
gh pr create --title "BETA-32：团队归档 MCP daemon（headless + MCP over streamable-HTTP）" \
    --body "$(cat <<'EOF'
## Summary
- 新增 packages/locifind-server + apps/daemon（headless MCP daemon `locifindd`）
- 复用 packages/{intent-parser,harness,result-normalizer,ranker,indexer,model-runtime,search-backends/*} 作 library
- 仅 packages/indexer 加 IndexProgress callback trait + schema_meta 表（桌面 app 行为等价）
- packages/evals 加 --mode daemon + top-K 集合等价闸门
- 三平台 binary CI workflow（Mac arm/x86、Windows x86、Linux x86）
- spec：docs/superpowers/specs/2026-06-27-beta-32-team-archive-mcp-daemon-design.md
- plan：docs/superpowers/plans/2026-06-27-beta-32-team-archive-mcp-daemon.md

## Test plan
- [x] cargo fmt + clippy 0w + workspace test 全过
- [x] locifind-server 单测全过
- [x] apps/daemon e2e 集成测试 5 条全过
- [x] 评测层 top-K 集合等价闸门（stub embedder + v05 合成集）
- [x] desktop build 不破
- [x] manual install 烟雾测 /health 返 200
- [ ] **真机部署 DEFERRED 用户自验**（BETA-31 同款节奏）：管理员一台机器起 daemon、Claude Code 在另一台机器接 MCP server URL、跑通 5 example query
EOF
)"
```

Expected: PR 创建成功、URL 返回。

- [ ] **Step 14.10: 合 main + 占位符回填 + 分支清理**

```bash
# CI 通过后
gh pr merge --merge --delete-branch
# 回填 STATUS / ROADMAP 中的 PR #__ + merge commit hash
# 最后 commit 把回填落库 — 与 BETA-31 同款
```

---

## 收工 checklist（CONVENTIONS §3）

- [ ] STATUS.md 顶部追加 2026-06-27 会话日志
- [ ] ROADMAP.md BETA-32 卡片 → done
- [ ] 各 task 标 done
- [ ] 一次合并 commit 推 main
- [ ] 用户确认提交内容

---

## Self-Review notes

**Spec coverage check**：
- §2.2 不在范围内 12 条 → plan 全部不触
- §3 架构 → Task 3/4/5/6/7/8 实现 locifind-server 各组件、Task 9/10 实现 daemon binary
- §4 组件三件套 → Task 1（IndexProgress）+ Task 2（schema_meta）+ Task 3-8（locifind-server）+ Task 9-10（daemon）
- §5 数据流 → Task 10（启动 + 索引）+ Task 7（reindex 原子 swap）+ Task 5/6（查询）
- §6 错误处理 → Task 4（auth 401）+ Task 5（InvalidParams）+ Task 7（409）+ Task 10（fail-fast 6 条）
- §7 测试 三层金字塔 → Task 4-8 单元 + Task 11 集成 + Task 12 评测层
- §8.1 BETA-32 卡片 → Task 13
- §8.2 红线 1-9 → Task 14 step 1-5（red lines 6-9 中 fixture SHA / tsc / Tauri build 不涉及，spec §8.2 已说明）
- §8.3 估时 ~2-3w → 14 task 大约对齐

**Placeholder scan**：
- `todo!("subagent 按 ... 当前 API 填充")` 出现 4 处（Task 1 step 1.5 / Task 6 step 6.3 / Task 10 step 10.3 / Task 11 step 11.1） —— 这些是**显式 subagent 指令、不是 plan 内残留**，subagent 在执行时按当时真实 crate API 补全；本 plan 给出形状 + 期望行为已足够。
- 无 "TBD" / "TODO" / "implement later" 散落。

**Type consistency**：
- `IndexProgress` trait 在 Task 1 定义、Task 11 集成测试用同款
- `Tool` / `ToolError` 在 Task 5 定义、Task 6/8 沿用
- `ServerConfig` / `ServerCtx` / `RuntimeState` 在 Task 4 定义、Task 5/6/7/8/10/11 沿用
- `ReindexError::InFlight` 在 Task 7 定义、Task 11 集成测试 e2e_reindex_409 沿用
- `DaemonHandle` 在 Task 11 定义、五条 e2e 沿用
