# MVP-19+ Slice B — Tauri search 走 ToolRegistry Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `apps/desktop/src-tauri/src/search.rs` 从直调 `SpotlightBackend::new()` + `resolve_intent` 改为走 `main.rs` 已构建的 `ToolRegistry`，含 PolicyEngine gate + IntentRouter，保留 `Channel<SearchEvent>` 流式协议（加 `tool_id` 字段）。

**Architecture:** 引入 `SearchableTool: Tool` 子 trait 解决 `dyn Tool` 不能调 `search()` 的 dispatch 缺口；`ToolRegistry` 内部并行维护 `BTreeMap<String, Arc<dyn SearchableTool>>`；`SearchableToolHandle` newtype 让通用 `tools` 表与 search-typed `searchable` 表共享同一份 Arc，不重复堆分配。`IntentRouter` 加 `route_search` 返回 `Arc<dyn SearchableTool>`。Tauri search command 用 `tauri::State` 拿 `Arc<ToolRegistry>` + `Arc<PolicyEngine>`。

**Tech Stack:** Rust（locifind-harness / locifind-search-backend / locifind-intent-parser），Tauri 2，TypeScript（SearchView.tsx）。

**前置 spec:** [docs/superpowers/specs/2026-05-28-mvp-19-slice-b-tool-registry-wiring-design.md](../specs/2026-05-28-mvp-19-slice-b-tool-registry-wiring-design.md)

---

## 文件结构总览

| 文件 | 操作 | 用途 |
|---|---|---|
| `packages/harness/src/searchable_tool.rs` | 新建 | `SearchableTool: Tool` 子 trait + impl for `SearchTool<B>` + 单测 |
| `packages/harness/src/lib.rs` | 修改 | 加 `pub mod searchable_tool` + `pub use`；`ToolRegistry` 内部加 `searchable` 表 + 新方法 + `SearchableToolHandle` newtype + 新单测 |
| `packages/harness/src/intent_router.rs` | 修改 | 加 `route_search` 方法 + 单测 |
| `apps/desktop/src-tauri/Cargo.toml` | 修改 | `[target.'cfg(target_os = "windows")'.dependencies]` 加 windows-search + everything |
| `apps/desktop/src-tauri/src/main.rs` | 修改 | `register` → `register_search`；`Arc::new(registry)`；新增 `Arc<PolicyEngine>` state；Windows cfg 注册 WindowsSearch + Everything |
| `apps/desktop/src-tauri/src/search.rs` | 重写 | 删 SpotlightBackend 直调；从 `tauri::State` 拿 Arc；policy gate；IntentRouter::route_search；`SearchEvent::Started` 加 `tool_id` 字段 |
| `apps/desktop/src/SearchView.tsx` | 修改 | `SearchEvent.started` TS 类型加 `tool_id`；`IntentSummary` 加 `tool_id`；`IntentBadge` 显示 `via {tool_id}` |
| `STATUS.md` | 修改 | 当前阶段 + 会话日志（顶部追加）|
| `ROADMAP.md` | 修改 | MVP-19+ Slice B 标 done |

---

## Task 1: 新建 `SearchableTool` 子 trait + 单测

**Files:**
- Create: `packages/harness/src/searchable_tool.rs`
- Modify: `packages/harness/src/lib.rs:46-49` (注册新 mod)

**目标**：在 `Tool` trait 之外加 `SearchableTool: Tool` 子 trait 暴露 `search()`；`SearchTool<B>` blanket-impl 它（delegate 到 `backend.search()`）。这一步只动 harness，不改 ToolRegistry 也不改 search.rs。

- [ ] **Step 1: 新建 `searchable_tool.rs` 含 trait + impl + 1 单测**

```rust
//! `SearchableTool` 子 trait：在 `Tool` 之上加 `search()` dispatch，让 `dyn Tool`
//! 无法访问的 `backend.search()` 通过 `dyn SearchableTool` 可达。
//!
//! Tool trait 故意保留"最小公约数"设计（lib.rs §2 注释），不在其上加 invoke()。
//! 本 trait 专门承载 Search 类工具的流式调用接口；FileActionTool 走另外的子 trait
//! （未来引入）。

use std::sync::Arc;

use locifind_search_backend::{BackendSearchFuture, CancellationToken, SearchBackend, SearchIntent};

use crate::{SearchTool, Tool};

/// Search 类工具的 dispatch trait。
///
/// 任何实现 [`SearchBackend`] 的具体类型，通过 [`SearchTool`] 包装后即得到
/// [`SearchableTool`] 实现，可经 `Arc<dyn SearchableTool>` 跨边界 dispatch。
pub trait SearchableTool: Tool {
    /// 异步流式调用底层 backend 的 `search()`。
    ///
    /// 与 [`SearchBackend::search`] 同形状；语义由 backend 决定。
    fn search<'a>(
        &'a self,
        intent: &'a SearchIntent,
        cancel: CancellationToken,
    ) -> BackendSearchFuture<'a>;
}

impl<B: SearchBackend + 'static> SearchableTool for SearchTool<B> {
    fn search<'a>(
        &'a self,
        intent: &'a SearchIntent,
        cancel: CancellationToken,
    ) -> BackendSearchFuture<'a> {
        self.backend().search(intent, cancel)
    }
}

/// 便于 `Arc<dyn SearchableTool>` 在外部代码中跨 await 边界传递的类型别名。
pub type SharedSearchableTool = Arc<dyn SearchableTool>;

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::SupportedIntent;
    use futures_util::StreamExt;
    use locifind_search_backend::{
        backend_stream_from_results, BackendKind, FileSearch, ImplementationStatus, SchemaVersion,
    };

    #[derive(Debug)]
    struct FakeBackend;

    impl SearchBackend for FakeBackend {
        fn kind(&self) -> BackendKind {
            BackendKind::Spotlight
        }
        fn implementation_status(&self) -> ImplementationStatus {
            ImplementationStatus::Real
        }
        fn is_available(&self) -> bool {
            true
        }
        fn search<'a>(
            &'a self,
            _intent: &'a SearchIntent,
            cancel: CancellationToken,
        ) -> BackendSearchFuture<'a> {
            Box::pin(async move { Ok(backend_stream_from_results(Vec::new(), cancel)) })
        }
    }

    fn file_search() -> SearchIntent {
        SearchIntent::FileSearch(FileSearch {
            schema_version: SchemaVersion::V1,
            language: None,
            keywords: Some(vec!["budget".to_owned()]),
            extensions: None,
            file_type: None,
            location: None,
            modified_time: None,
            created_time: None,
            accessed_time: None,
            size: None,
            exclude_extensions: None,
            exclude_file_type: None,
            sort: None,
            limit: None,
        })
    }

    #[tokio::test]
    async fn searchable_tool_via_dyn_dispatch_returns_backend_stream() {
        let tool: Arc<dyn SearchableTool> = Arc::new(SearchTool::new(
            "search.fake",
            "Fake",
            FakeBackend,
            vec![SupportedIntent::FileSearch],
            "fake backend for dispatch test",
        ));

        let intent = file_search();
        let stream = tool
            .search(&intent, CancellationToken::new())
            .await
            .expect("search() should succeed");

        let results: Vec<_> = stream.collect().await;
        assert!(results.is_empty(), "empty stub backend yields zero results");
    }
}
```

- [ ] **Step 2: 在 `lib.rs` 注册新 mod**

修改 `packages/harness/src/lib.rs`：在现有 `pub mod fallback;` 一段之后追加：

```rust
pub mod searchable_tool;
pub use searchable_tool::{SearchableTool, SharedSearchableTool};
```

具体在 `pub mod fallback;` 后插入（约第 410 行附近）。

- [ ] **Step 3: 加 dev 依赖 `tokio` (test only) 与 `futures_util` 到 harness**

检查 `packages/harness/Cargo.toml`：若 `tokio` / `futures_util` 不在 `[dev-dependencies]`，则添加。

```bash
grep -E "^(tokio|futures_util)" packages/harness/Cargo.toml
```

预期：若有输出说明已存在；若无，手动加：

```toml
[dev-dependencies]
tokio = { version = "1", features = ["macros", "rt"] }
futures_util = "0.3"
```

- [ ] **Step 4: 跑单测**

```bash
cargo test -p locifind-harness searchable_tool::tests::searchable_tool_via_dyn_dispatch_returns_backend_stream -- --nocapture
```

预期：PASS。

- [ ] **Step 5: 跑全 harness test 确保未引入 regression**

```bash
cargo test -p locifind-harness
```

预期：全 pass。

- [ ] **Step 6: commit**

```bash
git add packages/harness/src/searchable_tool.rs packages/harness/src/lib.rs packages/harness/Cargo.toml
git commit -m "harness: 加 SearchableTool 子 trait + dyn dispatch 单测"
```

---

## Task 2: `ToolRegistry` 扩展 `searchable` 表 + `register_search` + 查询方法 + `SearchableToolHandle` newtype

**Files:**
- Modify: `packages/harness/src/lib.rs` (`ToolRegistry` 区块 §4 + tests §5)

**目标**：让 `ToolRegistry` 在内部并行维护 `searchable: BTreeMap<String, Arc<dyn SearchableTool>>`；通过 `register_search` 同时入两表；新增 `find_search_tool` / `available_search_tools_supporting`；用 `SearchableToolHandle(Arc<dyn SearchableTool>)` newtype 让通用 `tools` 表持有一份 Arc，避免双重堆分配。

- [ ] **Step 1: 先写 3 个 failing test**

在 `packages/harness/src/lib.rs` 末尾的 `#[cfg(test)] mod tests` 内追加三个 test（与现有测试同 module，复用 `make_search_tool` 不便所以独立写）：

```rust
    // ===== Task 2 新加测试：register_search + searchable 表 =====

    #[test]
    fn register_search_indexes_both_tables() {
        let mut registry = ToolRegistry::new();
        let tool = make_search_tool(
            "search.spotlight",
            BackendKind::Spotlight,
            ImplementationStatus::Real,
            true,
        );
        registry.register_search(tool).unwrap();

        // 通用 tools 表能找到（通过 SearchableToolHandle 转包）
        let via_tools = registry.find_by_id("search.spotlight").unwrap();
        assert_eq!(via_tools.id(), "search.spotlight");
        assert_eq!(via_tools.kind(), ToolKind::Search);
        assert_eq!(
            via_tools.capability().backend_kind,
            Some(BackendKind::Spotlight)
        );

        // search-typed 表也能找到
        let via_searchable = registry.find_search_tool("search.spotlight").unwrap();
        assert_eq!(via_searchable.id(), "search.spotlight");
    }

    #[test]
    fn register_search_rejects_duplicate_id() {
        let mut registry = ToolRegistry::new();
        registry
            .register_search(make_search_tool(
                "search.spotlight",
                BackendKind::Spotlight,
                ImplementationStatus::Real,
                true,
            ))
            .unwrap();
        let err = registry
            .register_search(make_search_tool(
                "search.spotlight",
                BackendKind::Spotlight,
                ImplementationStatus::Real,
                true,
            ))
            .unwrap_err();
        assert_eq!(err, DuplicateToolId("search.spotlight".to_owned()));
    }

    #[test]
    fn available_search_tools_supporting_filters_stub_and_unavailable() {
        let mut registry = ToolRegistry::new();
        // Real + available → 入选
        registry
            .register_search(make_search_tool(
                "search.spotlight",
                BackendKind::Spotlight,
                ImplementationStatus::Real,
                true,
            ))
            .unwrap();
        // Real + 不可用 → 不入选
        registry
            .register_search(make_search_tool(
                "search.windows",
                BackendKind::WindowsSearch,
                ImplementationStatus::Real,
                false,
            ))
            .unwrap();
        // Stub → 不入选
        registry
            .register_search(make_search_tool(
                "search.everything",
                BackendKind::Everything,
                ImplementationStatus::Stub,
                true,
            ))
            .unwrap();

        let available = registry.available_search_tools_supporting(SupportedIntent::FileSearch);
        assert_eq!(available.len(), 1);
        assert_eq!(available[0].id(), "search.spotlight");
    }
```

- [ ] **Step 2: 跑测试确认 fail（方法未定义）**

```bash
cargo test -p locifind-harness register_search 2>&1 | head -20
```

预期：编译失败 `no method named register_search` / `find_search_tool` / `available_search_tools_supporting`。

- [ ] **Step 3: 改 `ToolRegistry` struct 加 `searchable` 字段**

修改 `packages/harness/src/lib.rs` 第 298-301 行：

```rust
#[derive(Debug, Default)]
pub struct ToolRegistry {
    tools: BTreeMap<String, Box<dyn Tool>>,
    searchable: BTreeMap<String, Arc<dyn SearchableTool>>,
}
```

并改 `new()` (第 305-310 行):

```rust
    #[must_use]
    pub fn new() -> Self {
        Self {
            tools: BTreeMap::new(),
            searchable: BTreeMap::new(),
        }
    }
```

- [ ] **Step 4: 加 `SearchableToolHandle` newtype + `Tool` impl**

在 `ToolRegistry` 定义之前（约第 297 行前）插入：

```rust
/// 内部 newtype：让通用 `tools` 表持有 `Arc<dyn SearchableTool>` 的一份引用，
/// 复用同一份堆分配，避免 `register_search` 时同一个 backend 被装入两个独立 Box。
///
/// 仅 `ToolRegistry::register_search` 构造它；外部不可见。
pub(crate) struct SearchableToolHandle(Arc<dyn SearchableTool>);

impl fmt::Debug for SearchableToolHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("SearchableToolHandle")
            .field(&self.0.id())
            .finish()
    }
}

impl Tool for SearchableToolHandle {
    fn id(&self) -> &str {
        self.0.id()
    }
    fn name(&self) -> &str {
        self.0.name()
    }
    fn kind(&self) -> ToolKind {
        self.0.kind()
    }
    fn capability(&self) -> &ToolCapability {
        self.0.capability()
    }
    fn implementation_status(&self) -> ImplementationStatus {
        self.0.implementation_status()
    }
    fn is_available(&self) -> bool {
        self.0.is_available()
    }
}
```

- [ ] **Step 5: 给 `ToolRegistry` 加三个新方法**

在现有 `register_boxed` 之后（约第 325 行后）插入：

```rust
    /// Search 类工具的专用注册：同时入 `tools` 表（通过 [`SearchableToolHandle`]）
    /// 和 `searchable` 表，共享同一份 Arc。
    ///
    /// **Search 类工具必须用本方法注册**；若错调通用 [`Self::register`]，
    /// `searchable` 表不会被填充，[`Self::find_search_tool`] 与
    /// [`Self::available_search_tools_supporting`] 将无法找到该工具。
    pub fn register_search<B: SearchBackend + 'static>(
        &mut self,
        tool: SearchTool<B>,
    ) -> Result<(), DuplicateToolId> {
        let id = tool.id().to_owned();
        if self.tools.contains_key(&id) {
            return Err(DuplicateToolId(id));
        }
        let arc: Arc<dyn SearchableTool> = Arc::new(tool);
        self.searchable.insert(id.clone(), Arc::clone(&arc));
        self.tools.insert(id, Box::new(SearchableToolHandle(arc)));
        Ok(())
    }

    /// 按 id 查找 search-typed 工具；未注册或非 Search 类返回 `None`。
    #[must_use]
    pub fn find_search_tool(&self, id: &str) -> Option<Arc<dyn SearchableTool>> {
        self.searchable.get(id).cloned()
    }

    /// 生产链中支持指定 intent 且当前可用的 search-typed 工具子集。
    ///
    /// 三层过滤：`ImplementationStatus::Real` + `is_available()` + 支持该 intent。
    /// 排序按工具 id 升序（BTreeMap 自然有序），确保跨平台一致性。
    #[must_use]
    pub fn available_search_tools_supporting(
        &self,
        intent: SupportedIntent,
    ) -> Vec<Arc<dyn SearchableTool>> {
        self.searchable
            .values()
            .filter(|tool| tool.implementation_status() == ImplementationStatus::Real)
            .filter(|tool| tool.is_available())
            .filter(|tool| tool.capability().supported_intents.contains(&intent))
            .cloned()
            .collect()
    }
```

注意：还需在文件顶部 `use` 已经有 `SearchableTool`（从 Task 1 的 `pub use searchable_tool::{SearchableTool, ...}` 间接可达，但本 mod 内需显式或限定）。在 `lib.rs` `use` 区块（约第 32 行附近）加：

```rust
use crate::searchable_tool::SearchableTool;
```

（注意：在同一 lib.rs 里通过 `crate::` 取自己的子 mod 是合法的；这避免和 `pub use` 重导出的循环。）

- [ ] **Step 6: 同步更新 `register` doc comment，警示误用**

修改现有 `pub fn register<T: Tool + 'static>(...)` 的 doc：

```rust
    /// 注册一个工具。id 必须全局唯一。
    ///
    /// **Search 类工具请改用 [`Self::register_search`]**：本方法不会填充
    /// `searchable` 表，会导致 [`Self::find_search_tool`] / Intent Router 找不到。
    pub fn register<T: Tool + 'static>(&mut self, tool: T) -> Result<(), DuplicateToolId> {
        self.register_boxed(Box::new(tool))
    }
```

- [ ] **Step 7: 跑刚加的 3 测试 + 全 harness**

```bash
cargo test -p locifind-harness register_search
cargo test -p locifind-harness available_search_tools_supporting_filters_stub_and_unavailable
cargo test -p locifind-harness
```

预期：3 新测试 + 现有全部 pass。

- [ ] **Step 8: commit**

```bash
git add packages/harness/src/lib.rs
git commit -m "harness: ToolRegistry 加 register_search + searchable 表 + 3 单测"
```

---

## Task 3: `IntentRouter::route_search` + 单测

**Files:**
- Modify: `packages/harness/src/intent_router.rs`

**目标**：给 `IntentRouter` 加 `route_search(intent) -> Result<Arc<dyn SearchableTool>, RouteError>`，用于流式调用。复用现有 `route` 不动。

- [ ] **Step 1: 先写 3 个 failing test**

在 `packages/harness/src/intent_router.rs` 文件末尾 `#[cfg(test)] mod tests` 内追加：

```rust
    // ===== Task 3 新加测试：route_search =====

    #[test]
    fn route_search_returns_real_backend_arc() {
        let mut registry = ToolRegistry::new();
        registry
            .register_search(crate::SearchTool::new(
                "search.real",
                "Real",
                crate::tests_support::FakeSearchBackend::new(
                    BackendKind::Spotlight,
                    ImplementationStatus::Real,
                    true,
                ),
                vec![SupportedIntent::FileSearch],
                "real fake backend",
            ))
            .unwrap();

        let router = IntentRouter::new(&registry);
        let tool = router.route_search(&file_search_intent()).unwrap();
        assert_eq!(tool.id(), "search.real");
    }

    #[test]
    fn route_search_no_backend_when_none_available() {
        let registry = ToolRegistry::new();
        let router = IntentRouter::new(&registry);
        let err = router.route_search(&file_search_intent()).unwrap_err();
        assert_eq!(err, RouteError::NoBackend);
    }

    #[test]
    fn route_search_clarify_returns_clarify_not_routable() {
        let registry = ToolRegistry::new();
        let router = IntentRouter::new(&registry);
        let intent = SearchIntent::Clarify(Clarify {
            schema_version: SchemaVersion::V1,
            language: None,
            reason: ClarifyReason::AmbiguousAction,
            question: "要打开还是查找？".to_owned(),
            options: None,
        });
        let err = router.route_search(&intent).unwrap_err();
        assert_eq!(err, RouteError::ClarifyNotRoutable);
    }
```

注意 `crate::tests_support::FakeSearchBackend` 在现有 codebase 不存在 — 需要新建。改方案：直接复用 lib.rs tests mod 中的 `FakeBackend`，但它在 `#[cfg(test)] mod tests` 内不可跨文件访问。所以最简单方式：**在 intent_router.rs 的 tests mod 内本地定义一个迷你 SearchBackend + SearchTool 包装**。

替换上面 Step 1 的代码中 `crate::tests_support::FakeSearchBackend` 那段为：在 tests mod 顶部加：

```rust
    use futures_util::FutureExt;
    use locifind_search_backend::{
        backend_stream_from_results, BackendSearchFuture, CancellationToken, SearchBackend,
    };

    #[derive(Debug)]
    struct FakeRealBackend;

    impl SearchBackend for FakeRealBackend {
        fn kind(&self) -> BackendKind {
            BackendKind::Spotlight
        }
        fn implementation_status(&self) -> ImplementationStatus {
            ImplementationStatus::Real
        }
        fn is_available(&self) -> bool {
            true
        }
        fn search<'a>(
            &'a self,
            _intent: &'a SearchIntent,
            cancel: CancellationToken,
        ) -> BackendSearchFuture<'a> {
            async move { Ok(backend_stream_from_results(Vec::new(), cancel)) }.boxed()
        }
    }
```

并把 `route_search_returns_real_backend_arc` 改为：

```rust
    #[test]
    fn route_search_returns_real_backend_arc() {
        let mut registry = ToolRegistry::new();
        registry
            .register_search(crate::SearchTool::new(
                "search.real",
                "Real",
                FakeRealBackend,
                vec![SupportedIntent::FileSearch],
                "real fake backend",
            ))
            .unwrap();

        let router = IntentRouter::new(&registry);
        let tool = router.route_search(&file_search_intent()).unwrap();
        assert_eq!(tool.id(), "search.real");
    }
```

- [ ] **Step 2: 跑测试确认 fail**

```bash
cargo test -p locifind-harness route_search 2>&1 | head -20
```

预期：编译错误 `no method named route_search`。

- [ ] **Step 3: 实现 `route_search` 方法**

修改 `packages/harness/src/intent_router.rs` 在现有 `route` 方法后追加。先更新 `use`（顶部）：

```rust
use crate::{SearchableTool, SupportedIntent, Tool, ToolRegistry};
use std::sync::Arc;
```

然后在 `impl<'a> IntentRouter<'a>` 内（`route` 之后）加：

```rust
    /// 为 intent 选择第一个可用的 search-typed 工具，用于流式调用。
    ///
    /// 与 [`Self::route`] 的区别：返回 `Arc<dyn SearchableTool>` 而非 `&dyn Tool`，
    /// 调用方可直接 `.search().await` 流式拿结果，无需再做 dispatch。
    pub fn route_search(
        &self,
        intent: &locifind_search_backend::SearchIntent,
    ) -> Result<Arc<dyn SearchableTool>, RouteError> {
        let supported_intent = SupportedIntent::from_intent(intent);
        if supported_intent == SupportedIntent::Clarify {
            return Err(RouteError::ClarifyNotRoutable);
        }
        self.registry
            .available_search_tools_supporting(supported_intent)
            .into_iter()
            .next()
            .ok_or(RouteError::NoBackend)
    }
```

- [ ] **Step 4: 跑全 harness test**

```bash
cargo test -p locifind-harness
```

预期：全 pass（含 Task 1 + Task 2 + Task 3 新加 test）。

- [ ] **Step 5: commit**

```bash
git add packages/harness/src/intent_router.rs
git commit -m "harness: IntentRouter::route_search 返回 Arc<dyn SearchableTool>"
```

---

## Task 4: `main.rs` 升 `register_search` + Arc + PolicyEngine state + Windows backend cfg

**Files:**
- Modify: `apps/desktop/src-tauri/Cargo.toml`
- Modify: `apps/desktop/src-tauri/src/main.rs`

**目标**：(a) Windows cfg 加 WindowsSearch + Everything backend 依赖；(b) `build_registry` 用 `register_search`；(c) `Arc` 化 registry 并 `.manage()`；(d) `.manage(Arc::new(PolicyEngine::new()))`；(e) `build_registry_exposes_real_spotlight_on_macos` test 同步用 `find_search_tool`。

- [ ] **Step 1: 加 Windows backend 依赖**

修改 `apps/desktop/src-tauri/Cargo.toml`：在已有 `[target.'cfg(target_os = "macos")'.dependencies]` 段之后追加：

```toml
[target.'cfg(target_os = "windows")'.dependencies]
locifind-search-backend-windows-search = { path = "../../../packages/search-backends/windows-search" }
locifind-search-backend-everything = { path = "../../../packages/search-backends/everything" }
```

- [ ] **Step 2: 改 `main.rs` 顶部 use**

修改 `apps/desktop/src-tauri/src/main.rs:10-13`：

```rust
use std::sync::Arc;

use locifind_harness::{PolicyEngine, SearchTool, SupportedIntent, ToolRegistry};

#[cfg(target_os = "macos")]
use locifind_search_backend_spotlight::SpotlightBackend;

#[cfg(target_os = "windows")]
use locifind_search_backend_everything::EverythingBackend;
#[cfg(target_os = "windows")]
use locifind_search_backend_windows_search::WindowsSearchBackend;
```

- [ ] **Step 3: 改 `build_registry()` 用 `register_search` + Windows cfg 注册**

替换 `apps/desktop/src-tauri/src/main.rs:20-43` 整个 `build_registry`：

```rust
fn build_registry() -> ToolRegistry {
    let mut registry = ToolRegistry::new();

    #[cfg(target_os = "macos")]
    match SpotlightBackend::new() {
        Ok(backend) => {
            let tool = SearchTool::new(
                "search.spotlight",
                "Spotlight",
                backend,
                vec![SupportedIntent::FileSearch, SupportedIntent::MediaSearch],
                "macOS Spotlight 系统搜索（mdfind）",
            );
            if let Err(err) = registry.register_search(tool) {
                eprintln!("注册 SpotlightBackend 失败: {err}");
            }
        }
        Err(err) => {
            eprintln!("初始化 SpotlightBackend 失败: {err}");
        }
    }

    #[cfg(target_os = "windows")]
    {
        match WindowsSearchBackend::new() {
            Ok(backend) => {
                let tool = SearchTool::new(
                    "search.windows",
                    "Windows Search",
                    backend,
                    vec![SupportedIntent::FileSearch, SupportedIntent::MediaSearch],
                    "Windows Search 系统索引（OLE DB / SystemIndex SQL）",
                );
                if let Err(err) = registry.register_search(tool) {
                    eprintln!("注册 WindowsSearchBackend 失败: {err}");
                }
            }
            Err(err) => eprintln!("初始化 WindowsSearchBackend 失败: {err}"),
        }
        match EverythingBackend::new() {
            Ok(backend) => {
                let tool = SearchTool::new(
                    "search.everything",
                    "Everything",
                    backend,
                    vec![SupportedIntent::FileSearch, SupportedIntent::MediaSearch],
                    "Everything 加速搜索（es.exe CLI）",
                );
                if let Err(err) = registry.register_search(tool) {
                    eprintln!("注册 EverythingBackend 失败: {err}");
                }
            }
            Err(err) => eprintln!("初始化 EverythingBackend 失败: {err}"),
        }
    }

    registry
}
```

- [ ] **Step 4: 改 `main()` Arc 化 registry + manage PolicyEngine**

替换 `apps/desktop/src-tauri/src/main.rs:45-70` 的 `fn main()`：

```rust
fn main() {
    let registry = Arc::new(build_registry());
    let policy = Arc::new(PolicyEngine::new());

    tauri::Builder::default()
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .manage(registry)
        .manage(policy)
        .setup(|app| {
            shortcut::register_global_shortcut(&app.handle().clone())?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            echo,
            search::search,
            status::get_backend_status,
            settings::get_settings,
            settings::update_settings,
            permissions::check_macos_full_disk_access,
            permissions::open_macos_fda_settings,
            permissions::check_windows_search_indexed,
            permissions::open_windows_indexing_options,
            permissions::get_onboarding_state,
            permissions::complete_onboarding,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

- [ ] **Step 5: 改 test 用 `find_search_tool`**

替换 `apps/desktop/src-tauri/src/main.rs:73-101` 的 `#[cfg(test)] mod tests`：

```rust
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::panic)]

    use super::*;
    use locifind_harness::{CapabilityDiscovery, ImplementationStatus, ToolKind};

    #[cfg(target_os = "macos")]
    #[test]
    fn build_registry_exposes_real_spotlight_on_macos() {
        let registry = build_registry();

        // 通用 tools 表
        let tool = registry
            .find_by_id("search.spotlight")
            .expect("macOS 构建应注册 search.spotlight");
        assert_eq!(tool.kind(), ToolKind::Search);
        assert_eq!(tool.implementation_status(), ImplementationStatus::Real);
        assert!(
            tool.capability()
                .supported_intents
                .contains(&SupportedIntent::FileSearch),
            "SpotlightBackend 应支持 FileSearch"
        );

        // search-typed 表也应找到（验证 register_search 双入表）
        let search_tool = registry
            .find_search_tool("search.spotlight")
            .expect("register_search 应同时填充 searchable 表");
        assert_eq!(search_tool.id(), "search.spotlight");

        // available_search_tools_supporting 也应包含
        let available =
            registry.available_search_tools_supporting(SupportedIntent::FileSearch);
        assert!(
            available.iter().any(|t| t.id() == "search.spotlight"),
            "search.spotlight 应在 FileSearch available 列表中"
        );

        // 兼容现有 CapabilityDiscovery
        let summaries = CapabilityDiscovery::new(&registry).backend_summary();
        assert!(
            summaries.iter().any(|s| s.id == "search.spotlight"),
            "StatusIndicator 应能看到 search.spotlight"
        );
    }
}
```

- [ ] **Step 6: 编译验证（注意 search.rs 还没改，可能临时不通过 — 用 check 跳过 main 链接）**

```bash
cargo check -p locifind-harness
cargo check -p locifind-desktop 2>&1 | tail -20
```

预期：harness pass；desktop 可能因为 `search.rs` 还在直调 SpotlightBackend 而**仍能 check**（无破坏性 API 改动）。**不**跑 `cargo build`，留到 Task 5 search.rs 重写后一并验证。

- [ ] **Step 7: 不 commit，留到 Task 5 一并 commit**

main.rs 改动只有在 search.rs 重写后才能真正跑通；为避免中间状态 commit 后回滚困难，本 task 暂不 commit，与 Task 5 合并为一个 commit。

---

## Task 5: 重写 `search.rs` 走 ToolRegistry

**Files:**
- Rewrite: `apps/desktop/src-tauri/src/search.rs`

**目标**：从 `tauri::State` 拿 `Arc<ToolRegistry>` + `Arc<PolicyEngine>`；NL → intent → policy gate → IntentRouter::route_search → 流式调用；`SearchEvent::Started` 加 `tool_id` 字段；删除 `#[cfg(target_os = "macos")]` 在 command body 中的分支。

- [ ] **Step 1: 完整替换 `search.rs`**

完整替换 `apps/desktop/src-tauri/src/search.rs`：

```rust
//! Tauri search command —— UI 与 LociFind 核心管道之间的薄桥。
//!
//! MVP-19+ Slice B：走 [`ToolRegistry`] + [`IntentRouter`] + [`PolicyEngine`]，
//! 不再直调 backend。
//!
//! 协议（`SearchEvent`）：
//! - `Started`：先发，UI 立刻刷 intent / signals / fallback / tool_id 摘要
//! - `Result`：每条结果发一次
//! - `Complete`：结束，附 total / elapsed_ms
//! - `Error`：失败时发，前端切到错误态

use std::sync::Arc;
use std::time::Instant;

use futures::StreamExt;
use locifind_harness::{
    IntentRouter, PolicyAction, PolicyDecision, PolicyEngine, ToolRegistry,
};
use locifind_intent_parser::fallback::{resolve_intent, IntentSource};
use locifind_search_backend::{CancellationToken, SearchIntent};
use serde::Serialize;
use tauri::ipc::Channel;

/// 序列化给前端的搜索结果。
#[derive(Debug, Clone, Serialize)]
pub struct SearchResultJson {
    pub id: String,
    pub path: String,
    pub name: String,
    pub source: String,
    pub match_type: String,
    pub score: Option<f64>,
    pub modified_time: Option<String>,
    pub size_bytes: Option<u64>,
}

/// 增量流式事件。前端用 `Channel<SearchEvent>` 接收。
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum SearchEvent {
    /// 查询开始 — UI 立刻刷 intent / signals / tool_id。
    Started {
        intent_summary: String,
        fallback_used: bool,
        signals: Vec<&'static str>,
        /// 被路由到的工具 id（如 `"search.spotlight"`）。
        tool_id: String,
    },
    /// 单条结果。
    Result { item: SearchResultJson },
    /// 查询结束 — UI 切到 ready 态。
    Complete { total: usize, elapsed_ms: u64 },
    /// 查询失败 — UI 切到 error 态。
    Error { message: String },
}

/// 主搜索 command：query → SearchIntent → policy → IntentRouter → backend → 增量
/// emit 事件到前端。
///
/// 返回 `Result<(), String>` 仅表示"任务派发是否成功"；查询结果与失败均通过
/// `on_event` 流式投递（包括 `SearchEvent::Error`）。
#[tauri::command]
pub async fn search(
    query: String,
    on_event: Channel<SearchEvent>,
    registry: tauri::State<'_, Arc<ToolRegistry>>,
    policy: tauri::State<'_, Arc<PolicyEngine>>,
) -> Result<(), String> {
    let start = Instant::now();

    // State 是请求生命周期；克隆 Arc 出来跨 await 持有
    let registry = Arc::clone(&*registry);
    let policy = Arc::clone(&*policy);

    // 1) NL → intent
    let resolved = match resolve_intent(&query, None) {
        Ok(r) => r,
        Err(err) => {
            let _ = on_event.send(SearchEvent::Error {
                message: err.to_string(),
            });
            return Ok(());
        }
    };

    // 2) Policy gate
    let action = PolicyAction::from(&resolved.intent);
    match policy.evaluate(&action) {
        PolicyDecision::Allow => {}
        PolicyDecision::Deny { reason } => {
            let _ = on_event.send(SearchEvent::Error {
                message: format!("policy denied: {reason}"),
            });
            return Ok(());
        }
        PolicyDecision::RequireConfirmation => {
            // MVP search 流仅承载 read-only intent；FileAction 走另一条路径。
            // 防御性日志：若进到这里说明 intent 路由有 bug。
            let _ = on_event.send(SearchEvent::Error {
                message: "search 不应触发 RequireConfirmation".to_owned(),
            });
            return Ok(());
        }
    }

    // 3) Intent → SearchableTool
    let router = IntentRouter::new(&registry);
    let tool = match router.route_search(&resolved.intent) {
        Ok(t) => t,
        Err(err) => {
            let _ = on_event.send(SearchEvent::Error {
                message: err.to_string(),
            });
            return Ok(());
        }
    };

    // 4) Started 事件
    let _ = on_event.send(SearchEvent::Started {
        intent_summary: describe_intent(&resolved.intent),
        fallback_used: matches!(resolved.source, IntentSource::Model),
        signals: signals_to_labels(&resolved.signals),
        tool_id: tool.id().to_owned(),
    });

    // 5) 流式调用 backend
    let cancel = CancellationToken::new();
    let mut stream = match tool.search(&resolved.intent, cancel).await {
        Ok(s) => s,
        Err(err) => {
            let _ = on_event.send(SearchEvent::Error {
                message: err.to_string(),
            });
            return Ok(());
        }
    };

    let mut total = 0usize;
    while let Some(item) = stream.next().await {
        match item {
            Ok(result) => {
                let json = SearchResultJson {
                    id: result.id,
                    path: result.path.to_string_lossy().into_owned(),
                    name: result.name,
                    source: format!("{:?}", result.source).to_lowercase(),
                    match_type: format!("{:?}", result.match_type).to_lowercase(),
                    score: result.score,
                    modified_time: result.metadata.modified_time.map(|t| t.to_rfc3339()),
                    size_bytes: result.metadata.size_bytes,
                };
                total += 1;
                // channel 失败（前端断开）→ 中止流式
                if on_event.send(SearchEvent::Result { item: json }).is_err() {
                    break;
                }
            }
            Err(err) => {
                let _ = on_event.send(SearchEvent::Error {
                    message: err.to_string(),
                });
                return Ok(());
            }
        }
    }

    let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    let _ = on_event.send(SearchEvent::Complete { total, elapsed_ms });
    Ok(())
}

fn describe_intent(intent: &SearchIntent) -> String {
    match intent {
        SearchIntent::FileSearch(_) => "file_search".to_owned(),
        SearchIntent::MediaSearch(_) => "media_search".to_owned(),
        SearchIntent::FileAction(_) => "file_action".to_owned(),
        SearchIntent::Refine(_) => "refine".to_owned(),
        SearchIntent::Clarify(c) => format!("clarify: {}", c.question),
    }
}

fn signals_to_labels(
    signals: &locifind_intent_parser::signals::CandidateSignals,
) -> Vec<&'static str> {
    let mut out = Vec::new();
    if signals.time {
        out.push("time");
    }
    if signals.size {
        out.push("size");
    }
    if signals.sort {
        out.push("sort");
    }
    if signals.location {
        out.push("location");
    }
    if signals.action {
        out.push("action");
    }
    if signals.media {
        out.push("media");
    }
    out
}
```

- [ ] **Step 2: 编译 desktop crate**

```bash
cargo build -p locifind-desktop 2>&1 | tail -30
```

预期：编译成功。如果失败，常见 issue：
- `Arc` 未 use → 检查 `use std::sync::Arc;`
- `IntentSource` 未导入 → 检查 `use locifind_intent_parser::fallback::{resolve_intent, IntentSource};`
- harness 重新导出缺失 → 检查 `locifind_harness::{...}` 列表

- [ ] **Step 3: 跑 desktop test（含 Task 4 改的 test）**

```bash
cargo test -p locifind-desktop
```

预期：`build_registry_exposes_real_spotlight_on_macos` 在 macOS 上 pass。

- [ ] **Step 4: commit Task 4 + Task 5 合并**

```bash
git add apps/desktop/src-tauri/Cargo.toml \
        apps/desktop/src-tauri/src/main.rs \
        apps/desktop/src-tauri/src/search.rs
git commit -m "desktop: search command 走 ToolRegistry + PolicyEngine（MVP-19+ Slice B）"
```

---

## Task 6: 前端 `SearchView.tsx` 同步 `tool_id` 字段

**Files:**
- Modify: `apps/desktop/src/SearchView.tsx`

**目标**：TS 类型加 `tool_id`；`IntentSummary` 加 `tool_id`；`IntentBadge` 末尾显示 `via {tool_id}`。

- [ ] **Step 1: 修 `SearchEvent` TS type**

修改 `apps/desktop/src/SearchView.tsx:17-26`：

```typescript
// 与 src-tauri/src/search.rs::SearchEvent 对应（Tauri Channel 流式协议）
type SearchEvent =
  | {
      event: "started";
      intent_summary: string;
      fallback_used: boolean;
      signals: string[];
      tool_id: string;
    }
  | { event: "result"; item: SearchResultJson }
  | { event: "complete"; total: number; elapsed_ms: number }
  | { event: "error"; message: string };
```

- [ ] **Step 2: 修 `IntentSummary` interface 加 `tool_id`**

修改 `apps/desktop/src/SearchView.tsx:28-32`：

```typescript
interface IntentSummary {
  intent_summary: string;
  fallback_used: boolean;
  signals: string[];
  tool_id: string;
}
```

- [ ] **Step 3: 修 onmessage `started` 分支构造 IntentSummary 时填 tool_id**

修改 `apps/desktop/src/SearchView.tsx:73-86`（`case "started"` 分支）：

```typescript
        case "started": {
          const intent: IntentSummary = {
            intent_summary: msg.intent_summary,
            fallback_used: msg.fallback_used,
            signals: msg.signals,
            tool_id: msg.tool_id,
          };
          streamRef.current.intent = intent;
          setStatus({
            kind: "streaming",
            intent,
            results: streamRef.current.results,
          });
          break;
        }
```

- [ ] **Step 4: 修 `IntentBadge` 显示 tool_id**

修改 `apps/desktop/src/SearchView.tsx:254-281`（`IntentBadge` 函数）：

```typescript
function IntentBadge({
  intent,
  streaming,
  results,
  total,
  elapsed_ms,
}: IntentBadgeProps) {
  return (
    <div className="intent-summary">
      <span className="intent-label">intent</span>
      <code>{intent.intent_summary}</code>
      {intent.signals.length > 0 && (
        <>
          <span className="intent-label">signals</span>
          <code>{intent.signals.join(", ")}</code>
        </>
      )}
      {intent.fallback_used && (
        <span className="intent-fallback">model fallback</span>
      )}
      <span className="intent-tool">via {intent.tool_id}</span>
      <span className="intent-stats">
        {streaming
          ? `${results ?? 0} 条 · 流式中…`
          : `${total} 条 · ${elapsed_ms}ms`}
      </span>
    </div>
  );
}
```

- [ ] **Step 5: TS 编译检查**

```bash
cd apps/desktop && npm run build 2>&1 | tail -20
```

预期：build 成功。常见 issue：`tool_id` 在 IntentSummary 必填但其它地方没填 — 整文件 grep `IntentSummary` 确保所有构造点都填。

```bash
grep -n "IntentSummary" apps/desktop/src/SearchView.tsx
```

- [ ] **Step 6: commit**

```bash
git add apps/desktop/src/SearchView.tsx
git commit -m "desktop UI: SearchEvent.started 加 tool_id 字段 + IntentBadge 显示"
```

---

## Task 7: 全 CI 跑通

**Files:** 无（验证步骤）

**目标**：`bash scripts/ci.sh` 全过；任何 lint / fmt / clippy 失败当场修。

- [ ] **Step 1: 跑 ci.sh**

```bash
bash scripts/ci.sh 2>&1 | tail -40
```

预期：全 pass。

- [ ] **Step 2: 修任何失败**

常见 fix：
- `cargo fmt` 未跑 → `cargo fmt --all`
- clippy warning → 按提示修
- 缺 doc comment → 补
- unused import → 删

修完再跑一次。

- [ ] **Step 3: 跑 v0.5 evals 验证不可回归**

```bash
cargo run --release -p locifind-evals --bin evals -- --with-fallback --hybrid 2>&1 | tail -20
```

预期：pass=480 / partial=18 / fail=2 / 字段精确匹配 96.0%（与本会话开始时 byte-equal — parser/evals/LoRA 都没动）。

> 注：如果 LoRA 模型未本地下载或 daemon 未启动，可改跑 parser-only baseline：
> `cargo run --release -p locifind-evals --bin evals` 应该 pass=472 / partial=26 / fail=2 (parser-only baseline)。

- [ ] **Step 4: 若有 fmt/clippy 改动，独立 commit**

```bash
git status --short
git diff --stat
```

如有改动：

```bash
git add -u
git commit -m "ci: fmt / clippy 收尾"
```

---

## Task 8: macOS 手测 + STATUS/ROADMAP 收尾

**Files:**
- Modify: `STATUS.md`
- Modify: `ROADMAP.md`

**目标**：手测 macOS dev 实际跑通流式 + IntentBadge 显示 tool_id；STATUS 顶部加第 27 阶段会话日志；ROADMAP 记录 MVP-19+ Slice B done。

- [ ] **Step 1: 启 Tauri dev 模式**

```bash
cd apps/desktop && npm run tauri dev 2>&1 | tee /tmp/locifind-dev.log &
```

等 ~30s 编译完毕（首次更久）。

- [ ] **Step 2: 手测 4 个 case，每个都记录观察**

| Case | Query | 预期 |
|---|---|---|
| C1 | `find pdf` | IntentBadge 显示 `intent file_search` + `via search.spotlight` + 流式结果出现 |
| C2 | `find png in screenshots` | IntentBadge 显示 `intent media_search` + `via search.spotlight` + 流式结果 |
| C3 | `搜下` | IntentBadge 不出现；切到 error 态显示 `clarify intent is not routable`（Clarify 不可路由）|
| C4 | 极短查询如 `a` | 视 parser 处理而定 — 若产 FileSearch 走 spotlight；若产 Clarify 同 C3 |

观察重点：
- 结果是否**逐条** append（不是一次性闪出）
- IntentBadge 是否包含 `via search.spotlight`
- error 态文案是否清晰

- [ ] **Step 3: 关闭 dev 模式**

```bash
kill %1 2>/dev/null || true
```

- [ ] **Step 4: STATUS.md 顶部加会话日志**

在 `STATUS.md` 文件顶部 `## 会话日志` 区块之后、现有 `### 2026-05-28 — Claude Code (Opus 4.7) — parser screenshot extensions fix` 条目之前插入：

```markdown
### 2026-05-28 — Claude Code (Opus 4.7) — MVP-19+ Slice B：Tauri search 走 ToolRegistry（第 27 阶段，主会话 inline）

**关键决策**

- 承接第 26 阶段后 STATUS 锁定的 "下一会话 = search.rs → ToolRegistry wiring（MVP-19+ Slice B）"，本会话执行
- 完整走 superpowers 流程：brainstorming → writing-plans → executing-plans
- 4 个设计决策：
  - **Fallback chain 范围**：B1 IntentRouter 只选首位可用（不做 mid-stream retry，B 阶段或更高）
  - **Dispatch 设计**：A `SearchableTool: Tool` 子 trait + 并行 Arc 表 + `SearchableToolHandle` newtype 共享 Arc，保留 Tool trait 最小公约数原则
  - **Policy gate**：进入即 evaluate；Deny / RequireConfirmation 转 SearchEvent::Error
  - **Streaming**：保留 Channel<SearchEvent> v0.2 协议，加 `tool_id` 字段让 UI 显示 via {backend}
- spec [docs/superpowers/specs/2026-05-28-mvp-19-slice-b-tool-registry-wiring-design.md](./docs/superpowers/specs/2026-05-28-mvp-19-slice-b-tool-registry-wiring-design.md)（10 节）
- plan [docs/superpowers/plans/2026-05-28-mvp-19-slice-b-tool-registry-wiring.md](./docs/superpowers/plans/2026-05-28-mvp-19-slice-b-tool-registry-wiring.md)（8 task）

**产出**

- **N commit**（main 分支）：
  - `<sha>` harness: 加 SearchableTool 子 trait + dyn dispatch 单测
  - `<sha>` harness: ToolRegistry 加 register_search + searchable 表 + 3 单测
  - `<sha>` harness: IntentRouter::route_search 返回 Arc<dyn SearchableTool>
  - `<sha>` desktop: search command 走 ToolRegistry + PolicyEngine（MVP-19+ Slice B）
  - `<sha>` desktop UI: SearchEvent.started 加 tool_id 字段 + IntentBadge 显示
  - 本 commit：spec + plan + STATUS + ROADMAP
- `packages/harness/src/searchable_tool.rs` 新建（trait + impl + 1 单测）
- `packages/harness/src/lib.rs`：ToolRegistry 加 `searchable` 表 + `register_search` / `find_search_tool` / `available_search_tools_supporting` + `SearchableToolHandle` newtype + 3 单测
- `packages/harness/src/intent_router.rs`：加 `route_search` + 3 单测
- `apps/desktop/src-tauri/Cargo.toml`：加 Windows backend cfg 依赖
- `apps/desktop/src-tauri/src/main.rs`：register → register_search、Arc 化 registry、新增 PolicyEngine state、Windows cfg 注册分支
- `apps/desktop/src-tauri/src/search.rs`：完整重写 ~120 行，删 SpotlightBackend 直调 + cfg 分支，加 policy gate + route_search + tool_id
- `apps/desktop/src/SearchView.tsx`：SearchEvent.started + IntentSummary 加 tool_id；IntentBadge 显示 via {tool_id}
- intent-parser tests / harness tests / desktop tests / scripts/ci.sh 全 pass
- v0.5 evals --with-fallback --hybrid 重跑 byte-equal pass 480 / partial 18 / fail 2（评测路径未改动）

**手测观察**

C1 `find pdf` / C2 `find png in screenshots`：IntentBadge 显示 `via search.spotlight` + 结果流式
C3 `搜下`：error 态显示 clarify 不可路由

**未尽事宜 → 已转入下一步**

- MVP-19+ Slice B done；M5 中只剩 MVP-26 跨平台一致性（卡 Windows 真机）+ MVP-28 出场评测（依赖 MVP-26）
- 下一会话候选：Class A（BETA-09 (a) Windows / MVP-26 / 长周期事项）— 都卡用户外部条件
- 真 fallback chain mid-stream retry / Tracing 接入 / ContextMemory 多轮：留 backlog
```

- [ ] **Step 5: STATUS.md 「当前阶段」段更新**

定位 `> **下一会话已定**：**search.rs → ToolRegistry wiring（MVP-19+ Slice B）**。` 整段（约 STATUS.md:69 附近）改为：

```markdown
> **下一会话候选**：Class A 全部卡用户外部条件 — BETA-09 (a) Windows 真机加载 v1 GGUF / MVP-26 跨平台一致性 / 长周期事项（Apple Developer 注册、Windows 签名证书采购、locifind.ai 域名）。开场先确认用户具备哪条的启动条件。
```

并在「**Class B — 代码层 backlog**」中删除 `**【下一会话】search.rs → ToolRegistry wiring**` 那条（约 STATUS.md:81）；本 task 已 done。

- [ ] **Step 6: STATUS.md 「当前 Task」段更新**

定位 `## 当前 Task` 段（约 STATUS.md:53），改为：

```markdown
## 当前 Task

无进行中。本会话第 27 阶段：**MVP-19+ Slice B — Tauri search 走 ToolRegistry**（**done**，harness `SearchableTool` 子 trait + Registry 扩展 + IntentRouter::route_search + search.rs 重写 + PolicyEngine wire + SearchView.tsx tool_id 同步）。
```

- [ ] **Step 7: ROADMAP.md 同步 MVP-19 状态**

定位 ROADMAP.md MVP-19 行（M4 子阶段表），把状态备注从 `done（v0.1 collect 模式，event streaming 留升级）` 改为：

```markdown
| MVP-19 | 搜索框 UI + 流式结果列表 | done（Slice B：走 ToolRegistry + PolicyEngine + IntentRouter；含 tool_id 显示）| apps/desktop | MVP-18, MVP-07 | 3d |
```

- [ ] **Step 8: commit STATUS + ROADMAP + spec + plan**

```bash
git add STATUS.md ROADMAP.md \
        docs/superpowers/specs/2026-05-28-mvp-19-slice-b-tool-registry-wiring-design.md \
        docs/superpowers/plans/2026-05-28-mvp-19-slice-b-tool-registry-wiring.md
git commit -m "MVP-19+ Slice B 收工：spec + plan + STATUS + ROADMAP"
```

- [ ] **Step 9: 向用户确认提交**

```bash
git log --oneline -7
```

向用户报告本会话的 commit 序列（约 6 个 commit）、关键指标变化（应 byte-equal）、未尽事宜（Class A 仍卡外部条件）。

---

## Self-Review 后注

本 plan 写完后已 self-review：

**Spec 覆盖**：spec 的 §3.3 模块分工 6 个文件全部落到 plan 文件结构总览；§4 关键接口（SearchableTool / register_search / find_search_tool / route_search / SearchEvent.tool_id）全部对应到具体 Task；§5 测试策略 5.1 + 5.2 + 5.4 各项有对应 Task；5.3 手测在 Task 8 落地；§7 不可回归（v0.5 evals byte-equal）在 Task 7 Step 3 落地。

**类型一致性**：harness 中 `Arc<dyn SearchableTool>` 在 register_search / find_search_tool / available_search_tools_supporting / route_search 中类型完全一致；search.rs 使用 `Arc::clone(&*registry)` 显式解 State 引用后 clone Arc，符合 Tauri async command 规范；PolicyDecision 三变体（Allow / RequireConfirmation / Deny）与 policy.rs 第 80-92 行 source-of-truth 一致；PolicyEngine::evaluate 取 `&PolicyAction` 与 source 一致。

**无 placeholder**：每 Step 都有完整 code 或完整 command；无 TBD / 待补；不存在"和 Task N 类似"省略；commit message 都已写完整。

**Task 4/5 合并 commit**：Task 4 的 main.rs 改动单独 commit 会导致 search.rs 与 main.rs State 注册不匹配 → 编译失败的中间状态；故 Task 4 不 commit，与 Task 5 一并 commit 保证每个 commit 都能编译。
