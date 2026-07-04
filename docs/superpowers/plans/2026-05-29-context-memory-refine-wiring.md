# ContextMemory 多轮接入 Tauri search command 实现计划

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 harness 的 `ContextMemory` 接进 Tauri `search` command，使 `SearchIntent::Refine` 能合并上一轮基准并执行，实现多轮渐进收窄。

**Architecture:** 方案 A —— `search_impl` 内联合并。新增 `Arc<Mutex<ContextMemory>>` 作 Tauri managed State；解析出 intent 后若为 `Refine` 则锁 context 调 `apply_refine` 得到具体 FileSearch/MediaSearch（无上一轮 → `SearchEvent::Error`）；流式结束后 `record(effective, results)` 为新一轮。harness ContextMemory 一行不动。

**Tech Stack:** Rust / Tauri 2 / `std::sync::Mutex` / `futures::StreamExt` / `#[tokio::test]`。

设计 spec：[docs/superpowers/specs/2026-05-29-context-memory-refine-wiring-design.md](../specs/2026-05-29-context-memory-refine-wiring-design.md)

---

## 关键事实（实现前必读）

- `ContextMemory` 在 harness `pub mod context` 下，**未在顶层 re-export**。引用路径：`locifind_harness::context::{ContextMemory, RefineMergeError}`。
- `apply_refine(&self, &Refine) -> Result<RefineOutcome, RefineMergeError>`；`RefineOutcome { intent: SearchIntent, conflicts: Vec<RefineConflict> }`。合并产出必为 FileSearch/MediaSearch。
- `record(&mut self, intent: SearchIntent, results: Vec<SearchResult>)`。
- `RefineMergeError` 变体：`NoLastIntent` / `InvalidBase { intent_kind }` / `FieldNotApplicable { field, base }`，实现了 `Display`。
- `SearchResult` derive 了 `Clone`；`SearchIntent` derive 了 `Clone`。
- **clippy 约束**：workspace `unwrap_used` / `expect_used` = `warn`，CI 跑 `-D warnings`。生产代码 lock 一律用 `.lock().unwrap_or_else(|e| e.into_inner())`（main.rs 测试已用此写法，见 main.rs:221）。**测试模块**已 `#![allow(clippy::unwrap_used, clippy::expect_used)]`，测试内可 `.unwrap()`。
- 未使用的函数参数在 `-D warnings` 下会失败 —— 故 `context` 参数的"加入"与"使用"必须同一 task 落地。
- 已 de-risk 的查询串（CLI `--intent-only` 实测）：
  - `find pdf` → `FileSearch`（见 search.rs 既有常量 `QUERY_FOR_FILE_SEARCH`）
  - `只看 png` → `Refine { delta: { extensions:["png"], file_type:image } }`
  - `只看下载目录` → `Refine { delta: { location:{ hint:"下载" } } }`
  - `找最近的` → `Clarify`（见 search.rs 既有常量 `QUERY_CLARIFY`）

## 文件结构

| 文件 | 责任 | 改动 |
|---|---|---|
| `apps/desktop/src-tauri/src/search.rs` | Tauri search 桥层：解析 → 合并 → policy → route → 流式 → record | 加 `apply_refine_if_needed` 自由函数；`search_impl` 加 `context` 参数 + 合并 + record；`search` 包装解新 State；新增 `FakeCapturingBackend` + 6 个新测试；更新既有 4 个集成测试调用点 |
| `apps/desktop/src-tauri/src/main.rs` | 构造并注入 managed State | 加 `ContextMemory` State `.manage(...)` |

无 TS 改动。无 parser / harness / evals 改动。

---

## Task 1：`apply_refine_if_needed` 自由函数 + 单元测试

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`（加自由函数 + 测试模块内 3 个单测）

- [ ] **Step 1: 写失败测试（3 个单测）**

在 `search.rs` 测试模块 `mod tests` 内追加（放在文件末尾 `}` 之前）。先在测试模块顶部已有的 `use` 区补充导入（与现有 `use` 并列）：

```rust
    use locifind_harness::context::{ContextMemory, RefineMergeError};
    use locifind_search_backend::{
        BaseRef, FileSearch, FileType, Language, Refine, RefineDelta, SchemaVersion,
    };
```

测试本体：

```rust
    // ---- Task 1: apply_refine_if_needed 单元测试 ----

    fn mk_base_file_search_pdf() -> SearchIntent {
        SearchIntent::FileSearch(FileSearch {
            schema_version: SchemaVersion::V1,
            language: Some(Language::Zh),
            keywords: None,
            extensions: Some(vec!["pdf".to_owned()]),
            file_type: Some(FileType::Document),
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

    fn mk_refine_extensions_png() -> SearchIntent {
        SearchIntent::Refine(Refine {
            schema_version: SchemaVersion::V1,
            language: Some(Language::Zh),
            base_ref: BaseRef::LastIntent,
            delta: RefineDelta {
                extensions: Some(vec!["png".to_owned()]),
                ..RefineDelta::default()
            },
            clear: None,
        })
    }

    #[test]
    fn apply_refine_passthrough_non_refine() {
        let ctx = ContextMemory::new();
        let intent = mk_base_file_search_pdf();
        let out = apply_refine_if_needed(intent, &ctx).unwrap();
        match out {
            SearchIntent::FileSearch(fs) => {
                assert_eq!(fs.extensions, Some(vec!["pdf".to_owned()]));
            }
            other => panic!("应原样返回 FileSearch, 实得 {other:?}"),
        }
    }

    #[test]
    fn apply_refine_merges_with_context() {
        let mut ctx = ContextMemory::new();
        ctx.record(mk_base_file_search_pdf(), vec![]);
        let out = apply_refine_if_needed(mk_refine_extensions_png(), &ctx).unwrap();
        match out {
            SearchIntent::FileSearch(fs) => {
                // delta 覆盖：extensions 应变成 png
                assert_eq!(fs.extensions, Some(vec!["png".to_owned()]));
            }
            other => panic!("合并后应是 FileSearch, 实得 {other:?}"),
        }
    }

    #[test]
    fn apply_refine_without_context_errors() {
        let ctx = ContextMemory::new();
        let err = apply_refine_if_needed(mk_refine_extensions_png(), &ctx).unwrap_err();
        assert!(
            matches!(err, RefineMergeError::NoLastIntent),
            "空 context 应 NoLastIntent, 实得 {err:?}"
        );
    }
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test -p locifind-desktop apply_refine 2>&1 | tail -20`
Expected: 编译失败 —— `cannot find function apply_refine_if_needed`。

> 包名为 `locifind-desktop`（已确认，见 `apps/desktop/src-tauri/Cargo.toml`）。

- [ ] **Step 3: 写最小实现**

在 `search.rs` 顶部 `use` 区补充：

```rust
use std::sync::Mutex;

use locifind_harness::context::{ContextMemory, RefineMergeError};
```

在 `search_impl` 函数之后、`describe_intent` 之前加自由函数：

> **dead_code 暂存标注**：本 task 只加函数、不接调用点（调用点在 Task 2），故在 binary target 下该函数是 dead code，`clippy --all-targets -D warnings` 会失败。加 `#[cfg_attr(not(test), allow(dead_code))]` 标注为"暂存"。**Task 2 接线后移除此属性**。

```rust
/// 若 intent 是 `Refine`，按 [`ContextMemory`] 合并上一轮基准；否则原样返回。
///
/// `conflicts`（clear 与 delta 同名字段冲突，已按 clear 为准合并）走 `eprintln`
/// 记录，不阻断。合并失败（无上一轮 / 基准非法 / 字段不适用）返回
/// [`RefineMergeError`]，由调用方转 [`SearchEvent::Error`]。
// Task 1 暂存:调用点在 Task 2 接入,届时移除本属性。
#[cfg_attr(not(test), allow(dead_code))]
fn apply_refine_if_needed(
    intent: SearchIntent,
    ctx: &ContextMemory,
) -> Result<SearchIntent, RefineMergeError> {
    match intent {
        SearchIntent::Refine(refine) => {
            let outcome = ctx.apply_refine(&refine)?;
            if !outcome.conflicts.is_empty() {
                eprintln!(
                    "search: refine 字段冲突(以 clear 为准): {:?}",
                    outcome.conflicts
                );
            }
            Ok(outcome.intent)
        }
        other => Ok(other),
    }
}
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test -p locifind-desktop apply_refine 2>&1 | tail -20`
Expected: 3 个测试 PASS（`apply_refine_passthrough_non_refine` / `apply_refine_merges_with_context` / `apply_refine_without_context_errors`）。

- [ ] **Step 5: 提交**

```bash
git add apps/desktop/src-tauri/src/search.rs
git commit -m "desktop: 加 apply_refine_if_needed 合并函数 + 单测(ContextMemory 接入 1/3)"
```

---

## Task 2：`search_impl` 接入 ContextMemory（合并 + record）+ State 注入

本 task 把 `context` 参数贯穿端到端并真正使用（合并 + record），同步更新 `search` 包装、`main.rs` State 注入、以及既有 4 个集成测试调用点。完成后既有测试全绿（无回归），新行为由 Task 3 验证。

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`（`search` + `search_impl` + 4 个既有测试调用点）
- Modify: `apps/desktop/src-tauri/src/main.rs`（`.manage` 注入）

- [ ] **Step 1: 改 `search` 包装解新 State 并传入**

把 `search` 命令替换为（新增 `context` State 解包 + 透传）：

```rust
/// 主搜索 command:thin wrapper,解 State 后委托 [`search_impl`]。
#[tauri::command]
pub async fn search(
    query: String,
    on_event: Channel<SearchEvent>,
    registry: tauri::State<'_, Arc<ToolRegistry>>,
    policy: tauri::State<'_, Arc<PolicyEngine>>,
    tracer: tauri::State<'_, Arc<locifind_harness::Tracer>>,
    context: tauri::State<'_, Arc<Mutex<ContextMemory>>>,
) -> Result<(), String> {
    search_impl(
        query,
        on_event,
        Arc::clone(&*registry),
        Arc::clone(&*policy),
        Arc::clone(&*tracer),
        Arc::clone(&*context),
    )
    .await
}
```

- [ ] **Step 2: 改 `search_impl` 签名 + 合并 + record**

> **先移除 Task 1 的暂存属性**：删掉 `apply_refine_if_needed` 上方的 `// Task 1 暂存:...` 注释行与 `#[cfg_attr(not(test), allow(dead_code))]` 属性行 —— 本 task 在 `search_impl` 里真正调用了该函数，binary target 不再 dead code。

把 `search_impl` 整体替换为下面版本（核心改动：destructure `resolved`；`resolve_intent` 后插入合并；所有 `&resolved.intent` 改 `&effective`；流式累积 `Vec<SearchResult>`；`Complete` 前 `record`）：

```rust
/// search command 主体。不依赖 [`tauri::State`],可被单测注入 mock 调用。
///
/// 返回 `Result<(), String>` 仅表示"任务派发是否成功"；查询结果与失败均通过
/// `on_event` 流式投递(包括 `SearchEvent::Error`)。
pub(crate) async fn search_impl(
    query: String,
    on_event: Channel<SearchEvent>,
    registry: Arc<ToolRegistry>,
    policy: Arc<PolicyEngine>,
    tracer: Arc<locifind_harness::Tracer>,
    context: Arc<Mutex<ContextMemory>>,
) -> Result<(), String> {
    let start = Instant::now();

    // 1) NL → intent
    let resolved = match resolve_intent(&query, None) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("search: intent 解析失败: {err}");
            let _ = on_event.send(SearchEvent::Error {
                message: err.to_string(),
            });
            return Ok(());
        }
    };
    let locifind_intent_parser::fallback::ResolvedIntent {
        intent, source, signals, ..
    } = resolved;

    // 2) Refine 合并:Refine → 合并上一轮基准；其余原样。pre-tool 失败不进 trace。
    let effective = {
        let guard = context.lock().unwrap_or_else(|e| e.into_inner());
        apply_refine_if_needed(intent, &guard)
    };
    let effective = match effective {
        Ok(i) => i,
        Err(err) => {
            let message = match err {
                RefineMergeError::NoLastIntent => {
                    "没有可细化的上一轮搜索，请先发起一次搜索".to_owned()
                }
                other => other.to_string(),
            };
            eprintln!("search: refine 合并失败: {message}");
            let _ = on_event.send(SearchEvent::Error { message });
            return Ok(());
        }
    };

    // 3) Policy gate(跑在合并后的 effective 上)
    let action = PolicyAction::from(&effective);
    match policy.evaluate(&action) {
        PolicyDecision::Allow => {}
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
    }

    // 4) Intent → SearchableTool
    let router = IntentRouter::new(&registry);
    let tool = match router.route_search(&effective) {
        Ok(t) => t,
        Err(err) => {
            eprintln!("search: 无可用 tool: {err}");
            let _ = on_event.send(SearchEvent::Error {
                message: err.to_string(),
            });
            return Ok(());
        }
    };

    let tool_id = tool.id().to_owned();
    let tool_start = Instant::now();

    // Trace A: tool 即将被调用
    tracer.on_tool_call(&locifind_harness::ToolCallEvent {
        tool_id: tool_id.clone(),
        tool_kind: locifind_harness::ToolKind::Search,
        intent_variant: locifind_harness::SupportedIntent::from_intent(&effective),
    });

    // 5) Started 事件
    let _ = on_event.send(SearchEvent::Started {
        intent_summary: describe_intent(&effective),
        fallback_used: matches!(source, IntentSource::Model),
        signals: signals_to_labels(&signals),
        tool_id: tool_id.clone(),
    });

    // 6) 流式调用 backend
    let cancel = CancellationToken::new();
    let mut stream = match tool.search(&effective, cancel).await {
        Ok(s) => s,
        Err(err) => {
            tracer.on_error(&locifind_harness::ToolErrorEvent {
                tool_id: tool_id.clone(),
                duration: tool_start.elapsed(),
                error_type: search_error_kind(&err).to_owned(),
            });
            let _ = on_event.send(SearchEvent::Error {
                message: err.to_string(),
            });
            return Ok(());
        }
    };

    let mut total = 0usize;
    // record 需要原始 SearchResult；边发 UI 事件边累积。
    let mut recorded: Vec<locifind_search_backend::SearchResult> = Vec::new();
    while let Some(item) = stream.next().await {
        match item {
            Ok(result) => {
                let json = SearchResultJson {
                    id: result.id.clone(),
                    path: result.path.to_string_lossy().into_owned(),
                    name: result.name.clone(),
                    source: format!("{:?}", result.source).to_lowercase(),
                    match_type: format!("{:?}", result.match_type).to_lowercase(),
                    score: result.score,
                    modified_time: result.metadata.modified_time.map(|t| t.to_rfc3339()),
                    size_bytes: result.metadata.size_bytes,
                };
                total += 1;
                recorded.push(result);
                if on_event.send(SearchEvent::Result { item: json }).is_err() {
                    break;
                }
            }
            Err(err) => {
                tracer.on_error(&locifind_harness::ToolErrorEvent {
                    tool_id: tool_id.clone(),
                    duration: tool_start.elapsed(),
                    error_type: search_error_kind(&err).to_owned(),
                });
                let _ = on_event.send(SearchEvent::Error {
                    message: err.to_string(),
                });
                return Ok(());
            }
        }
    }

    tracer.on_tool_result(&locifind_harness::ToolResultEvent {
        tool_id: tool_id.clone(),
        duration: tool_start.elapsed(),
        result_count: total,
    });

    // 7) 成功完成 → 记录本轮为新的 last turn(渐进收窄链的基准)
    {
        let mut guard = context.lock().unwrap_or_else(|e| e.into_inner());
        guard.record(effective, recorded);
    }

    let elapsed_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    let _ = on_event.send(SearchEvent::Complete { total, elapsed_ms });
    Ok(())
}
```

> 注意：`describe_intent` / `signals_to_labels` / `search_error_kind` / `SearchResultJson` / `SearchEvent` 等保持不变。`IntentSource` 已在文件顶部 `use locifind_intent_parser::fallback::{resolve_intent, IntentSource};` 导入；`ResolvedIntent` 用全路径 `locifind_intent_parser::fallback::ResolvedIntent` 即可（无需额外 import）。

- [ ] **Step 3: 更新既有 4 个集成测试调用点**

既有测试 `search_impl_success_emits_call_then_result` / `search_impl_open_err_emits_call_then_error` / `search_impl_mid_stream_err_emits_call_then_error` / `search_impl_pre_tool_failure_emits_no_trace` 每个都调用 `search_impl(...)`，现在需多传一个 `context` 实参。

在测试模块加一个 helper（放在 `build_tracer_with_mock` 附近）：

```rust
    fn empty_context() -> Arc<Mutex<ContextMemory>> {
        Arc::new(Mutex::new(ContextMemory::new()))
    }
```

把 4 处调用从：

```rust
        search_impl(QUERY_FOR_FILE_SEARCH.into(), ch, registry, policy, tracer)
            .await
            .unwrap();
```

改为（每处对应的 query 常量不变，仅在末尾加 `empty_context()`）：

```rust
        search_impl(
            QUERY_FOR_FILE_SEARCH.into(),
            ch,
            registry,
            policy,
            tracer,
            empty_context(),
        )
        .await
        .unwrap();
```

（`search_impl_pre_tool_failure_emits_no_trace` 用的是 `QUERY_CLARIFY`，同样在末尾加 `empty_context()`。）

测试模块顶部 `use` 区需确保已导入 `ContextMemory`（Task 1 Step 1 已加 `use locifind_harness::context::{ContextMemory, RefineMergeError};`）与 `Mutex`（已有 `use std::sync::{Arc, Mutex};`，确认；若只有 `Arc` 则补 `Mutex`）。

- [ ] **Step 4: 改 `main.rs` 注入 State**

`main.rs` 顶部 `use` 区改：

```rust
use std::sync::{Arc, Mutex};

use locifind_harness::{PolicyEngine, SearchTool, SupportedIntent, ToolRegistry, Tracer};
use locifind_harness::context::ContextMemory;
```

`fn main()` 内，在构造 tracer 之后加：

```rust
    let context = Arc::new(Mutex::new(ContextMemory::new()));
```

`tauri::Builder` 链上，在 `.manage(tracer)` 之后加：

```rust
        .manage(context)
```

- [ ] **Step 5: 跑测试确认全绿（无回归）**

Run: `cargo test -p locifind-desktop 2>&1 | tail -25`
Expected: 既有测试 + Task 1 的 3 个单测全 PASS；无编译警告（`-D warnings` 由 clippy 把关，见 Task 4）。

- [ ] **Step 6: 提交**

```bash
git add apps/desktop/src-tauri/src/search.rs apps/desktop/src-tauri/src/main.rs
git commit -m "desktop: search_impl 接入 ContextMemory 合并+record + State 注入(2/3)"
```

---

## Task 3：集成测试 —— record-then-refine / 无上下文 / 链式

新增可捕获 effective intent 的 fake backend，写 3 个集成测试验证多轮行为。

**Files:**
- Modify: `apps/desktop/src-tauri/src/search.rs`（测试模块加 `FakeCapturingBackend` + 3 个集成测试）

- [ ] **Step 1: 写失败测试（capturing backend + 3 集成测试）**

在测试模块加 capturing backend（放在 `FakeMidErrBackend` 之后）：

```rust
    /// 捕获每次 search 收到的 effective intent，返回 N 条 fake 结果。
    #[derive(Debug)]
    struct FakeCapturingBackend {
        seen: Arc<Mutex<Vec<SearchIntent>>>,
        n: usize,
    }
    impl SearchBackend for FakeCapturingBackend {
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
            intent: &'a SearchIntent,
            _cancel: CancellationToken,
        ) -> BackendSearchFuture<'a> {
            self.seen
                .lock()
                .unwrap()
                .push(intent.clone());
            let n = self.n;
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

    /// 用 capturing backend 建 registry，并返回捕获表句柄。
    fn build_capturing_registry(n: usize) -> (Arc<ToolRegistry>, Arc<Mutex<Vec<SearchIntent>>>) {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let backend = FakeCapturingBackend {
            seen: Arc::clone(&seen),
            n,
        };
        let mut r = ToolRegistry::new();
        let tool = SearchTool::new(
            "search.fake",
            "Fake",
            backend,
            vec![SupportedIntent::FileSearch, SupportedIntent::MediaSearch],
            "capturing fake backend",
        );
        r.register_search(tool).unwrap();
        (Arc::new(r), seen)
    }
```

集成测试（用已 de-risk 的查询串）：

```rust
    /// "只看 png" 稳定解析为 Refine(delta.extensions=[png])。
    const QUERY_REFINE_PNG: &str = "只看 png";
    /// "只看下载目录" 稳定解析为 Refine(delta.location.hint=下载)。
    const QUERY_REFINE_DOWNLOADS: &str = "只看下载目录";

    #[tokio::test]
    async fn search_impl_record_then_refine_merges_base() {
        let (registry, seen) = build_capturing_registry(2);
        let policy = Arc::new(PolicyEngine::new());
        let (tracer, _calls) = build_tracer_with_mock();
        let ctx = empty_context();

        // 第一轮:基准 find pdf → 记录 FileSearch{pdf}
        let (ch1, _c1) = capture_channel();
        search_impl(
            QUERY_FOR_FILE_SEARCH.into(),
            ch1,
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&tracer),
            Arc::clone(&ctx),
        )
        .await
        .unwrap();

        // 第二轮:refine 只看 png → 合并上一轮 → FileSearch{png}
        let (ch2, events2) = capture_channel();
        search_impl(
            QUERY_REFINE_PNG.into(),
            ch2,
            Arc::clone(&registry),
            Arc::clone(&policy),
            Arc::clone(&tracer),
            Arc::clone(&ctx),
        )
        .await
        .unwrap();

        // 第二轮应成功(complete 而非 error)
        let events2 = events2.lock().unwrap();
        assert!(
            events2.iter().any(|e| e.contains("\"complete\"")),
            "refine 第二轮应 complete, 实得: {events2:?}"
        );
        assert!(
            !events2.iter().any(|e| e.contains("\"error\"")),
            "refine 第二轮不应 error, 实得: {events2:?}"
        );

        // capturing backend 第二次收到的 effective intent 应是合并后的 FileSearch{png}
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 2, "应两次进 backend, 实得 {}", seen.len());
        match &seen[1] {
            SearchIntent::FileSearch(fs) => {
                assert_eq!(
                    fs.extensions,
                    Some(vec!["png".to_owned()]),
                    "合并后 extensions 应为 png"
                );
            }
            other => panic!("第二轮 effective 应是 FileSearch, 实得 {other:?}"),
        }
    }

    #[tokio::test]
    async fn search_impl_refine_without_context_errors() {
        let (registry, seen) = build_capturing_registry(1);
        let policy = Arc::new(PolicyEngine::new());
        let (tracer, calls) = build_tracer_with_mock();
        let ctx = empty_context();

        let (ch, events) = capture_channel();
        search_impl(
            QUERY_REFINE_PNG.into(),
            ch,
            registry,
            policy,
            tracer,
            ctx,
        )
        .await
        .unwrap();

        // 应 error 且文案含"上一轮"
        let events = events.lock().unwrap();
        assert!(
            events.iter().any(|e| e.contains("\"error\"") && e.contains("上一轮")),
            "空 context refine 应 error 且文案含'上一轮', 实得: {events:?}"
        );
        // pre-tool 失败:不进 trace、不进 backend
        assert!(calls.lock().unwrap().is_empty(), "合并失败不应 trace");
        assert!(seen.lock().unwrap().is_empty(), "合并失败不应进 backend");
    }

    #[tokio::test]
    async fn search_impl_chained_refine_accumulates() {
        let (registry, seen) = build_capturing_registry(1);
        let policy = Arc::new(PolicyEngine::new());
        let (tracer, _calls) = build_tracer_with_mock();
        let ctx = empty_context();

        // 基准 → refine1(下载目录) → refine2(png)
        for q in [QUERY_FOR_FILE_SEARCH, QUERY_REFINE_DOWNLOADS, QUERY_REFINE_PNG] {
            let (ch, _c) = capture_channel();
            search_impl(
                q.into(),
                ch,
                Arc::clone(&registry),
                Arc::clone(&policy),
                Arc::clone(&tracer),
                Arc::clone(&ctx),
            )
            .await
            .unwrap();
        }

        // 末轮 effective 应同时含 refine1(location 下载) + refine2(extensions png)
        let seen = seen.lock().unwrap();
        assert_eq!(seen.len(), 3, "应三轮进 backend");
        match &seen[2] {
            SearchIntent::FileSearch(fs) => {
                assert_eq!(
                    fs.extensions,
                    Some(vec!["png".to_owned()]),
                    "末轮应含 refine2 的 png"
                );
                let hint = fs.location.as_ref().and_then(|l| l.hint.as_deref());
                assert_eq!(hint, Some("下载"), "末轮应保留 refine1 的 location 下载");
            }
            other => panic!("末轮 effective 应是 FileSearch, 实得 {other:?}"),
        }
    }
```

- [ ] **Step 2: 跑测试**

Run: `cargo test -p locifind-desktop 2>&1 | tail -25`
Expected: 新 3 个 tokio 测试 + 既有测试全 PASS。

> 若 `search_impl_chained_refine_accumulates` 的 location 断言失败：检查 `apply_to_file_search` 对 location 的覆盖语义（delta location Some 时覆盖；refine2 的 delta 不含 location 故应保留 refine1 的）。若 `record` 链未保留 refine1 → 回看 Task 2 Step 2 是否 `record(effective, ...)` 用的是合并后 intent。

- [ ] **Step 3: 提交**

```bash
git add apps/desktop/src-tauri/src/search.rs
git commit -m "desktop: 多轮 refine 集成测试(record-then-refine/无上下文/链式)(3/3)"
```

---

## Task 4：全量 CI + 收尾

**Files:** 无新增改动（仅验证）。

- [ ] **Step 1: fmt**

Run: `cargo fmt --all`
Expected: 无 diff 或仅格式微调；若有 diff 则 `git add -A` 进下面提交。

- [ ] **Step 2: 全量 CI**

Run: `bash scripts/ci.sh`
Expected: fmt + clippy(`-D warnings`) + build + test 全过。**重点确认 clippy 不报 `unwrap_used` / `expect_used`**（生产代码 lock 用 `unwrap_or_else(|e| e.into_inner())`）。

- [ ] **Step 3: evals 不回归（抽查 parser-only baseline）**

Run: `cargo run -q -p locifind-evals --bin evals 2>&1 | tail -15`
Expected: parser-only baseline 维持 pass 472 / partial 26 / fail 2（wiring 层不动 parser；数字以最近 STATUS 为准，允许等值）。

- [ ] **Step 4: 提交 fmt（若 Step 1 有 diff）**

```bash
git add -A
git commit -m "desktop: ci fmt 收尾"
```

（若 Step 1 无 diff 则跳过本步。）

---

## 验收标准

- [ ] `apply_refine_if_needed` 3 单测过：非 Refine 透传 / Refine+context 合并 / Refine 空 context 报 NoLastIntent。
- [ ] `search_impl` 接 `Arc<Mutex<ContextMemory>>`，Refine 合并上一轮基准、成功后 record；合并失败转 `SearchEvent::Error` + 不 trace + 不 record。
- [ ] 3 集成测试过：record-then-refine 合并出 FileSearch{png}；空 context refine 报错文案含"上一轮"且不进 backend；链式 refine 末轮同时含 location + extensions。
- [ ] `main.rs` 注入 `ContextMemory` State；`search` 包装解新 State。
- [ ] `bash scripts/ci.sh` 全过（含 clippy `-D warnings`）。
- [ ] evals parser-only baseline 不回归。
- [ ] 无 TS 改动、无 parser/harness/evals 源改动。

## 收工时记录（未尽事宜）

- **macOS Tauri dev UI 真机手测**（agent 无法点击窗口，需用户驱动）：
  - `npm run tauri dev` 后：输入 `find pdf` → 有结果；再输入 `只看 png` → 结果应是 png（多轮收窄生效）；
  - 首查直接输入 `只看 png` → UI error 态显示"没有可细化的上一轮搜索"。
- 更新 STATUS.md 会话日志 + ROADMAP MVP-19 行（追加 ContextMemory 多轮接入）。
