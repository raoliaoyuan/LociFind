# Tracing/Hooks 接入 Tauri search command — 实施 plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 `Arc<Tracer>` 注入 Tauri `search` command,在 tool 层 3 个时点（call / result / error）触发 Tracer 事件,环境变量 `LOCIFIND_TRACE` 控制是否写 JSONL 文件,默认 0 hook 无开销。

**Architecture:** main.rs `build_tracer()` 读 env 构造 `Arc<Tracer>` 注入 Tauri State；search.rs 抽出 `search_impl` inner async fn 不依赖 Tauri State,#[tauri::command] 变 thin wrapper；3 trace 点（route_search 成功后 / Complete 前 / open+mid-stream err）；pre-tool 失败（intent/policy/router）沿用 `eprintln!` 不进 Tracer。

**Tech Stack:** Rust（已用：tauri 2.11、tokio、futures、`locifind-harness::{Tracer, ToolCallEvent, ToolResultEvent, ToolErrorEvent, JsonLinesHook, TracingHook, SupportedIntent}`、`SupportedIntent::from_intent` 已复用）；新增 std::fs::OpenOptions + std::env；无新 Cargo dep。

**Spec:** [docs/superpowers/specs/2026-05-28-tracing-hooks-search-wiring-design.md](../specs/2026-05-28-tracing-hooks-search-wiring-design.md)

---

## File Structure

| 文件 | 责任 | 改动类型 |
|---|---|---|
| `apps/desktop/src-tauri/src/main.rs` | `build_tracer()` 函数 + `.manage(build_tracer())` 注入 + 3 个 build_tracer 测试 | modify |
| `apps/desktop/src-tauri/src/search.rs` | 抽 `search_impl` inner fn + thin `#[tauri::command] search` wrapper + tracer State 参数 + 3 trace 点 + `search_error_kind` helper + MockHook 集成测试 + pre-tool 失败 `eprintln!` | modify |

无新文件。Cargo.toml 无新 dep。

## Self-check 前置确认（写实施前已验证）

1. ✅ `SupportedIntent::from_intent(&SearchIntent) -> Self` 已存在于 `packages/harness/src/lib.rs:91`,**直接复用**,不私写 helper
2. ✅ `tauri::ipc::Channel::new<F>(handler) -> Channel<TSend>` 是公开 API,可用闭包+Arc<Mutex<Vec>> 捕获前端事件做单测
3. ✅ 项目无 `serial_test` crate;env 测试用 module-level `std::sync::Mutex` 串行化
4. ✅ harness Tracer 的 `Debug` impl 已输出 `"hook_count: N"`,测试用 `format!("{:?}", *tracer).contains("hook_count: 0")` 验证

---

### Task 1: build_tracer 最小骨架 (default-noop only)

**Files:**
- Modify: `apps/desktop/src-tauri/src/main.rs`（新增 `build_tracer` 函数 + `.manage` 注入 + 1 个测试）

- [ ] **Step 1: 写失败测试 `build_tracer_default_is_noop`**

在 `main.rs` 的 `#[cfg(test)] mod tests` 块尾追加:

```rust
    #[test]
    fn build_tracer_default_is_noop() {
        // 防止其他 test set 了 LOCIFIND_TRACE
        let _guard = TRACER_ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        // SAFETY: 测试串行化由 TRACER_ENV_MUTEX 保证
        unsafe { std::env::remove_var("LOCIFIND_TRACE") };
        let tracer = build_tracer();
        let debug = format!("{tracer:?}");
        assert!(
            debug.contains("hook_count: 0"),
            "默认应 0 hook, 实得: {debug}"
        );
    }
```

并在 `mod tests` 顶部加 mutex（用 OnceLock 而非 Lazy 避免新 dep）:

```rust
    use std::sync::{Mutex, OnceLock};

    // env 变量是进程级状态; 串行化所有读写 LOCIFIND_TRACE 的测试
    static TRACER_ENV_MUTEX_INNER: OnceLock<Mutex<()>> = OnceLock::new();
    #[allow(non_upper_case_globals)]
    fn TRACER_ENV_MUTEX() -> &'static Mutex<()> {
        TRACER_ENV_MUTEX_INNER.get_or_init(|| Mutex::new(()))
    }
```

把 test 里的 `TRACER_ENV_MUTEX.lock()` 改成 `TRACER_ENV_MUTEX().lock()`(函数调用)。

- [ ] **Step 2: 跑测试确认 fail**

```bash
cd /Users/alice/Work/LocalFind && cargo test -p locifind-desktop build_tracer_default_is_noop 2>&1 | tail -20
```
Expected: 编译错 `cannot find function build_tracer in this scope`

- [ ] **Step 3: 实现最小 build_tracer (仅 default 分支)**

在 `main.rs` 顶部 use 加:

```rust
use locifind_harness::Tracer;
```

在 `build_registry()` 函数之后追加:

```rust
/// 构造 Tracer。环境变量 LOCIFIND_TRACE 控制 hook:
/// - 未设/空 → 0 hook（默认无开销）
/// - 设非空 path → 尝试 OpenOptions append 打开,成功挂 JsonLinesHook,失败 fallback noop + stderr warn
fn build_tracer() -> Arc<Tracer> {
    Arc::new(Tracer::with_hooks(vec![]))
}
```

注意:**这一步先不读 env**,只让默认分支编译通过,Task 2 再加完整逻辑。

- [ ] **Step 4: main() 注入 .manage(build_tracer())**

在 `main()` 内 `let policy = Arc::new(PolicyEngine::new());` 之后加:

```rust
    let tracer = build_tracer();
```

并在 Tauri builder 的 `.manage(policy)` 之后加:

```rust
        .manage(tracer)
```

- [ ] **Step 5: 跑测试确认 pass + 其它 test 不破**

```bash
cd /Users/alice/Work/LocalFind && cargo test -p locifind-desktop 2>&1 | tail -30
```
Expected: `build_tracer_default_is_noop` PASS;`build_registry_exposes_real_spotlight_on_macos` 仍 PASS（macOS）。

- [ ] **Step 6: Commit**

```bash
cd /Users/alice/Work/LocalFind && git add apps/desktop/src-tauri/src/main.rs && git commit -m "desktop: 加 build_tracer 骨架 + .manage 注入(默认 noop)"
```

---

### Task 2: build_tracer env 完整支持 (valid path + invalid fallback)

**Files:**
- Modify: `apps/desktop/src-tauri/src/main.rs`（扩 `build_tracer` + 2 个测试）

- [ ] **Step 1: 写两个失败测试**

在 `mod tests` 内追加:

```rust
    #[test]
    fn build_tracer_with_valid_env_attaches_jsonlines() {
        let _guard = TRACER_ENV_MUTEX().lock().unwrap_or_else(|e| e.into_inner());
        let tmpdir = std::env::temp_dir();
        let path = tmpdir.join(format!("locifind-trace-test-{}.jsonl", std::process::id()));
        let _ = std::fs::remove_file(&path);
        // SAFETY: 测试串行化由 TRACER_ENV_MUTEX 保证
        unsafe { std::env::set_var("LOCIFIND_TRACE", &path) };

        let tracer = build_tracer();
        let debug = format!("{tracer:?}");
        assert!(
            debug.contains("hook_count: 1"),
            "valid path 应 1 hook, 实得: {debug}"
        );
        assert!(path.exists(), "build_tracer 应已创建文件 {}", path.display());

        // 触发一条 event 验证文件真的可写
        use locifind_harness::{SupportedIntent, ToolCallEvent, ToolKind};
        tracer.on_tool_call(&ToolCallEvent {
            tool_id: "test.tool".into(),
            tool_kind: ToolKind::Search,
            intent_variant: SupportedIntent::FileSearch,
        });
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("test.tool"), "JSONL 应含 tool_id, 实得: {content}");

        // SAFETY: 同上
        unsafe { std::env::remove_var("LOCIFIND_TRACE") };
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn build_tracer_with_invalid_path_falls_back() {
        let _guard = TRACER_ENV_MUTEX().lock().unwrap_or_else(|e| e.into_inner());
        // /proc 在 macOS 不存在; /dev/null/foo 不是目录无法 create
        // SAFETY: 测试串行化由 TRACER_ENV_MUTEX 保证
        unsafe { std::env::set_var("LOCIFIND_TRACE", "/dev/null/不可创建/x.jsonl") };

        let tracer = build_tracer();
        let debug = format!("{tracer:?}");
        assert!(
            debug.contains("hook_count: 0"),
            "invalid path 应 fallback 0 hook, 实得: {debug}"
        );

        // SAFETY: 同上
        unsafe { std::env::remove_var("LOCIFIND_TRACE") };
    }
```

- [ ] **Step 2: 跑测试确认 fail**

```bash
cd /Users/alice/Work/LocalFind && cargo test -p locifind-desktop build_tracer 2>&1 | tail -25
```
Expected: `default_is_noop` PASS;另两个 FAIL（hook_count 仍 0 / 文件不存在）。

- [ ] **Step 3: 实现 build_tracer env 完整逻辑**

把 `build_tracer` 替换为:

```rust
fn build_tracer() -> Arc<Tracer> {
    use locifind_harness::{JsonLinesHook, TracingHook};
    use std::fs::OpenOptions;

    let path = std::env::var("LOCIFIND_TRACE").ok().filter(|s| !s.is_empty());
    let hooks: Vec<Box<dyn TracingHook>> = match path {
        None => vec![],
        Some(p) => match OpenOptions::new().create(true).append(true).open(&p) {
            Ok(file) => vec![Box::new(JsonLinesHook::new(file))],
            Err(err) => {
                eprintln!("LOCIFIND_TRACE 打开 {p} 失败 ({err}), tracing 禁用");
                vec![]
            }
        },
    };
    Arc::new(Tracer::with_hooks(hooks))
}
```

- [ ] **Step 4: 跑测试确认 3 个都 pass**

```bash
cd /Users/alice/Work/LocalFind && cargo test -p locifind-desktop build_tracer 2>&1 | tail -15
```
Expected: 3 tests PASS。

- [ ] **Step 5: clippy + fmt**

```bash
cd /Users/alice/Work/LocalFind && cargo fmt --all && cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -20
```
Expected: no warnings。

- [ ] **Step 6: Commit**

```bash
cd /Users/alice/Work/LocalFind && git add apps/desktop/src-tauri/src/main.rs && git commit -m "desktop: build_tracer 完整 env 支持 + 2 测试(valid/invalid fallback)"
```

---

### Task 3: search_error_kind helper + 单测

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`（新增 private fn + `#[cfg(test)] mod tests`）

- [ ] **Step 1: 写失败测试 `search_error_kind_maps_all_variants`**

在 `search.rs` 文件尾追加:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use locifind_search_backend::SearchError;
    use std::path::PathBuf;

    #[test]
    fn search_error_kind_maps_all_variants() {
        assert_eq!(
            search_error_kind(&SearchError::BackendUnavailable { reason: "x".into() }),
            "BackendUnavailable"
        );
        assert_eq!(
            search_error_kind(&SearchError::PermissionDenied { path: Some(PathBuf::from("/x")) }),
            "PermissionDenied"
        );
        assert_eq!(
            search_error_kind(&SearchError::InvalidIntent { detail: "x".into() }),
            "InvalidIntent"
        );
        assert_eq!(
            search_error_kind(&SearchError::UnsupportedIntent { detail: "x".into() }),
            "UnsupportedIntent"
        );
        assert_eq!(
            search_error_kind(&SearchError::Timeout { elapsed_ms: 1000 }),
            "Timeout"
        );
        assert_eq!(
            search_error_kind(&SearchError::Io { detail: "x".into() }),
            "Io"
        );
    }
}
```

- [ ] **Step 2: 跑测试确认 fail**

```bash
cd /Users/alice/Work/LocalFind && cargo test -p locifind-desktop search_error_kind 2>&1 | tail -10
```
Expected: 编译错 `cannot find function search_error_kind`。

- [ ] **Step 3: 实现 search_error_kind**

在 `search.rs` 文件内 `signals_to_labels` 函数之后,`#[cfg(test)]` 之前追加:

```rust
/// 返回 SearchError variant 名,不含 detail（避免泄路径）。供 trace 用。
fn search_error_kind(err: &locifind_search_backend::SearchError) -> &'static str {
    use locifind_search_backend::SearchError;
    match err {
        SearchError::BackendUnavailable { .. } => "BackendUnavailable",
        SearchError::PermissionDenied { .. } => "PermissionDenied",
        SearchError::InvalidIntent { .. } => "InvalidIntent",
        SearchError::UnsupportedIntent { .. } => "UnsupportedIntent",
        SearchError::Timeout { .. } => "Timeout",
        SearchError::Io { .. } => "Io",
    }
}
```

- [ ] **Step 4: 跑测试确认 pass**

```bash
cd /Users/alice/Work/LocalFind && cargo test -p locifind-desktop search_error_kind 2>&1 | tail -10
```
Expected: PASS。

- [ ] **Step 5: Commit**

```bash
cd /Users/alice/Work/LocalFind && git add apps/desktop/src-tauri/src/search.rs && git commit -m "desktop: 加 search_error_kind helper(SearchError→variant 名)"
```

---

### Task 4: 抽 search_impl inner fn + 注入 tracer State 参数

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`（重构 search command 为 thin wrapper + inner fn）

> **重要**:本 task 仅做重构,**不加 trace 点**(Task 5 才加)。重构后所有现有行为必须 byte-equal。

- [ ] **Step 1: 重构 search 为 thin wrapper + search_impl inner fn**

把 `search.rs` 的 `pub async fn search(...)` 替换为以下两个函数:

```rust
/// 主搜索 command：thin wrapper,解 State 后委托 [`search_impl`]。
#[tauri::command]
pub async fn search(
    query: String,
    on_event: Channel<SearchEvent>,
    registry: tauri::State<'_, Arc<ToolRegistry>>,
    policy: tauri::State<'_, Arc<PolicyEngine>>,
    tracer: tauri::State<'_, Arc<locifind_harness::Tracer>>,
) -> Result<(), String> {
    search_impl(
        query,
        on_event,
        Arc::clone(&*registry),
        Arc::clone(&*policy),
        Arc::clone(&*tracer),
    )
    .await
}

/// search command 主体。不依赖 [`tauri::State`],可被单测注入 mock 调用。
///
/// 返回 `Result<(), String>` 仅表示"任务派发是否成功"；查询结果与失败均通过
/// `on_event` 流式投递(包括 `SearchEvent::Error`)。
pub(crate) async fn search_impl(
    query: String,
    on_event: Channel<SearchEvent>,
    registry: Arc<ToolRegistry>,
    policy: Arc<PolicyEngine>,
    _tracer: Arc<locifind_harness::Tracer>, // Task 5 使用,本 task 仅占位
) -> Result<(), String> {
    let start = Instant::now();

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
```

- [ ] **Step 2: 编译通过**

```bash
cd /Users/alice/Work/LocalFind && cargo check -p locifind-desktop 2>&1 | tail -20
```
Expected: no errors。

- [ ] **Step 3: 现有 test 仍过**

```bash
cd /Users/alice/Work/LocalFind && cargo test -p locifind-desktop 2>&1 | tail -20
```
Expected: 所有 test PASS（含 build_tracer 3 + search_error_kind 1 + build_registry 1 = 5 个）。

- [ ] **Step 4: clippy + fmt**

```bash
cd /Users/alice/Work/LocalFind && cargo fmt --all && cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -20
```
Expected: no warnings。

- [ ] **Step 5: Commit**

```bash
cd /Users/alice/Work/LocalFind && git add apps/desktop/src-tauri/src/search.rs && git commit -m "desktop: search 抽 inner search_impl + 注入 tracer State 参数(纯重构,无 trace 点)"
```

---

### Task 5: 在 search_impl 加 3 个 trace 点

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`（仅 `search_impl` 函数体内）

> **重要**:本 task **仅加 trace 调用**,不动控制流。`_tracer` → `tracer`(去 underscore)。

- [ ] **Step 1: 加 Trace A (tool_call)**

在 `search_impl` 内 `// 4) Started 事件` **之前**,`tool` 已绑定之后,插入:

```rust
    // Trace A: tool 即将被调用
    tracer.on_tool_call(&locifind_harness::ToolCallEvent {
        tool_id: tool.id().to_owned(),
        tool_kind: locifind_harness::ToolKind::Search,
        intent_variant: locifind_harness::SupportedIntent::from_intent(&resolved.intent),
    });
    let tool_start = Instant::now();
```

同时把签名里 `_tracer: Arc<locifind_harness::Tracer>` 改为 `tracer: Arc<locifind_harness::Tracer>`。

- [ ] **Step 2: 加 Trace C (open err)**

把 `let mut stream = match tool.search(...).await { Ok(s) => s, Err(err) => { ... } };` 的 `Err` 分支改为:

```rust
        Err(err) => {
            tracer.on_error(&locifind_harness::ToolErrorEvent {
                tool_id: tool.id().to_owned(),
                duration: tool_start.elapsed(),
                error_type: search_error_kind(&err).to_owned(),
            });
            let _ = on_event.send(SearchEvent::Error {
                message: err.to_string(),
            });
            return Ok(());
        }
```

- [ ] **Step 3: 加 Trace C (mid-stream err)**

把 `while let Some(item) = stream.next().await` 内 `Err(err) => { ... }` 分支改为:

```rust
            Err(err) => {
                tracer.on_error(&locifind_harness::ToolErrorEvent {
                    tool_id: tool.id().to_owned(),
                    duration: tool_start.elapsed(),
                    error_type: search_error_kind(&err).to_owned(),
                });
                let _ = on_event.send(SearchEvent::Error {
                    message: err.to_string(),
                });
                return Ok(());
            }
```

- [ ] **Step 4: 加 Trace B (tool_result before Complete)**

在 `let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);` **之前**,插入:

```rust
    tracer.on_tool_result(&locifind_harness::ToolResultEvent {
        tool_id: tool.id().to_owned(),
        duration: tool_start.elapsed(),
        result_count: total,
    });
```

- [ ] **Step 5: 编译 + 现有测试不破**

```bash
cd /Users/alice/Work/LocalFind && cargo test -p locifind-desktop 2>&1 | tail -20
```
Expected: 5 个 test 仍 PASS。

- [ ] **Step 6: clippy + fmt**

```bash
cd /Users/alice/Work/LocalFind && cargo fmt --all && cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -20
```
Expected: no warnings。

- [ ] **Step 7: Commit**

```bash
cd /Users/alice/Work/LocalFind && git add apps/desktop/src-tauri/src/search.rs && git commit -m "desktop: search_impl 加 3 个 trace 点(call/result/error)"
```

---

### Task 6: search_impl 集成测试 (MockHook + fake tools)

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`（扩 `#[cfg(test)] mod tests` 块）

- [ ] **Step 1: 写 MockHook + fake backend + 4 个失败测试**

把 `#[cfg(test)] mod tests` 整块替换为:

```rust
#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;
    use futures::stream;
    use locifind_harness::{
        BackendKind, Capability, ImplementationStatus, MatchType, PolicyEngine, SearchResult,
        SearchResultMetadata, SearchTool, SearchableTool, SupportedIntent, ToolCallEvent,
        ToolErrorEvent, ToolKind, ToolRegistry, Tracer, TracingHook,
    };
    use locifind_search_backend::{
        BackendStream, CancellationToken, FileSearchIntent, SearchBackend, SearchError,
        SearchIntent,
    };
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use tauri::ipc::{Channel, InvokeResponseBody};

    /// 记录 trace 事件序列(用法同 tracing.rs 的 MockHook)。
    #[derive(Default)]
    struct MockHook {
        calls: Arc<Mutex<Vec<String>>>,
    }
    impl MockHook {
        fn new() -> (Self, Arc<Mutex<Vec<String>>>) {
            let calls = Arc::new(Mutex::new(Vec::new()));
            let me = Self {
                calls: Arc::clone(&calls),
            };
            (me, calls)
        }
    }
    impl TracingHook for MockHook {
        fn on_tool_call(&self, e: &ToolCallEvent) {
            self.calls
                .lock()
                .unwrap()
                .push(format!("call:{}", e.tool_id));
        }
        fn on_tool_result(&self, e: &locifind_harness::ToolResultEvent) {
            self.calls
                .lock()
                .unwrap()
                .push(format!("result:{}:{}", e.tool_id, e.result_count));
        }
        fn on_error(&self, e: &ToolErrorEvent) {
            self.calls
                .lock()
                .unwrap()
                .push(format!("error:{}:{}", e.tool_id, e.error_type));
        }
    }

    /// 捕获前端 channel 事件。
    fn capture_channel() -> (Channel<SearchEvent>, Arc<Mutex<Vec<String>>>) {
        let captured = Arc::new(Mutex::new(Vec::<String>::new()));
        let captured_clone = Arc::clone(&captured);
        let ch = Channel::new(move |body| {
            if let InvokeResponseBody::Json(s) = body {
                captured_clone.lock().unwrap().push(s);
            }
            Ok(())
        });
        (ch, captured)
    }

    /// 返回 N 条 fake SearchResult 的 backend。
    struct FakeOkBackend(usize);
    impl SearchBackend for FakeOkBackend {
        fn kind(&self) -> BackendKind {
            BackendKind::Spotlight
        }
        fn implementation_status(&self) -> ImplementationStatus {
            ImplementationStatus::Real
        }
        fn search(
            &self,
            _intent: &SearchIntent,
            _cancel: CancellationToken,
        ) -> locifind_search_backend::BackendSearchFuture<'_> {
            let n = self.0;
            Box::pin(async move {
                let items: Vec<Result<SearchResult, SearchError>> = (0..n)
                    .map(|i| {
                        Ok(SearchResult {
                            id: format!("id-{i}"),
                            path: PathBuf::from(format!("/tmp/f{i}")),
                            name: format!("f{i}"),
                            source: BackendKind::Spotlight,
                            match_type: MatchType::Filename,
                            score: None,
                            metadata: SearchResultMetadata::default(),
                        })
                    })
                    .collect();
                Ok(Box::pin(stream::iter(items)) as BackendStream)
            })
        }
    }

    /// 立刻 open err 的 backend。
    struct FakeOpenErrBackend;
    impl SearchBackend for FakeOpenErrBackend {
        fn kind(&self) -> BackendKind {
            BackendKind::Spotlight
        }
        fn implementation_status(&self) -> ImplementationStatus {
            ImplementationStatus::Real
        }
        fn search(
            &self,
            _intent: &SearchIntent,
            _cancel: CancellationToken,
        ) -> locifind_search_backend::BackendSearchFuture<'_> {
            Box::pin(async {
                Err(SearchError::Timeout { elapsed_ms: 42 })
            })
        }
    }

    /// 先发 1 条结果再 mid-stream err 的 backend。
    struct FakeMidErrBackend;
    impl SearchBackend for FakeMidErrBackend {
        fn kind(&self) -> BackendKind {
            BackendKind::Spotlight
        }
        fn implementation_status(&self) -> ImplementationStatus {
            ImplementationStatus::Real
        }
        fn search(
            &self,
            _intent: &SearchIntent,
            _cancel: CancellationToken,
        ) -> locifind_search_backend::BackendSearchFuture<'_> {
            Box::pin(async {
                let items: Vec<Result<SearchResult, SearchError>> = vec![
                    Ok(SearchResult {
                        id: "x".into(),
                        path: PathBuf::from("/tmp/x"),
                        name: "x".into(),
                        source: BackendKind::Spotlight,
                        match_type: MatchType::Filename,
                        score: None,
                        metadata: SearchResultMetadata::default(),
                    }),
                    Err(SearchError::Io {
                        detail: "boom".into(),
                    }),
                ];
                Ok(Box::pin(stream::iter(items)) as BackendStream)
            })
        }
    }

    fn build_registry(backend: impl SearchBackend + 'static) -> Arc<ToolRegistry> {
        let mut r = ToolRegistry::new();
        let tool = SearchTool::new(
            "search.fake",
            "Fake",
            backend,
            vec![SupportedIntent::FileSearch],
            "fake backend for test",
        );
        r.register_search(tool).unwrap();
        Arc::new(r)
    }

    fn build_tracer_with_mock() -> (Arc<Tracer>, Arc<Mutex<Vec<String>>>) {
        let (mock, calls) = MockHook::new();
        let tracer = Arc::new(Tracer::with_hooks(vec![Box::new(mock)]));
        (tracer, calls)
    }

    /// 用一个明确解析为 FileSearch 的 query;若 parser 行为变了,
    /// 测试期望也需同步。
    const QUERY_FOR_FILE_SEARCH: &str = "find pdf";

    #[tokio::test]
    async fn search_impl_success_emits_call_then_result() {
        let registry = build_registry(FakeOkBackend(3));
        let policy = Arc::new(PolicyEngine::new());
        let (tracer, calls) = build_tracer_with_mock();
        let (ch, captured) = capture_channel();

        search_impl(QUERY_FOR_FILE_SEARCH.into(), ch, registry, policy, tracer)
            .await
            .unwrap();

        let calls = calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 2, "应 1 call + 1 result, 实得 {calls:?}");
        assert!(calls[0].starts_with("call:search.fake"));
        assert_eq!(calls[1], "result:search.fake:3");

        let events = captured.lock().unwrap();
        assert!(events.iter().any(|e| e.contains("\"started\"")));
        assert!(events.iter().any(|e| e.contains("\"complete\"")));
    }

    #[tokio::test]
    async fn search_impl_open_err_emits_call_then_error() {
        let registry = build_registry(FakeOpenErrBackend);
        let policy = Arc::new(PolicyEngine::new());
        let (tracer, calls) = build_tracer_with_mock();
        let (ch, _captured) = capture_channel();

        search_impl(QUERY_FOR_FILE_SEARCH.into(), ch, registry, policy, tracer)
            .await
            .unwrap();

        let calls = calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 2, "应 1 call + 1 error, 实得 {calls:?}");
        assert!(calls[0].starts_with("call:search.fake"));
        assert_eq!(calls[1], "error:search.fake:Timeout");
    }

    #[tokio::test]
    async fn search_impl_mid_stream_err_emits_call_then_error() {
        let registry = build_registry(FakeMidErrBackend);
        let policy = Arc::new(PolicyEngine::new());
        let (tracer, calls) = build_tracer_with_mock();
        let (ch, _captured) = capture_channel();

        search_impl(QUERY_FOR_FILE_SEARCH.into(), ch, registry, policy, tracer)
            .await
            .unwrap();

        let calls = calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 2, "应 1 call + 1 error, 实得 {calls:?}");
        assert!(calls[0].starts_with("call:search.fake"));
        assert_eq!(calls[1], "error:search.fake:Io");
    }

    #[tokio::test]
    async fn search_impl_pre_tool_failure_emits_no_trace() {
        // clarify 类 query → router 拒(pre-tool)
        let registry = build_registry(FakeOkBackend(0));
        let policy = Arc::new(PolicyEngine::new());
        let (tracer, calls) = build_tracer_with_mock();
        let (ch, _captured) = capture_channel();

        // "搜下" 应被 parser 判为 Clarify → router 拒
        search_impl("搜下".into(), ch, registry, policy, tracer)
            .await
            .unwrap();

        let calls = calls.lock().unwrap().clone();
        assert!(
            calls.is_empty(),
            "pre-tool 失败不应触发 trace, 实得 {calls:?}"
        );
    }

    // 保留原有 search_error_kind 测试
    use locifind_search_backend::SearchError as SE2;
    #[test]
    fn search_error_kind_maps_all_variants() {
        assert_eq!(search_error_kind(&SE2::BackendUnavailable { reason: "x".into() }), "BackendUnavailable");
        assert_eq!(search_error_kind(&SE2::PermissionDenied { path: Some(PathBuf::from("/x")) }), "PermissionDenied");
        assert_eq!(search_error_kind(&SE2::InvalidIntent { detail: "x".into() }), "InvalidIntent");
        assert_eq!(search_error_kind(&SE2::UnsupportedIntent { detail: "x".into() }), "UnsupportedIntent");
        assert_eq!(search_error_kind(&SE2::Timeout { elapsed_ms: 1000 }), "Timeout");
        assert_eq!(search_error_kind(&SE2::Io { detail: "x".into() }), "Io");
    }

    // Helper unused if everything compiles
    #[allow(dead_code)]
    fn _unused_capability() -> Capability {
        Capability::default()
    }
    #[allow(dead_code)]
    fn _unused_fi(_: FileSearchIntent) {}
}
```

- [ ] **Step 2: 跑测试观察 fail/error 分布**

```bash
cd /Users/alice/Work/LocalFind && cargo test -p locifind-desktop 2>&1 | tail -40
```

可能命中:
- 编译错: `SearchBackend` trait 路径偏差 / `SearchTool::new` 签名变 / `Capability` 路径偏差 — 修 use 路径
- `QUERY_FOR_FILE_SEARCH = "find pdf"` parser 实际输出不是 FileSearch — 调 query 至确认能命中 FileSearch
- `"搜下"` 实际 parser 输出不是 Clarify — 调 query 至命中 pre-tool 失败

- [ ] **Step 3: 修复直到全 pass**

按编译/断言错误逐项修。常见调整点:

- `use` 路径(`locifind_harness::*` vs `locifind_search_backend::*`)对照 search.rs 主体既有 import
- 若 parser 把 `"find pdf"` 解析为 MediaSearch,改 fake tool 的 supported_intents 加 MediaSearch,或换 query
- 若 `"搜下"` 经 parser 直接返回 Clarify,router::route_search 会报 not routable → pre-tool 失败 → trace 空(符合预期)

终态:5 个 test 全 PASS(原有 + 4 新 search_impl + search_error_kind 1)。

- [ ] **Step 4: 清理 dead_code 占位 + clippy/fmt**

把 `_unused_capability` / `_unused_fi` 删掉(如果上述 use 不需要它们撑场)。

```bash
cd /Users/alice/Work/LocalFind && cargo fmt --all && cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -20
```
Expected: no warnings。

- [ ] **Step 5: Commit**

```bash
cd /Users/alice/Work/LocalFind && git add apps/desktop/src-tauri/src/search.rs && git commit -m "desktop: search_impl 集成测试(MockHook + fake backend × 3 路径)"
```

---

### Task 7: pre-tool 失败补 eprintln 日志

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`（`search_impl` 内 3 个 Error 分支前各加 1 行 eprintln）

- [ ] **Step 1: intent 解析 err 加 eprintln**

把 `search_impl` 内 `// 1) NL → intent` 段的 `Err(err) => {` 分支首行加:

```rust
        Err(err) => {
            eprintln!("search: intent 解析失败: {err}");
            let _ = on_event.send(SearchEvent::Error {
                message: err.to_string(),
            });
            return Ok(());
        }
```

- [ ] **Step 2: policy denied / RequireConfirmation 加 eprintln**

把 `// 2) Policy gate` 段的两个分支首行加:

```rust
        PolicyDecision::Deny { reason } => {
            eprintln!("search: policy 拒绝: {reason}");
            let _ = on_event.send(SearchEvent::Error {
                message: format!("policy denied: {reason}"),
            });
            return Ok(());
        }
        PolicyDecision::RequireConfirmation => {
            eprintln!("search: 不应触发 RequireConfirmation(intent 路由 bug)");
            let _ = on_event.send(SearchEvent::Error {
                message: "search 不应触发 RequireConfirmation".to_owned(),
            });
            return Ok(());
        }
```

- [ ] **Step 3: route_search err 加 eprintln**

把 `// 3) Intent → SearchableTool` 段的 `Err(err) => {` 分支首行加:

```rust
        Err(err) => {
            eprintln!("search: 无可用 tool: {err}");
            let _ = on_event.send(SearchEvent::Error {
                message: err.to_string(),
            });
            return Ok(());
        }
```

- [ ] **Step 4: 跑全部 test**

```bash
cd /Users/alice/Work/LocalFind && cargo test -p locifind-desktop 2>&1 | tail -20
```
Expected: 全 PASS(eprintln 不影响测试断言)。

- [ ] **Step 5: clippy + fmt**

```bash
cd /Users/alice/Work/LocalFind && cargo fmt --all && cargo clippy -p locifind-desktop --all-targets -- -D warnings 2>&1 | tail -20
```
Expected: no warnings。

- [ ] **Step 6: Commit**

```bash
cd /Users/alice/Work/LocalFind && git add apps/desktop/src-tauri/src/search.rs && git commit -m "desktop: search_impl pre-tool 失败 3 处加 eprintln(辅助开发观测)"
```

---

### Task 8: workspace 验证 + 手测 + STATUS/ROADMAP 同步

**Files:**
- Modify: `STATUS.md`、`ROADMAP.md`（同步本会话产出 + Class B backlog 去掉此项）

- [ ] **Step 1: 全 workspace CI**

```bash
cd /Users/alice/Work/LocalFind && bash scripts/ci.sh 2>&1 | tail -25
```
Expected: fmt + clippy + build + test 全 PASS。

- [ ] **Step 2: v0.5 evals parser-only baseline byte-equal 不可回归(可选,仅 wiring 不动 parser)**

由于本 wiring 仅动 Tauri 桥接层,完全不改 parser / harness / evals,可跳过 evals 重跑。若需安心:

```bash
cd /Users/alice/Work/LocalFind && cargo run --release -p locifind-evals --bin evals -- --json 2>&1 | tail -10
```
Expected: pass 472 / partial 26 / fail 2(parser-only baseline)。

- [ ] **Step 3: 手测 — 不设 env(无文件)**

请用户执行(agent 无法点 Tauri 窗口):

```bash
cd /Users/alice/Work/LocalFind/apps/desktop && npm run tauri dev
# 窗口出来后输入 "find pdf" 回车,观察:
# - 结果照常显示(IntentBadge + 流式 + Complete)
# - ls /tmp/locifind-trace.jsonl 不存在
```

- [ ] **Step 4: 手测 — 设 env(2 行 JSONL)**

```bash
# 关上一步的 dev,然后:
cd /Users/alice/Work/LocalFind/apps/desktop && LOCIFIND_TRACE=/tmp/locifind-trace.jsonl npm run tauri dev
# 窗口出来后输入 "find pdf" 回车,然后查看:
cat /tmp/locifind-trace.jsonl
# Expected: 2 行 — 第 1 行 {"tag":"tool_call",...,"tool_id":"search.spotlight",...}
#                 第 2 行 {"tag":"tool_result",...,"result_count":N,...}
```

- [ ] **Step 5: 手测 — pre-tool 失败不进 trace**

```bash
# dev 仍开着,输入 "搜下" 回车,然后:
wc -l /tmp/locifind-trace.jsonl
# Expected: 2(没新增,因为 router 拒了)
# 同时 dev 的 stderr 应看到 "search: 无可用 tool: clarify intent is not routable" 之类
```

- [ ] **Step 6: 关 dev server + 清 trace 文件**

```bash
pkill -f "tauri dev" 2>&1; pkill -f locifind-desktop 2>&1
rm -f /tmp/locifind-trace.jsonl
```

- [ ] **Step 7: 更新 STATUS.md「当前 Task」+ 顶部一句话总结 + 会话日志**

参考第 27 阶段 Slice B 的格式,在 STATUS.md `## 当前 Task` 上方 `>` 块新加一段:

```markdown
> **Class B Tracing/Hooks 接入 Tauri search**(第 28 阶段):承接 Slice B 后的代码层 backlog 顶部。完整 superpowers 流程:brainstorming → writing-plans → executing-plans。用户对齐 3 边界:用途=开发/调试观测、默认=NoopHook + env `LOCIFIND_TRACE` 开关、pre-tool 失败(intent/policy/router)不进 Tracer 沿用 eprintln。改动:main.rs `build_tracer()` 函数 + `.manage(Arc<Tracer>)` + 3 单测;search.rs 抽 `search_impl` inner fn + 注入 tracer State + 3 trace 点(call/result/error)+ `search_error_kind` helper + MockHook 集成测试(success/open-err/mid-err/pre-tool 4 路径)+ pre-tool 3 处 eprintln。净增 LOC ≈ 100。`bash scripts/ci.sh` 全过;evals 不动(wiring 层),byte-equal 维持 pass 472 parser-only / 480 hybrid Q4_K_M。
```

并把 `## 当前 Task` 改为本会话标识,会话日志在文件顶部追加(对照之前阶段格式)。

- [ ] **Step 8: ROADMAP.md 在 M4 / Class B backlog 同步**

`ROADMAP.md` 内:

- 把 STATUS.md「Class B 代码层 backlog」段的「Tracing / Hooks 接入 search command」一行打勾 / 移除
- M4 / MVP-19 行不需要改(Slice B 已 done),Tracing 是 Class B 副线,不入 M5 进度

- [ ] **Step 9: 最终 commit**

```bash
cd /Users/alice/Work/LocalFind && git add STATUS.md ROADMAP.md && git commit -m "Class B Tracing/Hooks 接入 Tauri search 收工:STATUS + ROADMAP 同步"
```

- [ ] **Step 10: 向用户报告**

输出本会话产出摘要:8 个 commit / 净增 LOC / 测试数 / 手测 3 case 结果 / 下一步候选。

---

## Self-Review

### 1. Spec coverage

| Spec §  | 内容 | 实施 task |
|---|---|---|
| §2 In: main.rs `build_tracer` + State 注入 | ✅ Task 1 + 2 |
| §2 In: search.rs 3 trace 点 | ✅ Task 5 (B/A/C) |
| §2 In: 单测 build_tracer + search MockHook | ✅ Task 2 (3 个) + Task 6 (4 个) + Task 3 (helper 1 个) |
| §2 Out: 不动 Tracer schema / 不引 tracing crate / 不接前端 / 不接 ContextMemory / pre-tool 不入 Tracer | ✅ 全 plan 遵守 |
| §4.1 build_tracer 签名 + 行为 | ✅ Task 2 step 3 完整实现 |
| §4.2 search 签名 + State 注入 | ✅ Task 4 step 1 |
| §4.3 search_error_kind | ✅ Task 3 |
| §4.4 intent_to_supported(已发现复用 SupportedIntent::from_intent) | ✅ Task 5 step 1 直接调 from_intent,省 helper |
| §5 trace 点 ABC + payload | ✅ Task 5 step 1/2/3/4 |
| §5 不进 Tracer 失败位点的 eprintln | ✅ Task 7 |
| §6 配置三态(未设/有效/无效) | ✅ Task 2 测试 + Task 8 手测 |
| §7 隐私 — schema 无 path/query/body / error_type 仅 variant 名 | ✅ Task 3 设计 |
| §8.1 build_tracer 3 测试(hook_count debug 字段) | ✅ Task 1+2 |
| §8.2 search MockHook 4 测试(success/open-err/mid-err/pre-tool) | ✅ Task 6 |
| §8.3 3 个手测 case | ✅ Task 8 step 3/4/5 |
| §9 R1(单测 channel 依赖) | ✅ 抽 search_impl + Channel::new 公开 API 解决 |
| §9 R2(env 测试 flake) | ✅ Task 1 mutex(OnceLock 无新 dep) |
| §10 不可回归(evals byte-equal) | ✅ Task 8 step 2 |
| §11 LOC ≈ 100 | ✅ 与 plan 实际相符 |

无 spec 项缺 task。

### 2. Placeholder scan

- ❌ ~~"TBD / TODO"~~:0 处
- ❌ ~~"添加适当错误处理"~~:0 处(所有错误分支有具体代码)
- ❌ ~~"参考 Task N"~~:0 处(所有代码块自包含)
- Task 6 step 3 写「按编译/断言错误逐项修」+ 列出常见调整点 — 这不是 placeholder 而是真实灰色区域(parser 对特定 query 的行为依赖 fixture 状态),合理。

### 3. Type consistency

- `Arc<Tracer>` 全程一致(Task 1/2/4/5/6)
- `tracer.on_tool_call` / `on_tool_result` / `on_error` 方法名对应 Tracer trait
- `ToolCallEvent` / `ToolResultEvent` / `ToolErrorEvent` 字段名对照 spec §5 与 tracing.rs 实际定义
- `SupportedIntent::from_intent(&SearchIntent)` 复用 harness 已有方法(grep 已确认)
- `search_error_kind` 签名 `(&SearchError) -> &'static str` 在 Task 3 / Task 5 / Task 6 一致

无不一致。

---

## 总结

**8 task / 预计 ~3 小时**(每 task 含 commit,bite-sized step,无外部依赖)。
**8 commit**:1 骨架 / 2 env 完整 / 3 helper / 4 重构 / 5 trace 点 / 6 集成测试 / 7 eprintln / 8 收工。
**LOC**:main.rs +50(含测试)/ search.rs +200(含测试)= 净增 ~250(spec 估 100,实际因测试 fixture 较重)。
